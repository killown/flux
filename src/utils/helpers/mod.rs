//! Shared UI helpers split by concern and re-exported here.

mod background;
mod format;
mod system;
mod theme;
#[allow(unused_imports)]
pub use crate::app::update::helpers::files;
pub use crate::app::update::helpers::input;
#[allow(unused_imports)]
pub use crate::app::update::helpers::selection;
#[allow(unused_imports)]
pub use crate::app::update::helpers::sidebar;
#[allow(unused_imports)]
pub use crate::app::update::helpers::view;

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

/// Widget name of the overlay that parents the context-menu popover.
pub const GRID_OVERLAY_NAME: &str = "flux-grid-overlay";

/// Depth-first lookup of a descendant (or `root` itself) by widget name.
pub fn find_widget_by_name(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    crate::hit!("find_widget_by_name");
    use gtk::prelude::*;
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(found) = find_widget_by_name(&c, name) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// Maps `(x, y)` from `from` into the popover parent's space, falling back to the input.
pub fn to_popover_space(from: &gtk::Widget, x: f64, y: f64) -> (f64, f64) {
    use gtk::prelude::*;
    from.root()
        .and_then(|r| find_widget_by_name(r.upcast_ref(), GRID_OVERLAY_NAME))
        .and_then(|overlay| translate_point(from, &overlay, x, y))
        .unwrap_or((x, y))
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
