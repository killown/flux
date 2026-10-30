//! Contextual Actions panel: the secondary (MIME-driven) menu templates shown as rows.
//!
//! The data comes from `~/.config/flux/menus/menu-*.rs` through the same
//! `resolve_secondary_menu_template` + `parse_secondary_template` pair the
//! right-click popover uses. The panel is a UI surface only: it adds no URI
//! scheme, no loader and no grid items.
//!
//! MIME resolution and template parsing run on a blocking worker guarded by
//! `FluxApp::actions_panel_session`, the same way the peek-preview panel guards
//! its loads.

use super::actions_registry;
use super::header::panel_header;
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::domain::model::{ActionsMsg, AppMsg, CustomAction, FluxApp, WindowMsg};
use crate::i18n::tr;
use crate::ui::constants;
use crate::ui::menu::secondary::action_matches_mime;
use crate::utils::search::fuzzy_match_compiled;
use adw::prelude::*;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use relm4::AsyncComponentSender;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

/// Widget-data key holding a row's search text.
const SEARCH_META_KEY: &str = "actions-search-meta";

/// Builds the contextual-actions panel and registers its widgets in [`actions_registry`].
pub fn build_actions_panel(initial_width: i32, sender: AsyncComponentSender<FluxApp>) -> gtk::Box {
    crate::hit!("build_actions_panel");
    let spec = PanelSpec::actions().with_initial(initial_width);

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    {
        let edit_btn = gtk::Button::builder()
            .icon_name("document-edit-symbolic")
            .css_classes(["flat", "circular"])
            .valign(gtk::Align::Center)
            .tooltip_text(tr("Edit actions template for this type"))
            .build();
        {
            let s = sender.clone();
            edit_btn.connect_clicked(move |btn| {
                let mime = actions_registry::mime();
                // Mirror append_secondary_action's target selection.
                let base_dir = dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("~/.config"))
                    .join("flux");
                let menus_dir = base_dir.join("menus");
                let filename =
                    crate::ui::menu::secondary::secondary_template_filename(&mime, false);
                let path = [&menus_dir, &base_dir]
                    .into_iter()
                    .map(|d| d.join(&filename))
                    .find(|p| p.is_file())
                    .unwrap_or_else(|| menus_dir.join(&filename));

                let parent = btn.root().and_downcast::<gtk::Window>();
                crate::ui::dialog::template_editor::show_template_editor(
                    parent.as_ref(),
                    path,
                    s.clone(),
                );
            });
        }

        let add_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .css_classes(["flat", "circular"])
            .valign(gtk::Align::Center)
            .tooltip_text(tr("New Action"))
            .build();
        let s = sender.clone();
        add_btn.connect_clicked(move |_| {
            s.input(AppMsg::Actions(ActionsMsg::PromptNewContextualAction {
                mime: actions_registry::mime(),
            }));
        });

        let s = sender.clone();
        let header = panel_header(
            Some("system-run-symbolic"),
            &tr("Contextual Actions"),
            &[edit_btn.upcast(), add_btn.upcast()],
            move || s.input(AppMsg::Actions(ActionsMsg::ToggleActionsPanel)),
        );
        panel.append(&header);
    }

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Filter actions…"))
        .hexpand(true)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(8)
        .build();
    panel.append(&search_entry);

    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .margin_end(4)
        .build();

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .overlay_scrolling(false)
        .propagate_natural_width(false)
        .hexpand(false)
        .vexpand(true)
        .margin_start(12)
        .margin_end(8)
        .margin_bottom(12)
        .child(&list_box)
        .build();

    // The description is Pango markup, so the literal `<category>` hint must be escaped.
    let empty_hint = gtk::glib::markup_escape_text(&tr(
        "Create ~/.config/flux/menus/menu-<category>-<subtype>.rs to add actions for this file type.",
    ));
    let empty_page = adw::StatusPage::builder()
        .icon_name("system-run-symbolic")
        .title(tr("No contextual actions"))
        .description(empty_hint.as_str())
        .vexpand(true)
        .build();
    // The empty page is where authoring matters most, so it carries its own button.
    let empty_add_btn = gtk::Button::builder()
        .label(tr("New Action"))
        .halign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .build();
    {
        let s = sender.clone();
        empty_add_btn.connect_clicked(move |_| {
            s.input(AppMsg::Actions(ActionsMsg::PromptNewContextualAction {
                mime: actions_registry::mime(),
            }));
        });
    }
    empty_page.set_child(Some(&empty_add_btn));

    let stack = gtk::Stack::builder().vexpand(true).build();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(&empty_page, Some("empty"));
    stack.set_visible_child_name("empty");
    panel.append(&stack);

    {
        let list_box = list_box.clone();
        search_entry.connect_search_changed(move |entry| {
            apply_filter(&list_box, &entry.text());
        });
    }

    actions_registry::set_current(list_box, search_entry, stack);

    let sender_for_resize = sender.clone();
    resizable_panel(&spec, &panel, move |w| {
        sender_for_resize.input(AppMsg::Actions(ActionsMsg::SetActionsPanelWidth(w)));
    })
}

