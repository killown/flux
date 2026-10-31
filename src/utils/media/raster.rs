//! Bridges the GTK-free [`Image`] carried by messages to `gdk::Texture`.

use crate::domain::model::Image;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::sync::Arc;

/// Pixel layout every [`Image`] uses.
const FORMAT: gdk::MemoryFormat = gdk::MemoryFormat::R8g8b8a8Premultiplied;

/// Copies the pixels of `texture` into an [`Image`].
///
/// Safe to call on a worker thread: `gdk::Texture` is immutable and thread-safe.
pub fn image_from_texture(texture: &gdk::Texture) -> Image {
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(FORMAT);
    let (bytes, stride) = downloader.download_bytes();
    Image {
        width: texture.width(),
        height: texture.height(),
        stride,
        pixels: Arc::from(&*bytes),
    }
}

/// Builds a `gdk::Texture` over the pixels of `image`, sharing the buffer.
pub fn texture_from_image(image: &Image) -> gdk::Texture {
    let bytes = glib::Bytes::from_owned(image.pixels.clone());
    gdk::MemoryTexture::new(image.width, image.height, FORMAT, &bytes, image.stride).upcast()
}
