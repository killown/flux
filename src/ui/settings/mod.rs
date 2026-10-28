use crate::i18n::tr;
use crate::model::Config;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::cell::RefCell;

mod page;

thread_local! {
    static CURRENT: RefCell<glib::WeakRef<adw::PreferencesWindow>> =
        RefCell::new(glib::WeakRef::new());
}

pub struct SettingsWindow {
    config: Config,
}

#[relm4::component(pub)]
impl SimpleComponent for SettingsWindow {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        adw::PreferencesWindow {
            set_title: Some(&tr("Preferences")),
            set_default_size: (800, 700),
            set_modal: false,
            set_search_enabled: true,
            add_css_class: "settings-window",

            add: &page::appearance::build(&model.config),
            add: &page::thumbnails::build(&model.config),
            add: &page::behavior::build(&model.config),
            add: &page::terminal::build(&model.config),
            add: &page::system::build(&model.config),
            add: &page::shortcuts::build(&model.config),
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = crate::utils::load_config();
        let model = SettingsWindow { config };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Opens the preferences window as a non-modal window of the running app.
pub fn open(parent: Option<&gtk::Window>) {
    crate::hit!("open");
    // Only reuse a window that is still on screen. The Relm4 runtime can keep
    // a just-closed window alive for a moment, and presenting that dead
    // window would silently do nothing.
    if let Some(existing) = CURRENT
        .with(|c| c.borrow().upgrade())
        .filter(|w| w.is_visible())
    {
        existing.present();
        return;
    }

    let controller = SettingsWindow::builder().launch(()).detach();
    let window = controller.widget().clone();
    if let Some(parent) = parent {
        window.set_transient_for(Some(parent));
        // Full height of the main window, 75% of its width. Falls back to the
        // default size from `view!` if the parent has no size yet.
        let (w, h) = (parent.width(), parent.height());
        if w > 0 && h > 0 {
            window.set_default_size(w * 3 / 4, h);
        }
    }

    CURRENT.with(|c| c.borrow().set(Some(&window)));

    let keep_alive = RefCell::new(Some(controller));
    window.connect_destroy(move |_| {
        CURRENT.with(|c| c.borrow().set(None));
        keep_alive.borrow_mut().take();
    });
    window.present();
}
