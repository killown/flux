use oxipng::{Options, StripChunks};

/// Optimizes raw PNG bytes in-place using oxipng.
pub fn optimize_png_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut opts = Options::from_preset(2);
    opts.strip = StripChunks::All;

    oxipng::optimize_from_memory(bytes, &opts).unwrap_or_else(|_| bytes.to_vec())
}
