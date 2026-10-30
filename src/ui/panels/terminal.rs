use super::header::panel_header;
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::domain::model::{AppMsg, TerminalMsg, FluxApp};
use crate::i18n::tr;
use adw::prelude::*;
use relm4::AsyncComponentSender;

/// Builds the sidebar terminal panel widget tree.
///
/// The caller owns the returned `Terminal` and is responsible for spawning
/// the shell and shutting it down. Spawning lives in the panel toggle
/// handler, not here, so that closing and reopening the panel respawns a
/// fresh shell instead of reusing a dead PTY.
pub fn build_terminal_panel(
    config: &crate::domain::model::TerminalConfig,
    initial_width: i32,
    sender: AsyncComponentSender<FluxApp>,
) -> (gtk::Box, crate::infra::terminal::Terminal) {
    crate::hit!("build_terminal_panel");
    let spec = PanelSpec::new(380, 800, 720).with_initial(initial_width);

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    {
        let s = sender.clone();
        let header = panel_header(
            Some("utilities-terminal-symbolic"),
            &tr("Terminal"),
            &[],
            move || s.input(AppMsg::Terminal(TerminalMsg::ToggleTerminalPanel)),
        );
        panel.append(&header);
    }

    let term = crate::infra::terminal::Terminal::new(config);
    term.drawing_area.add_css_class("flux-terminal-area");
    panel.append(&term.drawing_area);

    let root = resizable_panel(&spec, &panel, |_| {});
    (root, term)
}
