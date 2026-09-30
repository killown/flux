use aho_corasick::AhoCorasick;
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::FileExt;

#[allow(dead_code)]
pub const DEFAULT_MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;

const CHUNK: usize = 256 * 1024;
const SNIPPET_MAX: usize = 1024;
const BINARY_SNIFF: usize = 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub line_number: usize,
    pub line: String,
}

#[inline]
fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == b'\n').count()
}

fn read_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(total)
}

/// Reads a window around the match and returns the surrounding line.
/// Long lines get clipped to a fixed window centred on the match.
fn snippet(file: &File, m_start: u64, m_end: u64) -> Option<String> {
    let win_start = m_start.saturating_sub(SNIPPET_MAX as u64);
    let want = (m_end - win_start) as usize + SNIPPET_MAX;
    let mut win = vec![0u8; want];
    let mut got = 0usize;
    while got < want {
        match file.read_at(&mut win[got..], win_start + got as u64) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
    win.truncate(got);

    let rel_start = (m_start - win_start) as usize;
    if rel_start >= win.len() {
        return None;
    }
    let rel_end = ((m_end - win_start) as usize).min(win.len());

    // Expand outwards to the nearest newline on either side of the match.
    let mut line_start = win[..rel_start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut line_end = win[rel_end..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|i| rel_end + i)
        .unwrap_or(win.len());

    // Minified JS and other single-line files would otherwise carry megabytes
    // through the message channel per hit.
    if line_end - line_start > SNIPPET_MAX {
        let context = SNIPPET_MAX / 2;
        line_start = line_start.max(rel_start.saturating_sub(context));
        line_end = line_end.min(rel_end + context).max(rel_end);
    }

    let slice = &win[line_start..line_end];
    if slice.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(slice).trim().to_string())
}

/// Scans `file` for the first occurrence of any pattern in `matcher`.
///
/// `max_bytes` is the size cap, pass 0 to disable it. Reads in `CHUNK`
/// blocks with `max_pattern_len - 1` bytes of overlap between them so
/// matches straddling a chunk boundary are still caught. Returns the first
/// hit only, matching the one-row-per-file model of the content-search UI.
///
/// WARNING: binary detection is a NUL-byte sniff of the first BINARY_SNIFF
/// bytes only. Non-NUL binary that happens to be valid UTF-8 will be scanned.
pub fn scan_file(
    file: &File,
    matcher: &AhoCorasick,
    max_bytes: u64,
    should_stop: &dyn Fn() -> bool,
) -> io::Result<Option<Hit>> {
    let cap = if max_bytes == 0 { u64::MAX } else { max_bytes };

    let overlap = matcher.max_pattern_len().saturating_sub(1);
    let mut buf = vec![0u8; overlap + CHUNK];
    let mut reader = file;

    let mut carry = 0usize;
    let mut base_off: u64 = 0;
    let mut newlines_before: usize = 0;
    let mut total_read: u64 = 0;
    let mut first = true;

    loop {
        if should_stop() {
            return Ok(None);
        }

        let n = read_full(&mut reader, &mut buf[carry..carry + CHUNK])?;
        if n == 0 {
            return Ok(None);
        }
        if first {
            first = false;
            if buf[..n.min(BINARY_SNIFF)].contains(&0) {
                return Ok(None);
            }
        }

        let filled = carry + n;
        if let Some(m) = matcher.find(&buf[..filled]) {
            let line_number = newlines_before + count_newlines(&buf[..m.start()]) + 1;
            let abs_start = base_off + m.start() as u64;
            let abs_end = base_off + m.end() as u64;
            return Ok(snippet(file, abs_start, abs_end).map(|line| Hit { line_number, line }));
        }

        total_read += n as u64;
        if n < CHUNK || total_read >= cap {
            return Ok(None);
        }

        let keep = overlap.min(filled);
        let discard = filled - keep;
        newlines_before += count_newlines(&buf[..discard]);
        buf.copy_within(discard..filled, 0);
        base_off += discard as u64;
        carry = keep;
    }
}
