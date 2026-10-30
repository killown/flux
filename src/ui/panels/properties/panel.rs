//! Right-side file Properties panel.
//!
//! The widget tree is built once (lazily, by `toggle_sidebar_right_panel`) and
//! refilled on every load: the metadata is gathered on a blocking thread, then
//! [`PropertiesMsg::FilePropertiesReady`] hands it back to [`show_properties`].

use super::section::MetadataSection;
use super::sections::build_sections;
use crate::domain::model::{AppMsg, PropertiesMsg, WindowMsg, FluxApp, RightPanelType};
use crate::i18n::tr;
use crate::ui::panels::header::panel_header;
use crate::ui::panels::resize::resizable_panel;
use crate::ui::panels::spec::PanelSpec;
use adw::gio;
use adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Grace period before a load starts reading the file. Arrow-key scrubbing
/// emits one `SelectionChanged` per row, so only the selection the user
/// settles on is worth probing.
const LOAD_DEBOUNCE: Duration = Duration::from_millis(120);

/// A section title plus its `(label, value)` rows, as carried by
/// [`PropertiesMsg::FilePropertiesReady`].
type Sections = Vec<(String, Vec<(String, String)>)>;

/// Live widgets of the built panel, reachable from the `FluxApp` handlers.
struct Handles {
    /// Switches between the `"empty"` and `"content"` pages.
    stack: gtk::Stack,
    /// Section groups, the factory must outlive its widgets to keep forwarding copies.
    sections: FactoryVecDeque<MetadataSection>,
    /// Wraps `open_with_row`, hidden for anything but a regular file.
    open_with_group: adw::PreferencesGroup,
    /// Names the default application of the file, activating it opens the Open With dialog.
    open_with_row: adw::ActionRow,
}

thread_local! {
    /// The single panel instance. GTK objects are main-thread only, so no lock is needed.
    static HANDLES: RefCell<Option<Handles>> = const { RefCell::new(None) };
}

fn with_handles<R>(f: impl FnOnce(&mut Handles) -> R) -> Option<R> {
    HANDLES.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Gathers everything the panel shows for `path`. Blocking: call off the main thread.
///
/// Returns the metadata sections and, for a regular file, the name of its
/// default application (`"None"` when it has none). Only regular files are
/// read, so a FIFO or device node cannot hang the worker on `open`.
pub(crate) fn collect_properties(path: &Path) -> (Sections, Option<String>) {
    crate::hit!("collect_properties");
    let mime = crate::utils::media::get_mime_type(path);

    let mut content = Vec::new();
    if path.is_file() {
        if let Ok(f) = fs::File::open(path) {
            let _ = f.take(1024 * 1024).read_to_end(&mut content);
        }
    }

    let open_with = path.is_file().then(|| {
        gio::AppInfo::default_for_type(&mime, false)
            .map_or_else(|| tr("None"), |app| app.name().to_string())
    });

    (build_sections(path, &mime, &content), open_with)
}

/// Builds the Properties panel widget tree.
///
/// Registers the live widgets in a thread-local so [`show_properties`] and
/// [`show_empty`] can fill it without threading handles through `FluxApp`.
pub fn build_properties_panel(
    initial_width: i32,
    sender: AsyncComponentSender<FluxApp>,
) -> gtk::Box {
    crate::hit!("build_properties_panel");
    let spec = PanelSpec::properties().with_initial(initial_width);

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
            Some("document-properties-symbolic"),
            &tr("Properties"),
            &[],
            move || s.input(AppMsg::Properties(PropertiesMsg::ToggleFilePropertiesPanel)),
        );
        panel.append(&header);
    }

    // ── Metadata sections ───────────────────────────────────────────────
    let sections = FactoryVecDeque::<MetadataSection>::builder()
        .launch(gtk::Box::new(gtk::Orientation::Vertical, 18))
        .forward(sender.input_sender(), |v| AppMsg::Properties(PropertiesMsg::CopyPropertyValue(v)));

    // ── "Open With" row ─────────────────────────────────────────────────
    let open_with_row = adw::ActionRow::builder()
        .title(tr("Open With"))
        .activatable(true)
        .build();
    open_with_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    {
        let s = sender.clone();
        open_with_row.connect_activated(move |_| s.input(AppMsg::Properties(PropertiesMsg::OpenPropertiesTargetWith)));
    }
    let open_with_group = adw::PreferencesGroup::new();
    open_with_group.add(&open_with_row);
    open_with_group.set_visible(false);

    // ── Layout ──────────────────────────────────────────────────────────
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(12)
        .build();
    content.append(&open_with_group);
    content.append(sections.widget());

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();

    let empty = adw::StatusPage::builder()
        .icon_name("document-properties-symbolic")
        .title(tr("No file selected"))
        .vexpand(true)
        .build();

    let stack = gtk::Stack::new();
    stack.add_named(&empty, Some("empty"));
    stack.add_named(&scroller, Some("content"));
    stack.set_visible_child_name("empty");
    panel.append(&stack);

    HANDLES.with(|h| {
        *h.borrow_mut() = Some(Handles {
            stack,
            sections,
            open_with_group,
            open_with_row,
        });
    });

    resizable_panel(&spec, &panel, |_| {})
}

