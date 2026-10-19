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

/// Converts `(x, y)` from `from`'s coordinate space into `to`'s.
///
/// Replaces the deprecated `WidgetExt::translate_coordinates` (GTK 4.12) with
/// `WidgetExt::compute_point`, which also accounts for widget transforms.
pub fn translate_point(
    from: &impl gtk::prelude::IsA<gtk::Widget>,
    to: &impl gtk::prelude::IsA<gtk::Widget>,
    x: f64,
    y: f64,
) -> Option<(f64, f64)> {
    use gtk::prelude::WidgetExt;
    from.compute_point(to, &gtk::graphene::Point::new(x as f32, y as f32))
        .map(|p| (f64::from(p.x()), f64::from(p.y())))
}

#[allow(unused_imports)]
pub use background::{clear_custom_background_images, load_custom_background_images};
#[allow(unused_imports)]
pub use format::{format_display_label, format_right_status, is_recursive_paste};
#[allow(unused_imports)]
pub use input::{parse_mouse_button, should_trigger_preview};
#[allow(unused_imports)]
pub use system::{launch_editor_at_line, open_new_instance, register_resources};
#[allow(unused_imports)]
pub use theme::{apply_ui_scale, list_available_themes, load_custom_css};
