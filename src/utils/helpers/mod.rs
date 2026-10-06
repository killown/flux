//! Shared UI helpers split by concern and re-exported here.

mod background;
mod files;
mod format;
mod input;
mod selection;
mod sidebar;
mod system;
mod theme;
mod view;

#[allow(unused_imports)]
pub use background::{clear_custom_background_images, load_custom_background_images};
#[allow(unused_imports)]
pub use format::{format_display_label, format_right_status, is_recursive_paste};
#[allow(unused_imports)]
pub use input::parse_mouse_button;
#[allow(unused_imports)]
pub use system::{launch_editor_at_line, open_new_instance, register_resources};
#[allow(unused_imports)]
pub use theme::{apply_ui_scale, list_available_themes, load_custom_css};
