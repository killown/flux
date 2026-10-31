//! GTK-free raster image carried by messages and preview payloads.
//!
//! Worker threads decode into this type and the UI layer turns it into a
//! `gdk::Texture` when it applies the message, so `domain` never names a
//! toolkit type.

use std::sync::Arc;

/// Decoded image pixels: 8 bits per channel, RGBA order, premultiplied alpha.
///
/// Cloning is cheap, the pixel buffer is shared.
#[derive(Clone)]
pub struct Image {
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
    /// Distance in bytes between the starts of two consecutive rows.
    pub stride: usize,
    /// `stride * height` bytes of pixel data.
    pub pixels: Arc<[u8]>,
}

impl std::fmt::Debug for Image {
    // Hand-written so a derived `Debug` on a message never dumps megabytes of pixels.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .finish_non_exhaustive()
    }
}