/// Shows only the rows whose search text fuzzy-matches `query`.
///
/// Every whitespace-separated word of the query has to match somewhere in the
/// row's label, submenu, MIME types or command, so `convert mp4` finds
/// `Media Convert > To MP4`.
fn apply_filter(list: &gtk::ListBox, query: &str) {
    crate::hit!("apply_filter");
    let query = query.trim();
    let pattern = (!query.is_empty())
        .then(|| Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart));
    let mut child = list.first_child();
    while let Some(row) = child {
        let visible = match &pattern {
            None => true,
            // SAFETY: the key is only ever written by `build_row` with a `String`.
            Some(p) => unsafe {
                row.data::<String>(SEARCH_META_KEY)
                    .map(|meta| fuzzy_match_compiled(meta.as_ref(), p))
            }
            .unwrap_or(false),
        };
        row.set_visible(visible);
        child = row.next_sibling();
    }
}

/// Splits a label like `"󰈙 Convert to PDF"` into its icon glyph prefix and its text.
///
/// The glyph ends at the first alphanumeric character, `(` or `>`. A label with
/// no glyph yields an empty prefix.
fn split_glyph(label: &str) -> (&str, &str) {
    let idx = label
        .char_indices()
        .find(|(_, c)| c.is_alphanumeric() || *c == '(' || *c == '>')
        .map(|(i, _)| i)
        .unwrap_or(0);
    (label[..idx].trim(), label[idx..].trim())
}

