mod audio;
mod exe;
mod font;
mod format;
mod image;
mod mime;
mod pdf;
mod png;
mod thumbnail;
mod video;

// Public API surface (preserves original `pub` items).
pub use exe::extract_exe_icon;
pub use font::font_thumbnail;
pub use format::{aspect_ratio_label, format_duration};
pub use image::probe_image_dimensions;
pub use mime::{
    container_mime_masks_extension, get_mime_type, guess_mime_from_extension, icon_gen_params,
    is_audio_file, is_visual_media,
};
#[allow(unused_imports)]
pub use png::optimize_png_bytes;
pub use thumbnail::get_or_create_thumbnail;
pub use video::probe_media_duration;

// Crate-internal API surface (preserves original `pub(crate)` items).
// `audio_thumbnail` is `pub` in the original file, so re-export it as `pub`
// to keep the public path `crate::utils::media::audio_thumbnail` working.
pub use audio::audio_thumbnail;
pub(crate) use image::decode_image_thumbnail;
pub(crate) use thumbnail::generate_thumbnail_sync;
pub(crate) use video::extract_video_frame;
