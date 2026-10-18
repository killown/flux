mod appimage;
mod audio;
mod exe;
mod font;
mod format;
mod image;
mod mime;
mod pdf;
mod png;
mod raw;
mod thumbnail;
mod tiers;
mod video;

// Public API surface (preserves original `pub` items).
pub use appimage::{extract_appimage_icon, seven_zip_available};
pub use exe::extract_exe_icon;
pub use font::{font_specimen, font_thumbnail};
pub use format::{aspect_ratio_label, format_duration};
pub use image::probe_image_dimensions;
pub use mime::{
    container_mime_masks_extension, get_mime_type, guess_mime_from_extension, icon_gen_params,
    is_appimage, is_audio_file, is_raw_image_ext, is_visual_media,
};
#[allow(unused_imports)]
pub use png::optimize_png_bytes;
pub(crate) use raw::raw_thumbnail;
pub use raw::raw_thumbnailer_available;
pub use thumbnail::get_or_create_thumbnail;
#[allow(unused_imports)]
pub use tiers::{find_cached_thumbnail, thumbnail_lookup_tiers, tier_for_size};
pub use video::probe_media_duration;

// Crate-internal API surface (preserves original `pub(crate)` items).
pub use audio::audio_thumbnail;
pub(crate) use image::decode_image_thumbnail;
pub(crate) use thumbnail::generate_thumbnail_sync;
pub(crate) use video::extract_video_frame;