/// Builds one `adw::ActionRow` for `action`, its run button dispatches `ExecuteSecondaryAction`.
fn build_row(action: &CustomAction, sender: &AsyncComponentSender<FluxApp>) -> adw::ActionRow {
    crate::hit!("build_row");
    let (glyph, text) = split_glyph(&action.label);

    // Commands contain `<`, `&` and `%p`, none of which may be read as Pango markup.
    let row = adw::ActionRow::builder()
        .title(text)
        .subtitle(action.command.as_str())
        .subtitle_lines(1)
        .subtitle_selectable(true)
        .use_markup(false)
        .build();

    if let Some(sub) = &action.submenu {
        row.add_prefix(
            &gtk::Label::builder()
                .label(format!("{sub} ›"))
                .css_classes(["caption", "dim-label"])
                .valign(gtk::Align::Center)
                .build(),
        );
    }

    // Glyphs are Nerd Font characters, which render as tofu without one installed.
    if glyph.is_empty() || !crate::infra::git::is_nerd_font_available() {
        row.add_prefix(&gtk::Image::from_icon_name("system-run-symbolic"));
    } else {
        row.add_prefix(
            &gtk::Label::builder()
                .label(glyph)
                .css_classes(["flux-action-glyph"])
                .valign(gtk::Align::Center)
                .build(),
        );
    }

    let run_btn = gtk::Button::builder()
        .icon_name("system-run-symbolic")
        .css_classes(["flux-action-run-btn"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Run"))
        .build();
    {
        let s = sender.clone();
        let cmd = action.command.clone();
        let toast = action.toast.clone();
        let no_command_dialog = action.no_command_dialog;
        run_btn.connect_clicked(move |_| {
            s.input(AppMsg::Actions(ActionsMsg::ExecuteSecondaryAction {
                command: cmd.clone(),
                no_command_dialog,
            }));
            if let Some(ref msg) = toast {
                s.input(AppMsg::Window(WindowMsg::ShowToast(msg.clone())));
            }
        });
    }
    row.add_suffix(&run_btn);

    let meta = format!(
        "{} {} {} {}",
        action.submenu.as_deref().unwrap_or_default(),
        action.label,
        action.mime_types.join(", "),
        action.command,
    );
    // SAFETY: read back only by `apply_filter`, which asks for the same `String` type.
    unsafe {
        row.set_data(SEARCH_META_KEY, meta);
    }

    row
}

impl FluxApp {
    /// Bumps the panel session and asks for a refresh for the current selection.
    ///
    /// Called when the panel opens and on every `SelectionChanged` while it is visible.
    pub(crate) fn request_actions_panel_load(&self, sender: &AsyncComponentSender<Self>) {
        let session = self.actions_panel_session.fetch_add(1, Ordering::SeqCst) + 1;
        sender.input(AppMsg::Actions(ActionsMsg::ActionsPanelLoad { session }));
    }

    /// Invalidates in-flight loads and empties the panel, called whenever it is hidden.
    ///
    /// The widget tree is reused on the next open, so the registry handles stay in place.
    /// The cached template path is dropped too: the Edit button in the panel header
    /// would otherwise still open the previous selection's file when the panel is
    /// reopened on a different MIME type before the first async load lands.
    pub(crate) fn reset_actions_panel(&self) {
        self.actions_panel_session.fetch_add(1, Ordering::SeqCst);
        if let Some((list, _search, stack)) = actions_registry::with(Clone::clone) {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            stack.set_visible_child_name("empty");
        }
        actions_registry::set_template_path(None);
    }

    /// Resolves the selection's MIME type and template off the main thread.
    ///
    /// Also points `active_item_path` at the selection, because `ExecuteCommand`
    /// resolves its targets from it and a stale right-click target would otherwise win.
    pub fn handle_actions_panel_refresh(&mut self, session: u64) {
        crate::hit!("handle_actions_panel_refresh");
        if session != self.actions_panel_session.load(Ordering::SeqCst) {
            return;
        }
        if !self.actions_panel_visible {
            return;
        }

        // `None` (nothing selected) is intentional: `resolve_command_targets` then
        // falls back to `current_path`, which is what `builtin::new_folder` needs.
        let path = self.get_selected_path();
        self.active_item_path = path.clone();
        // No line context here, so `%l` actions are filtered out below.
        // Safe to clear: the context menus recompute this from the item's
        // `line_number` on every right-click, so wiping it does not affect them.
        self.active_item_line = 0;

        let session_arc = self.actions_panel_session.clone();
        let sender = crate::domain::model::SENDER.get().cloned();
        relm4::spawn_blocking(move || {
            if session_arc.load(Ordering::SeqCst) != session {
                return;
            }
            // Empty selection means the current directory, which is how
            // `builtin::new_folder` / `builtin::new_file` from `menu-all.rs` show up.
            let mime = path
                .as_ref()
                .map(|p| crate::utils::media::get_mime_type(p))
                .unwrap_or_else(|| constants::MIME_DIR.to_string());
            let template_path = crate::ui::menu::secondary::resolve_secondary_menu_template(&mime);
            let actions: Vec<CustomAction> = template_path
                .as_deref()
                .map(crate::ui::menu::secondary::parse_secondary_template)
                .unwrap_or_default()
                .into_iter()
                .filter(|a| action_matches_mime(a, &mime) && !a.command.contains("%l"))
                .collect();
            if session_arc.load(Ordering::SeqCst) != session {
                return;
            }
            if let Some(s) = sender {
                let _ = s.send(AppMsg::Actions(ActionsMsg::ActionsPanelReady {
                    session,
                    mime,
                    actions,
                    template_path,
                }));
            }
        });
    }

    /// Rebuilds the rows from a resolved action list. Stale sessions are dropped first.
    pub fn handle_actions_panel_ready(
        &self,
        session: u64,
        mime: String,
        actions: Vec<CustomAction>,
        template_path: Option<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_actions_panel_ready");
        if session != self.actions_panel_session.load(Ordering::SeqCst) {
            return;
        }
        if !self.actions_panel_visible {
            return;
        }
        let Some((list, search, stack)) = actions_registry::with(Clone::clone) else {
            return;
        };
        // The "+" buttons read this back to pre-fill the dialog, the Edit
        // button reads the template path to open the exact file the current
        // list came from. Both survive until the next load or panel reset.
        actions_registry::set_mime(&mime);
        actions_registry::set_template_path(template_path);

        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        // Nothing to filter on the empty page, so the entry is disabled there.
        search.set_sensitive(!actions.is_empty());
        if actions.is_empty() {
            stack.set_visible_child_name("empty");
            return;
        }

        for action in &actions {
            list.append(&build_row(action, sender));
        }
        // A filter typed before this refresh still applies to the new rows.
        apply_filter(&list, &search.text());
        stack.set_visible_child_name("list");
    }

    /// Reloads the panel after its template was edited and saved on disk.
    ///
    /// Reuses the regular refresh path so the new rows show up without the
    /// user having to touch the selection.
    pub fn handle_reload_actions_template(
        &self,
        _path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_reload_actions_template");
        if self.actions_panel_visible {
            self.request_actions_panel_load(sender);
        }
    }

    /// Appends the new action to its template file on a blocking worker.
    ///
    /// The result comes back as `ContextualActionSaved`, which reloads the panel.
    pub fn handle_save_contextual_action(
        &self,
        mime: String,
        category_scope: bool,
        action: crate::ui::menu::secondary::NewSecondaryAction,
    ) {
        crate::hit!("handle_save_contextual_action");
        let sender = crate::domain::model::SENDER.get().cloned();
        relm4::spawn_blocking(move || {
            let result =
                crate::ui::menu::secondary::append_secondary_action(&mime, category_scope, &action);
            if let Some(s) = sender {
                let _ = s.send(AppMsg::Actions(ActionsMsg::ContextualActionSaved(result)));
            }
        });
    }

    /// Reports the outcome and, on success, reloads the panel so the new row shows at once.
    pub fn handle_contextual_action_saved(
        &self,
        result: Result<PathBuf, String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_contextual_action_saved");
        match result {
            Ok(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                    "{} {}",
                    tr("Action saved to"),
                    name
                ))));
                if self.actions_panel_visible {
                    self.request_actions_panel_load(sender);
                }
            }
            Err(e) => sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                "{}: {e}",
                tr("Could not save action")
            )))),
        }
    }
}
