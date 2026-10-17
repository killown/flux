use gio::prelude::FileExt;
use oxipng::{IndexSet, Options, StripChunks};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::OnceLock;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
const IHDR_END: usize = 33;
const KEY_URI: &str = "Thumb::URI";
const KEY_MTIME: &str = "Thumb::MTime";
const OPTIMIZE_QUEUE_LEN: usize = 256;

/// Optimizes raw PNG bytes with oxipng, keeping `tEXt` chunks such as `Thumb::MTime`.
pub fn optimize_png_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut keep = IndexSet::default();
    keep.insert(*b"tEXt");

    let mut opts = Options::from_preset(2);
    opts.strip = StripChunks::Keep(keep);

    oxipng::optimize_from_memory(bytes, &opts).unwrap_or_else(|_| bytes.to_vec())
}

/// Returns the modification time in whole seconds since the Unix epoch.
pub fn mtime_secs(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// Reads the `Thumb::MTime` value stored in a thumbnail PNG, if present.
pub fn read_thumb_mtime(png: &[u8]) -> Option<u64> {
    read_text_chunk(png, KEY_MTIME)?.trim().parse().ok()
}

fn read_text_chunk(png: &[u8], key: &str) -> Option<String> {
    if png.len() < 8 || png[..8] != PNG_SIGNATURE {
        return None;
    }
    let mut pos = 8;
    while pos + 8 <= png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().ok()?) as usize;
        let kind = &png[pos + 4..pos + 8];
        if kind == b"IDAT" || kind == b"IEND" {
            return None;
        }
        let data_end = pos.checked_add(8)?.checked_add(len)?;
        if data_end.checked_add(4)? > png.len() {
            return None;
        }
        if kind == b"tEXt" {
            let data = &png[pos + 8..data_end];
            if let Some(nul) = data.iter().position(|&b| b == 0) {
                if &data[..nul] == key.as_bytes() {
                    return Some(data[nul + 1..].iter().map(|&b| b as char).collect());
                }
            }
        }
        pos = data_end + 4;
    }
    None
}

fn text_chunk(key: &str, value: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(key.len() + 1 + value.len());
    data.extend_from_slice(key.as_bytes());
    data.push(0);
    data.extend_from_slice(value.as_bytes());

    let mut crc = flate2::Crc::new();
    crc.update(b"tEXt");
    crc.update(&data);

    let mut chunk = Vec::with_capacity(data.len() + 12);
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&data);
    chunk.extend_from_slice(&crc.sum().to_be_bytes());
    chunk
}

/// Inserts `tEXt` chunks right after IHDR, returning the input unchanged if it is not a PNG.
pub fn inject_text_chunks(png: &[u8], entries: &[(&str, &str)]) -> Vec<u8> {
    if png.len() < IHDR_END || png[..8] != PNG_SIGNATURE || &png[12..16] != b"IHDR" {
        return png.to_vec();
    }
    let mut out = Vec::with_capacity(png.len() + 128);
    out.extend_from_slice(&png[..IHDR_END]);
    for (key, value) in entries {
        out.extend_from_slice(&text_chunk(key, value));
    }
    out.extend_from_slice(&png[IHDR_END..]);
    out
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_atomic(dest: &Path, bytes: &[u8]) -> bool {
    let Some(dir) = dest.parent() else {
        return false;
    };
    let tmp = dir.join(format!(
        ".tmp.{}.{}.png",
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, dest).is_ok() {
        true
    } else {
        let _ = fs::remove_file(&tmp);
        false
    }
}

/// Writes a thumbnail PNG with `Thumb::URI`/`Thumb::MTime` and queues a background oxipng pass.
pub fn store_thumbnail(png: &[u8], source: &Path, cache_path: &Path) {
    let uri = gio::File::for_path(source).uri().to_string();
    let mtime = fs::metadata(source)
        .ok()
        .and_then(|m| mtime_secs(&m))
        .map(|s| s.to_string());

    let mut entries = vec![(KEY_URI, uri.as_str())];
    if let Some(m) = mtime.as_deref() {
        entries.push((KEY_MTIME, m));
    }

    let stamped = inject_text_chunks(png, &entries);
    if write_atomic(cache_path, &stamped) {
        queue_optimize(cache_path.to_path_buf(), stamped);
    }
}

fn queue_optimize(path: PathBuf, bytes: Vec<u8>) {
    static QUEUE: OnceLock<Option<SyncSender<(PathBuf, Vec<u8>)>>> = OnceLock::new();
    let tx = QUEUE.get_or_init(|| {
        let (tx, rx) = sync_channel::<(PathBuf, Vec<u8>)>(OPTIMIZE_QUEUE_LEN);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .thread_name(|_| "flux-png-optimizer".to_string())
            .build()
            .ok()?;
        std::thread::Builder::new()
            .name("flux-png-queue".to_string())
            .spawn(move || {
                for (path, original) in rx {
                    let optimized = pool.install(|| optimize_png_bytes(&original));
                    if optimized.len() < original.len()
                        && fs::read(&path).is_ok_and(|cur| cur == original)
                    {
                        write_atomic(&path, &optimized);
                    }
                }
            })
            .ok()?;
        Some(tx)
    });
    if let Some(tx) = tx {
        // Full queue: keep the valid, unoptimized file instead of blocking the caller.
        let _ = tx.try_send((path, bytes));
    }
}
