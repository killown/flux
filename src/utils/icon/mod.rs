mod cache;
mod preload;
mod resolve;
mod theme;
mod xdg;

pub use cache::{
    begin_file_icon_decode, end_file_icon_decode, insert_cached_file_icon_texture,
    invalidate_themed_icon_cache, peek_cached_file_icon_texture,
};
pub use preload::preload_icon_theme;
pub use resolve::{get_icon_for_network_file, get_icon_for_path, get_icon_for_virtual_dir};

#[allow(unused_imports)]
pub use theme::resolve_folder_icon_with_fallbacks;
#[allow(unused_imports)]
pub use xdg::get_default_xdg_folder_icon;
