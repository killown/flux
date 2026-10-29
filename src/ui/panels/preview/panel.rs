//! Construction of the panel's widget tree and wiring of its signal handlers.

use super::audio::{ipc_send, AUDIO_SEEK_HOLD};
use super::controllers::{
    attach_context_menu, attach_menu_button, attach_pan, attach_zoom_scroll, double_click_reserved,
};
use super::folder::{
    attach_folder_zoom_scroll, build_folder_grid, FOLDER_GRID_ICON_SIZE, FOLDER_GRID_NAV_GUTTER,
};
use super::folder_thumbs::FolderThumbs;
use super::gutter::LineNumberGutter;
use super::handle::PreviewPanelHandle;
use super::media::PREVIEW_MUTED;
use super::pdf::PDF_GUTTER;
use super::registry;
use super::state::{Mode, State};
use super::text::body_font_family;
use super::widgets::{first_available_icon, flat_icon_button, info_clamp, install_css};
use super::zoom::ZOOM_STEP;
use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::panels::header::panel_header;
use crate::ui::panels::resize::resizable_panel;
use crate::ui::panels::spec::PanelSpec;
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::Ordering;

pub fn build_preview_panel(initial_width: i32, sender: AsyncComponentSender<FluxApp>) -> gtk::Box {
    crate::hit!("build_preview_panel");
    install_css();
    let spec = PanelSpec::preview().with_initial(initial_width);

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    // Header row. All controls are hoisted into `panel_header`'s `extra`
    // slot so the panel keeps a single HIG-style strip, matching Search /
    // Diff / Tag / Location / Connect.
    let nav_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .build();
    let prev_btn = flat_icon_button("go-previous-symbolic", &tr("Previous page"));
    let next_btn = flat_icon_button("go-next-symbolic", &tr("Next page"));
    let page_spin = gtk::SpinButton::with_range(1.0, 1.0, 1.0);
    page_spin.set_numeric(true);
    page_spin.set_width_chars(3);
    let page_total = gtk::Label::builder().css_classes(["dim-label"]).build();
    nav_box.append(&prev_btn);
    nav_box.append(&page_spin);
    nav_box.append(&page_total);
    nav_box.append(&next_btn);
    nav_box.set_visible(false);

    let wrap_btn = gtk::ToggleButton::builder()
        .icon_name("format-justify-fill-symbolic")
        .tooltip_text(tr("Wrap lines"))
        .css_classes(["flat"])
        .visible(false)
        .build();
    let invert_btn = gtk::ToggleButton::builder()
        .icon_name("weather-clear-night-symbolic")
        .tooltip_text(tr("Invert colors"))
        .css_classes(["flat"])
        .visible(false)
        .build();
    let md_source_btn = gtk::ToggleButton::builder()
        .icon_name("text-x-generic-symbolic")
        .tooltip_text(tr("Toggle Markdown source"))
        .css_classes(["flat"])
        .visible(false)
        .build();
    let install_font_btn =
        flat_icon_button("system-software-install-symbolic", &tr("Install font"));
    install_font_btn.set_visible(false);
    let zoom_out = flat_icon_button("zoom-out-symbolic", &tr("Zoom out"));
    let zoom_in = flat_icon_button("zoom-in-symbolic", &tr("Zoom in"));
    let zoom_btn = gtk::Button::builder()
        .label("100%")
        .tooltip_text(tr("Reset zoom"))
        .css_classes(["flat"])
        .build();

    let details_toggle = gtk::ToggleButton::builder()
        .icon_name("dialog-information-symbolic")
        .tooltip_text(tr("Show file details"))
        .css_classes(["flat"])
        .build();

    let wide_btn = gtk::ToggleButton::builder()
        .icon_name("view-dual-symbolic")
        .tooltip_text(tr("Always open preview wide (F11)"))
        .css_classes(["flat"])
        .active(crate::utils::load_config().ui.wide_preview)
        .build();

    let fullscreen_btn = flat_icon_button(
        &first_available_icon(&["view-fullscreen-symbolic", "zoom-fit-best-symbolic"]),
        &tr("Full screen"),
    );

    // Only shown while a video or audio file is previewed (see `apply`).
    let mute_btn = gtk::ToggleButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .tooltip_text(tr("Mute media (until the preview is closed)"))
        .css_classes(["flat"])
        .visible(false)
        .build();

    let mime_filter_btn = gtk::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(tr("Show only files of this type (Ctrl+M)"))
        .css_classes(["flat"])
        .build();

    // Only shown while a video or audio file is previewed (see `apply`).
    // Off shows "repeat" (media loops by default), on shows "play in sequence".
    let playlist_icon_off =
        first_available_icon(&["media-playlist-repeat-symbolic", "view-refresh-symbolic"]);
    let playlist_icon_on =
        first_available_icon(&["media-playlist-consecutive-symbolic", "view-list-symbolic"]);
    let playlist_btn = gtk::ToggleButton::builder()
        .icon_name(playlist_icon_off.as_str())
        .tooltip_text(tr("Playlist mode: play the next item when playback ends"))
        .css_classes(["flat"])
        .visible(false)
        .build();

    let spacer = gtk::Box::builder().hexpand(true).build();

    let banner = adw::Banner::builder().revealed(false).build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .hexpand(true)
        .build();

    let empty = adw::StatusPage::builder()
        .icon_name("eye-open-symbolic")
        .title(tr("No preview"))
        .description(tr("Select an item to preview."))
        .build();
    stack.add_named(&empty, Some("empty"));

    let loading = gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    stack.add_named(&loading, Some("loading"));

    let nopreview = adw::StatusPage::builder()
        .icon_name("text-x-generic-symbolic")
        .title(tr("No preview available"))
        .build();
    stack.add_named(&nopreview, Some("nopreview"));

    let media_video = gtk::Video::builder()
        .vexpand(true)
        .hexpand(true)
        .loop_(true)
        .autoplay(true)
        .build();
    // gtk::Video advertises the clip's intrinsic size as its natural size.
    // A non-propagating ScrolledWindow keeps that out of the panel's width
    // negotiation, otherwise the resize drag would follow the video.
    let media_video_frame = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .hexpand(true)
        .vexpand(true)
        .child(&media_video)
        .build();
    stack.add_named(&media_video_frame, Some("media-video"));

    let media_cover = gtk::Picture::builder()
        .can_shrink(true)
        .vexpand(true)
        .margin_top(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    let audio_icon = gtk::Image::builder()
        .icon_name("audio-x-generic-symbolic")
        .pixel_size(128)
        .vexpand(true)
        .css_classes(["dim-label"])
        .build();
    let audio_art_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();
    audio_art_stack.add_named(&media_cover, Some("cover"));
    audio_art_stack.add_named(&audio_icon, Some("icon"));

    let audio_status = gtk::Label::builder()
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dim-label"])
        .margin_start(16)
        .margin_end(16)
        .build();

    let audio_btn = gtk::Button::builder()
        .label(tr("Play"))
        .css_classes(["pill", "suggested-action"])
        .build();
    let audio_back = flat_icon_button("media-seek-backward-symbolic", &tr("Back 10 seconds"));
    let audio_fwd = flat_icon_button("media-seek-forward-symbolic", &tr("Forward 10 seconds"));
    let audio_time = gtk::Label::builder()
        .label("0:00")
        .css_classes(["dim-label", "numeric"])
        .build();
    let audio_dur = gtk::Label::builder()
        .label("0:00")
        .css_classes(["dim-label", "numeric"])
        .build();
    let audio_seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    audio_seek.set_draw_value(false);
    audio_seek.set_hexpand(true);
    let audio_vol = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    audio_vol.set_draw_value(false);
    audio_vol.set_width_request(96);
    audio_vol.set_tooltip_text(Some(&tr("Volume")));

    let audio_seek_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(16)
        .margin_end(16)
        .build();
    audio_seek_row.append(&audio_time);
    audio_seek_row.append(&audio_seek);
    audio_seek_row.append(&audio_dur);

    let audio_buttons_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .build();
    audio_buttons_row.append(&audio_back);
    audio_buttons_row.append(&audio_btn);
    audio_buttons_row.append(&audio_fwd);

    let audio_vol_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .margin_bottom(16)
        .build();
    audio_vol_row.append(&gtk::Image::from_icon_name("audio-volume-high-symbolic"));
    audio_vol_row.append(&audio_vol);

    let audio_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    audio_box.append(&audio_art_stack);
    audio_box.append(&audio_status);
    audio_box.append(&audio_seek_row);
    audio_box.append(&audio_buttons_row);
    audio_box.append(&audio_vol_row);
    stack.add_named(&audio_box, Some("media-audio"));

    let image = gtk::Picture::builder()
        .can_shrink(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    let image_scroller = gtk::ScrolledWindow::builder()
        .child(&image)
        .vexpand(true)
        .build();
    stack.add_named(&image_scroller, Some("image"));

    // Text and Markdown share this view, only the buffer and tag table
    // differ between the two arms of `apply`.
    let text_view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::None)
        .top_margin(16)
        .bottom_margin(16)
        .left_margin(16)
        .right_margin(16)
        .pixels_above_lines(3)
        .pixels_below_lines(3)
        .build();
    let text_tag = gtk::TextTag::builder().name("flux-zoom").scale(1.0).build();
    text_view.buffer().tag_table().add(&text_tag);

    // Applied to the whole Markdown buffer first, so unstyled runs between
    // styled spans do not fall back to whatever family the widget chain
    // happens to provide.
    let md_base_tag = gtk::TextTag::builder()
        .name("flux-md-base")
        .family(body_font_family())
        .build();
    text_view.buffer().tag_table().add(&md_base_tag);

    // The view itself is proportional, plain-text previews apply this tag
    // to the whole buffer to opt into monospace.
    let mono_tag = gtk::TextTag::builder()
        .name("flux-mono")
        .family("monospace")
        .build();
    text_view.buffer().tag_table().add(&mono_tag);

    let text_scroller = gtk::ScrolledWindow::builder()
        .child(&text_view)
        .vexpand(true)
        .hexpand(true)
        .build();
    let text_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .build();
    let gutter = LineNumberGutter::new(&text_view, &text_scroller);
    text_row.append(&gutter.area);
    text_row.append(&text_scroller);
    let text_clamp = adw::Clamp::builder()
        .maximum_size(1200)
        .tightening_threshold(800)
        .child(&text_row)
        .build();
    stack.add_named(&text_clamp, Some("text"));

    let pdf_picture = gtk::Picture::builder()
        .can_shrink(true)
        .halign(gtk::Align::Center)
        .build();
    pdf_picture.add_css_class("flux-pdf-page");
    let pdf_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(PDF_GUTTER / 2)
        .margin_end(PDF_GUTTER / 2)
        .halign(gtk::Align::Fill)
        .build();
    pdf_box.append(&pdf_picture);

    let pdf_scroller = gtk::ScrolledWindow::builder()
        .child(&pdf_box)
        .vexpand(true)
        .hexpand(true)
        .build();

    stack.add_named(&pdf_scroller, Some("pdf"));

    let table_view = gtk::ColumnView::builder()
        .show_row_separators(true)
        .show_column_separators(false)
        .build();

    let table_clamp = adw::Clamp::builder()
        .maximum_size(1400)
        .tightening_threshold(1000)
        .child(&table_view)
        .build();

    let table_scroller = gtk::ScrolledWindow::builder()
        .child(&table_clamp)
        .vexpand(true)
        .hexpand(true)
        .build();

    stack.add_named(&table_scroller, Some("table"));

    let meta_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .valign(gtk::Align::Start)
        .build();

    let meta_group = adw::PreferencesGroup::builder()
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    meta_group.add(&meta_list);

    let meta_scroller = gtk::ScrolledWindow::builder()
        .child(&info_clamp(&meta_group))
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    stack.add_named(&meta_scroller, Some("meta"));

    let folder_grid = build_folder_grid(&sender);
    let folder_thumbs = FolderThumbs::new(&folder_grid);
    let folder_view = folder_grid.borrow().view.clone();
    // Keep the cells clear of the overlaid nav arrows so none ends up under one.
    folder_view.set_margin_start(FOLDER_GRID_NAV_GUTTER);
    folder_view.set_margin_end(FOLDER_GRID_NAV_GUTTER);
    let folder_scroller = gtk::ScrolledWindow::builder()
        .child(&folder_view)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .hexpand(true)
        .build();
    attach_folder_zoom_scroll(&folder_scroller);
    stack.add_named(&folder_scroller, Some("folder"));

    stack.set_visible_child_name("empty");

    // Placeholder, never inserted anywhere, so calls to `footer.set_visible`
    // in `apply` do not need to special-case the panel variants that skip it.
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let details_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .valign(gtk::Align::Start)
        .build();

    let details_group = adw::PreferencesGroup::builder()
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(12)
        .build();
    details_group.add(&details_list);

    let details_scroller = gtk::ScrolledWindow::builder()
        .child(&info_clamp(&details_group))
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let view_stack = adw::ViewStack::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    view_stack.add_titled_with_icon(
        &stack,
        Some("preview"),
        &tr("Preview"),
        "view-compact-symbolic",
    );
    view_stack.add_titled_with_icon(
        &details_scroller,
        Some("details"),
        &tr("Details"),
        "dialog-information-symbolic",
    );
    // Left unparented on purpose, the window header hosts it as part of
    // the `preview-switcher` stack page.
    let switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .halign(gtk::Align::Center)
        .build();

    let menu_btn = flat_icon_button("view-more-symbolic", &tr("Menu"));
    let actions_btn = flat_icon_button("system-run-symbolic", &tr("Contextual actions"));
    attach_menu_button(&menu_btn, sender.clone(), false);
    attach_menu_button(&actions_btn, sender.clone(), true);

    let extras: Vec<gtk::Widget> = vec![
        nav_box.clone().upcast(),
        spacer.clone().upcast(),
        wrap_btn.clone().upcast(),
        md_source_btn.clone().upcast(),
        install_font_btn.clone().upcast(),
        invert_btn.clone().upcast(),
        details_toggle.clone().upcast(),
        zoom_out.clone().upcast(),
        zoom_btn.clone().upcast(),
        zoom_in.clone().upcast(),
        playlist_btn.clone().upcast(),
        mute_btn.clone().upcast(),
        wide_btn.clone().upcast(),
        fullscreen_btn.clone().upcast(),
        mime_filter_btn.clone().upcast(),
        menu_btn.clone().upcast(),
        actions_btn.clone().upcast(),
    ];
    let header = {
        let s = sender.clone();
        let header = panel_header(
            Some("view-compact-symbolic"),
            &tr("Preview"),
            &extras,
            move || s.input(AppMsg::TogglePreview),
        );
        panel.append(&header);
        header
    };
    {
        let s = sender.clone();
        wide_btn.connect_toggled(move |b| s.input(AppMsg::SetWidePreviewDefault(b.is_active())));
    }
    {
        let s = sender.clone();
        fullscreen_btn.connect_clicked(move |_| s.input(AppMsg::TogglePreviewFullscreen));
    }
    {
        let s = sender.clone();
        mime_filter_btn.connect_toggled(move |_b| {
            // Skip the message when the active state was pushed by
            // `set_mime_filter_active` from `FluxApp`.
            let suppress = registry::with(|h| h.state.mime_filter_suppress.get()).unwrap_or(true);
            if suppress {
                return;
            }
            s.input(AppMsg::FilterBySelectedMime);
        });
    }
    mute_btn.connect_toggled(|b| {
        PREVIEW_MUTED.store(b.is_active(), Ordering::SeqCst);
        b.set_icon_name(if b.is_active() {
            "audio-volume-muted-symbolic"
        } else {
            "audio-volume-high-symbolic"
        });
        registry::with(|h| h.apply_mute());
    });
    playlist_btn.connect_toggled(move |b| {
        b.set_icon_name(if b.is_active() {
            &playlist_icon_on
        } else {
            &playlist_icon_off
        });
        registry::with(|h| h.set_playlist(b.is_active()));
    });
    {
        let s = sender.clone();
        install_font_btn.connect_clicked(move |_| {
            let path = registry::with(|h| h.state.path.borrow().clone()).flatten();
            if let Some(path) = path {
                s.input(AppMsg::InstallFonts(vec![path]));
            }
        });
    }
    panel.append(&banner);

    // Floating prev/next buttons over the preview content. They step the grid
    // selection so the panel follows with the adjacent item (the same thing
    // Left/Right do on the grid). Semi-transparent by default, fully opaque on
    // hover so they stay discoverable without dominating the image.
    //
    // The glyph is set through an explicit `gtk::Image::pixel_size` child
    // instead of the button's `icon_name`. GTK4 buttons with `icon_name` fold
    // the icon's `-gtk-icon-size` into their natural size before the CSS
    // `min-width`/`min-height` floor applies, so increasing `-gtk-icon-size`
    // in CSS does not grow the visible button. `pixel_size` sets an absolute
    // child size that propagates straight to the button's natural size.
    let nav_prev = gtk::Button::builder()
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .margin_start(8)
        .css_classes(["flat", "circular", "preview-nav-btn"])
        .tooltip_text(tr("Previous item"))
        .build();
    nav_prev.set_child(Some(
        &gtk::Image::builder()
            .icon_name("go-previous-symbolic")
            .pixel_size(56)
            .build(),
    ));
    let nav_next = gtk::Button::builder()
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .margin_end(8)
        .css_classes(["flat", "circular", "preview-nav-btn"])
        .tooltip_text(tr("Next item"))
        .build();
    nav_next.set_child(Some(
        &gtk::Image::builder()
            .icon_name("go-next-symbolic")
            .pixel_size(56)
            .build(),
    ));

    {
        let s = sender.clone();
        nav_prev.connect_clicked(move |_| s.input(AppMsg::SelectionStep(-1)));
    }
    {
        let s = sender.clone();
        nav_next.connect_clicked(move |_| s.input(AppMsg::SelectionStep(1)));
    }

    let view_overlay = gtk::Overlay::builder().vexpand(true).hexpand(true).build();
    view_overlay.set_child(Some(&view_stack));
    view_overlay.add_overlay(&nav_prev);
    view_overlay.add_overlay(&nav_next);

    // The arrows are only shown while the pointer is over the preview panel, so
    // they do not sit on top of the content when it is not being interacted with.
    nav_prev.set_visible(false);
    nav_next.set_visible(false);
    let hover = gtk::EventControllerMotion::new();
    {
        let (prev, next) = (nav_prev.clone(), nav_next.clone());
        hover.connect_contains_pointer_notify(move |c| {
            let inside = c.contains_pointer();
            prev.set_visible(inside);
            next.set_visible(inside);
        });
    }
    panel.add_controller(hover);

    panel.append(&view_overlay);

    // Double-click on the preview content toggles full-screen preview.
    let dbl_click = gtk::GestureClick::new();
    dbl_click.set_button(gtk::gdk::BUTTON_PRIMARY);
    dbl_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let s = sender.clone();
        dbl_click.connect_pressed(move |g, n_press, x, y| {
            if n_press != 2 {
                return;
            }
            let Some(root) = g.widget() else {
                return;
            };
            if root
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .is_some_and(|w| double_click_reserved(&w))
            {
                return;
            }
            s.input(AppMsg::TogglePreviewFullscreen);
        });
    }
    view_overlay.add_controller(dbl_click);

    let handle = PreviewPanelHandle {
        stack,
        nav_box,
        page_spin: page_spin.clone(),
        page_total,
        prev_btn: prev_btn.clone(),
        next_btn: next_btn.clone(),
        wrap_btn: wrap_btn.clone(),
        invert_btn: invert_btn.clone(),
        md_source_btn: md_source_btn.clone(),
        wide_btn: wide_btn.clone(),
        fullscreen_btn: fullscreen_btn.clone(),
        mute_btn: mute_btn.clone(),
        header,
        folder_grid,
        mime_filter_btn: mime_filter_btn.clone(),
        playlist_btn: playlist_btn.clone(),
        install_font_btn: install_font_btn.clone(),
        zoom_btn: zoom_btn.clone(),
        banner,
        image,
        image_scroller: image_scroller.clone(),
        text_view: text_view.clone(),
        text_tag,
        md_base_tag,
        pdf_scroller: pdf_scroller.clone(),
        pdf_picture: pdf_picture.clone(),
        table_view: table_view.clone(),
        meta_list,
        nopreview,
        footer,
        switcher: switcher.clone(),
        view_stack: view_stack.clone(),
        details_list,
        media_video,
        media_cover,
        audio_art_stack,
        audio_status,
        audio_btn: audio_btn.clone(),
        audio_back: audio_back.clone(),
        audio_fwd: audio_fwd.clone(),
        audio_seek: audio_seek.clone(),
        audio_vol: audio_vol.clone(),
        audio_time: audio_time.clone(),
        audio_dur: audio_dur.clone(),
        state: Rc::new(State {
            mode: Cell::new(Mode::None),
            token: Cell::new(0),
            path: RefCell::new(None),
            pdf_path: RefCell::new(None),
            pages: Cell::new(0),
            page: Cell::new(0),
            zoom: Cell::new(1.0),
            suppress: Cell::new(false),
            rendered_w: Cell::new(0),
            resize_gen: Cell::new(0),
            tex_px: Cell::new((0, 0)),
            last_image_view: Cell::new((0, 0)),
            media: RefCell::new(None),
            syntax_tags: RefCell::new(Vec::new()),
            md_cache: RefCell::new(None),
            mime_filter_suppress: Cell::new(false),
            folder_thumbs,
            folder_icon_size: Cell::new(FOLDER_GRID_ICON_SIZE),
            folder_entries: RefCell::new(Vec::new()),
            fill_gen: Cell::new(0),
            playlist: Cell::new(false),
            playlist_skip_until: Cell::new(0),
        }),
    };
    registry::set_current(handle);

    // The Details toggle and the switcher drive the same stack page, each
    // listens to the other so whichever one the user touches wins.
    {
        let vs = view_stack.clone();
        details_toggle.connect_toggled(move |b| {
            let target = if b.is_active() { "details" } else { "preview" };
            vs.set_visible_child_name(target);
        });
    }
    {
        let toggle = details_toggle.clone();
        view_stack.connect_visible_child_name_notify(move |vs| {
            let on_details = vs.visible_child_name().as_deref() == Some("details");
            if toggle.is_active() != on_details {
                toggle.set_active(on_details);
            }
        });
    }

    audio_btn.connect_clicked(|_| {
        registry::with(|h| h.audio_toggle());
    });
    audio_back.connect_clicked(|_| ipc_send(r#"{"command":["seek",-10,"relative"]}"#));
    audio_fwd.connect_clicked(|_| ipc_send(r#"{"command":["seek",10,"relative"]}"#));
    audio_seek.connect_change_value(|_, _, value| {
        AUDIO_SEEK_HOLD.store(glib::monotonic_time() + 500_000, Ordering::SeqCst);
        ipc_send(&format!(
            r#"{{"command":["seek",{:.3},"absolute"]}}"#,
            value.max(0.0)
        ));
        glib::Propagation::Proceed
    });
    audio_vol.connect_change_value(|_, _, value| {
        ipc_send(&format!(
            r#"{{"command":["set_property","volume",{:.0}]}}"#,
            value.clamp(0.0, 100.0)
        ));
        glib::Propagation::Proceed
    });

    prev_btn.connect_clicked(|_| {
        registry::with(|h| h.go_to_page(h.state.page.get().saturating_sub(1)));
    });
    next_btn.connect_clicked(|_| {
        registry::with(|h| h.go_to_page(h.state.page.get() + 1));
    });
    page_spin.connect_value_changed(|s| {
        registry::with(|h| {
            if !h.state.suppress.get() {
                h.go_to_page((s.value() as usize).saturating_sub(1));
            }
        });
    });
    zoom_out.connect_clicked(|_| {
        registry::with(|h| h.zoom_by(1.0 / ZOOM_STEP));
    });
    zoom_in.connect_clicked(|_| {
        registry::with(|h| h.zoom_by(ZOOM_STEP));
    });
    zoom_btn.connect_clicked(|_| {
        registry::with(|h| h.set_zoom(1.0));
    });
    {
        let tv = text_view.clone();
        wrap_btn.connect_toggled(move |b| {
            tv.set_wrap_mode(if b.is_active() {
                gtk::WrapMode::WordChar
            } else {
                gtk::WrapMode::None
            });
        });
    }
    {
        let pic = pdf_picture.clone();
        invert_btn.connect_toggled(move |b| {
            if b.is_active() {
                pic.add_css_class("flux-pdf-invert");
            } else {
                pic.remove_css_class("flux-pdf-invert");
            }
        });
    }
    md_source_btn.connect_toggled(|_| {
        registry::with(|h| h.apply_markdown_view());
    });

    // Pan is only attached to the image and PDF scrollers, the text
    // scroller has its own caret-driven scroll behaviour and would fight
    // a drag-to-pan controller.
    for sw in [&pdf_scroller, &image_scroller] {
        attach_zoom_scroll(sw);
        attach_pan(sw);
    }
    attach_zoom_scroll(&text_scroller);

    pdf_scroller
        .hadjustment()
        .connect_notify_local(Some("page-size"), |_, _| {
            registry::with(|h| h.schedule_resize());
        });
    image_scroller
        .hadjustment()
        .connect_notify_local(Some("page-size"), |_, _| {
            registry::with(|h| h.schedule_resize());
        });
    image_scroller
        .vadjustment()
        .connect_notify_local(Some("page-size"), |_, _| {
            registry::with(|h| h.schedule_resize());
        });

    panel.connect_unmap(|_| {
        registry::with(|h| h.stop_media());
    });

    attach_context_menu(&panel, &text_view, sender.clone());

    let sender_for_resize = sender.clone();
    resizable_panel(&spec, &panel, move |w| {
        sender_for_resize.input(AppMsg::SetPreviewPanelWidth(w));
    })
}