/// Empties the panel and shows the placeholder page.
pub(crate) fn show_empty() {
    with_handles(|h| {
        h.sections.guard().clear();
        h.open_with_group.set_visible(false);
        h.stack.set_visible_child_name("empty");
    });
}

/// Fills the panel with a finished load, `open_with` is the default application
/// name, or `None` to hide the "Open With" row.
pub(crate) fn show_properties(sections: Sections, open_with: Option<String>) {
    crate::hit!("show_properties");
    with_handles(|h| {
        {
            let mut guard = h.sections.guard();
            guard.clear();
            for (title, items) in sections {
                guard.push_back((title, items));
            }
        }

        h.open_with_group.set_visible(open_with.is_some());
        if let Some(app) = open_with {
            h.open_with_row.set_subtitle(&app);
        }

        h.stack.set_visible_child_name("content");
    });
}

impl FluxApp {
    /// Bumps the panel session and starts a background load for `properties_target`.
    ///
    /// Called when the panel opens and on every `SelectionChanged` while it is
    /// visible. Virtual locations (`/archive://`, `trash:///`, `tags://`,
    /// `network:///`) have no on-disk metadata and show the placeholder.
    pub(crate) fn request_properties_panel_load(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("request_properties_panel_load");
        let session = self.properties_panel_session.fetch_add(1, Ordering::SeqCst) + 1;

        let Some(path) = self.properties_target.clone() else {
            show_empty();
            return;
        };
        if path.to_string_lossy().contains("://") {
            show_empty();
            return;
        }

        let live = self.properties_panel_session.clone();
        let s = sender.clone();
        relm4::spawn_blocking(move || {
            std::thread::sleep(LOAD_DEBOUNCE);
            if live.load(Ordering::SeqCst) != session {
                return;
            }
            let (sections, open_with) = collect_properties(&path);
            s.input(AppMsg::Properties(PropertiesMsg::FilePropertiesReady {
                session,
                sections,
                open_with,
            }));
        });
    }

    /// Invalidates in-flight loads and empties the panel, called whenever it is hidden.
    pub(crate) fn reset_properties_panel(&mut self) {
        self.properties_panel_session.fetch_add(1, Ordering::SeqCst);
        self.properties_target = None;
        show_empty();
    }

    /// Applies a finished load unless a newer one was requested or the panel closed.
    pub fn handle_file_properties_ready(
        &mut self,
        session: u64,
        sections: Sections,
        open_with: Option<String>,
    ) {
        crate::hit!("handle_file_properties_ready");
        if session != self.properties_panel_session.load(Ordering::SeqCst) {
            return;
        }
        if !self.properties_panel_visible {
            return;
        }
        show_properties(sections, open_with);
    }

    /// Opens the panel on `path`, or retargets it when it is already open.
    pub fn handle_open_file_properties(
        &mut self,
        path: std::path::PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_open_file_properties");
        self.properties_target = Some(path);
        if self.properties_panel_visible {
            self.request_properties_panel_load(sender);
        } else {
            self.toggle_sidebar_right_panel(RightPanelType::Properties, sender);
        }
    }

    /// Opens the Open With dialog for the file the Properties panel is showing.
    pub fn handle_open_properties_target_with(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_open_properties_target_with");
        if let Some(path) = self.properties_target.clone() {
            self.show_open_with_dialog(path, sender);
        }
    }

    /// Copies a Properties row value to the clipboard and confirms with a toast.
    pub fn handle_copy_property_value(
        &mut self,
        text: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        if let Some(display) = adw::gdk::Display::default() {
            display.clipboard().set_text(&text);
            sender.input(AppMsg::Window(WindowMsg::ShowToast(tr("Copied to clipboard"))));
        }
    }
}
