//! The panel's widget handle, plus the small operations that reflect app state
//! onto the header strip and fill the Details tab.

use super::content::FileInfo;
use super::state::{Mode, State};
use super::widgets::{clear_list, first_available_icon, push_row};
use crate::i18n::tr;
use crate::ui::FileItem;
use adw::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct PreviewPanelHandle {
    pub(crate) stack: gtk::Stack,
    pub(crate) nav_box: gtk::Box,
    pub(crate) page_spin: gtk::SpinButton,
    pub(crate) page_total: gtk::Label,
    pub(crate) prev_btn: gtk::Button,
    pub(crate) next_btn: gtk::Button,
    pub(crate) wrap_btn: gtk::ToggleButton,
    pub(crate) invert_btn: gtk::ToggleButton,
    pub(crate) md_source_btn: gtk::ToggleButton,
    pub(crate) wide_btn: gtk::ToggleButton,
    pub(crate) fullscreen_btn: gtk::Button,
    pub(crate) mute_btn: gtk::ToggleButton,
    pub(crate) header: gtk::Box,
    pub(crate) folder_grid: Rc<RefCell<TypedGridView<FileItem, gtk::MultiSelection>>>,
    pub(crate) mime_filter_btn: gtk::ToggleButton,
    pub(crate) playlist_btn: gtk::ToggleButton,
    pub(crate) install_font_btn: gtk::Button,
    pub(crate) zoom_btn: gtk::Button,
    pub(crate) banner: adw::Banner,
    pub(crate) image: gtk::Picture,
    pub(crate) image_scroller: gtk::ScrolledWindow,
    pub(crate) text_view: gtk::TextView,
    pub(crate) text_tag: gtk::TextTag,
    pub(crate) md_base_tag: gtk::TextTag,
    pub(crate) pdf_scroller: gtk::ScrolledWindow,
    pub(crate) pdf_picture: gtk::Picture,
    pub(crate) table_view: gtk::ColumnView,
    pub(crate) meta_list: gtk::ListBox,
    pub(crate) nopreview: adw::StatusPage,
    pub(crate) footer: gtk::Box,
    pub(crate) switcher: adw::ViewSwitcher,
    pub(crate) view_stack: adw::ViewStack,
    pub(crate) details_list: gtk::ListBox,
    pub(crate) media_video: gtk::Video,
    pub(crate) media_cover: gtk::Picture,
    pub(crate) audio_art_stack: gtk::Stack,
    pub(crate) audio_status: gtk::Label,
    pub(crate) audio_btn: gtk::Button,
    pub(crate) audio_back: gtk::Button,
    pub(crate) audio_fwd: gtk::Button,
    pub(crate) audio_seek: gtk::Scale,
    pub(crate) audio_vol: gtk::Scale,
    pub(crate) audio_time: gtk::Label,
    pub(crate) audio_dur: gtk::Label,
    pub(crate) state: Rc<State>,
}

impl PreviewPanelHandle {
    /// Reflects `config.ui.wide_preview` on the header button without echoing a message back.
    pub fn set_wide_default_active(&self, on: bool) {
        if self.wide_btn.is_active() != on {
            self.wide_btn.set_active(on);
        }
    }

    /// Reflects full-screen preview on the header button (icon and tooltip).
    pub fn set_fullscreen_active(&self, on: bool) {
        // Full screen shows only the content, Esc or double-click bring the strip back.
        self.header.set_visible(!on);
        let icon = if on {
            first_available_icon(&["view-restore-symbolic", "view-fullscreen-symbolic"])
        } else {
            first_available_icon(&["view-fullscreen-symbolic", "zoom-fit-best-symbolic"])
        };
        self.fullscreen_btn.set_icon_name(&icon);
        self.fullscreen_btn.set_tooltip_text(Some(&if on {
            tr("Exit full screen (Esc)")
        } else {
            tr("Full screen")
        }));
    }

    /// Reflects the ephemeral MIME filter state on the header button without
    /// echoing `FilterBySelectedMime` back into `FluxApp`.
    pub fn set_mime_filter_active(&self, on: bool) {
        if self.mime_filter_btn.is_active() == on {
            return;
        }
        self.state.mime_filter_suppress.set(true);
        self.mime_filter_btn.set_active(on);
        self.state.mime_filter_suppress.set(false);
    }

    pub(super) fn set_mode(&self, mode: Mode) {
        self.state.mode.set(mode);
        self.nav_box.set_visible(mode == Mode::Pdf);
        self.invert_btn.set_visible(mode == Mode::Pdf);
        self.wrap_btn
            .set_visible(mode == Mode::Text || mode == Mode::Markdown);
        self.md_source_btn.set_visible(mode == Mode::Markdown);
        self.install_font_btn.set_visible(false);
    }

    pub(super) fn show_info(&self, info: &FileInfo) {
        crate::hit!("show_info");
        clear_list(&self.details_list);
        push_row(&self.details_list, &tr("Name"), &info.name);
        push_row(&self.details_list, &tr("Type"), &info.mime);
        let virtual_path = info
            .path
            .to_string_lossy()
            .starts_with(crate::infra::archive::ARCHIVE_URI);
        if info.size > 0 || !virtual_path {
            push_row(
                &self.details_list,
                &tr("Size"),
                &format!("{} ({} B)", glib::format_size(info.size), info.size),
            );
        }
        if info.mtime > 0 {
            push_row(
                &self.details_list,
                &tr("Modified"),
                &glib::DateTime::from_unix_local(info.mtime)
                    .ok()
                    .and_then(|d| d.format("%Y-%m-%d %H:%M").ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
            );
        }
        for (k, v) in &info.extra {
            push_row(&self.details_list, k, v);
        }
        if let Some(dir) = info.path.parent() {
            push_row(&self.details_list, &tr("Location"), &dir.to_string_lossy());
        }

        *self.state.path.borrow_mut() = Some(info.path.clone());
        self.footer.set_visible(true);
        self.switcher.set_visible(true);
    }
}
