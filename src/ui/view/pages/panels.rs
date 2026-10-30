use adw::prelude::*;

pub(in crate::ui::view) const PANEL_TRANSITION_MS: u32 = 200;

pub(in crate::ui::view) fn panel_revealer(duration_ms: Option<u32>) -> gtk::Revealer {
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideLeft);
    if let Some(ms) = duration_ms {
        revealer.set_transition_duration(ms);
    }
    revealer.set_reveal_child(false);
    revealer.set_visible(false);
    revealer
}
