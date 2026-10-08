mod cache;
mod resolve;
mod theme;
mod xdg;

pub use cache::invalidate_themed_icon_cache;
pub use resolve::get_icon_for_path;

#[allow(unused_imports)]
pub use theme::resolve_folder_icon_with_fallbacks;
#[allow(unused_imports)]
pub use xdg::get_default_xdg_folder_icon;
