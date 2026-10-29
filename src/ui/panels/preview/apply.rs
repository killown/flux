//! Renders a `PreviewContent` into the panel's widgets.

use super::audio::{stop_audio_mpv, AUDIO_MPV_PID};
use super::content::PreviewContent;
use super::handle::PreviewPanelHandle;
use super::media::{PLAYLIST_END_SLACK_US, PLAYLIST_WRAP_ZONE_US, PREVIEW_MUTED};
use super::registry;
use super::state::Mode;
use super::table;
use super::text::byte_to_char_map;
use super::widgets::{clear_list, push_row};
use crate::i18n::tr;
use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Duration;

impl PreviewPanelHandle {
    pub fn apply(&self, content: &PreviewContent) {
        crate::hit!("apply");
        let st = &self.state;
        st.token.set(st.token.get() + 1);
        self.stop_media();
        self.banner.set_revealed(false);

        // Leaving a folder (another file, `Loading` while arrowing, an error): free its
        // grid and drop its thumbnail loads. A new folder refills the grid below.
        if !matches!(content, PreviewContent::Folder { .. }) {
            self.release_folder_grid();
        }

        // The playlist toggle only makes sense for playable media. `Loading` keeps the
        // current state so the header does not flicker between consecutive tracks.
        match content {
            PreviewContent::Media { .. } => {
                self.playlist_btn.set_visible(true);
                self.mute_btn.set_visible(true);
                st.playlist_skip_until.set(0);
            }
            PreviewContent::Loading => {}
            _ => {
                self.playlist_btn.set_visible(false);
                self.mute_btn.set_visible(false);
                // Reached by a playlist auto-advance: not playable, move on to the next item.
                if st.playlist.get() && glib::monotonic_time() < st.playlist_skip_until.get() {
                    self.playlist_advance();
                }
            }
        }

        match content {
            PreviewContent::Loading => {
                self.set_mode(Mode::None);
                self.footer.set_visible(false);
                self.switcher.set_visible(false);
                self.view_stack.set_visible_child_name("preview");
                self.stack.set_visible_child_name("loading");
            }

            PreviewContent::Image { tex, info } => {
                self.show_info(info);
                self.set_mode(Mode::Image);
                let is_font = crate::infra::fonts::is_font_path(&info.path);
                let installed = is_font && crate::infra::fonts::is_installed(&info.path);
                self.install_font_btn.set_visible(is_font);
                self.install_font_btn.set_sensitive(!installed);
                self.install_font_btn.set_tooltip_text(Some(&if installed {
                    tr("Already installed")
                } else {
                    tr("Install font")
                }));
                st.zoom.set(1.0);
                self.sync_zoom_label();
                st.tex_px.set((tex.width(), tex.height()));
                st.last_image_view.set((0, 0));
                self.image.set_paintable(Some(tex));
                self.stack.set_visible_child_name("image");
                let h = self.clone();
                gtk::glib::idle_add_local_once(move || h.relayout_image());
            }

            PreviewContent::Pdf {
                pages,
                info,
                render_path,
            } => {
                self.show_info(info);
                *st.pdf_path.borrow_mut() = Some(render_path.clone());
                self.set_mode(Mode::Pdf);
                st.zoom.set(1.0);
                st.page.set(0);
                st.pages.set((*pages).max(1));
                st.rendered_w.set(0);
                self.sync_zoom_label();
                self.page_spin.set_range(1.0, (*pages).max(1) as f64);
                self.page_total.set_label(&format!("/ {}", (*pages).max(1)));
                self.sync_nav();
                self.pdf_scroller.vadjustment().set_value(0.0);
                self.pdf_scroller.hadjustment().set_value(0.0);
                self.stack.set_visible_child_name("loading");
                self.render_pdf();
            }

            PreviewContent::Text {
                head,
                truncated,
                hex,
                info,
                highlighted,
                language,
            } => {
                self.show_info(info);
                self.set_mode(Mode::Text);
                st.zoom.set(1.0);
                self.sync_zoom_label();
                self.text_tag.set_scale(1.0);
                self.wrap_btn.set_sensitive(!*hex);
                self.text_view
                    .set_wrap_mode(if !*hex && self.wrap_btn.is_active() {
                        gtk::WrapMode::WordChar
                    } else {
                        gtk::WrapMode::None
                    });

                let buf = self.text_view.buffer();
                buf.set_text(head);

                // Monospace first, then syntax tags on top: the syntax tags
                // override fg/weight/style, not family, so the family has
                // to be applied to the whole buffer beforehand.
                let (bs, be) = buf.bounds();
                if let Some(mono) = buf.tag_table().lookup("flux-mono") {
                    buf.apply_tag(&mono, &bs, &be);
                }

                // syntect emits byte offsets, GtkTextBuffer iterators are
                // char-indexed.
                let ranges = self.rebuild_syntax_tags(highlighted);
                if !ranges.is_empty() {
                    let map = byte_to_char_map(head);
                    for (name, range) in ranges {
                        let start = buf.iter_at_offset(map[range.start.min(head.len())]);
                        let end = buf.iter_at_offset(map[range.end.min(head.len())]);
                        buf.apply_tag_by_name(&name, &start, &end);
                    }
                }
                let (s, e) = buf.bounds();
                buf.apply_tag(&self.text_tag, &s, &e);

                if *hex {
                    self.banner
                        .set_title(&tr("Binary file - showing the first bytes as hex."));
                    self.banner.set_revealed(true);
                } else if *truncated {
                    self.banner
                        .set_title(&tr("Preview truncated - showing the start of the file."));
                    self.banner.set_revealed(true);
                } else if let Some(lang) = language {
                    self.banner
                        .set_title(&format!("{} {}", tr("Language:"), lang));
                    self.banner.set_revealed(true);
                }

                self.stack.set_visible_child_name("text");
            }

            PreviewContent::Markdown {
                rendered,
                styles,
                source,
                info,
            } => {
                self.show_info(info);
                self.set_mode(Mode::Markdown);
                st.zoom.set(1.0);
                self.sync_zoom_label();
                self.text_tag.set_scale(1.0);
                *self.state.md_cache.borrow_mut() = Some((rendered.clone(), styles.clone()));
                unsafe {
                    self.text_view.set_data("flux-md-source", source.clone());
                }
                self.md_source_btn.set_active(false);
                self.apply_markdown_view();
                self.stack.set_visible_child_name("text");
            }

            PreviewContent::Table { data, info } => {
                self.show_info(info);
                self.set_mode(Mode::Table);
                let store = table::build_store(data);
                let selection = gtk::NoSelection::new(Some(store));
                self.table_view.set_model(Some(&selection));
                self.table_view.set_hexpand(true);
                self.table_view.set_vexpand(true);
                table::build_columns(&self.table_view, data);
                if data.truncated {
                    self.banner
                        .set_title(&tr("Table truncated - showing the first rows/columns."));
                    self.banner.set_revealed(true);
                }
                self.stack.set_visible_child_name("table");
            }

            PreviewContent::Folder {
                entries,
                more,
                info,
            } => {
                self.show_info(info);
                self.set_mode(Mode::None);
                self.fill_folder_grid(entries);
                if *more > 0 {
                    self.banner
                        .set_title(&tr("Folder truncated - showing the first items only."));
                    self.banner.set_revealed(true);
                }
                self.stack.set_visible_child_name("folder");
            }

            PreviewContent::Media {
                path,
                video,
                cover,
                info,
            } => {
                self.show_info(info);
                self.set_mode(Mode::None);

                if *video {
                    let mf = gtk::MediaFile::new();
                    // GStreamer sometimes plays video silently until the
                    // volume is nudged. Toggle mute and volume to force the
                    // audio sink to negotiate.
                    fn kick_audio(m: &gtk::MediaFile) {
                        m.set_muted(true);
                        m.set_muted(false);
                        m.set_volume(0.99);
                        m.set_volume(1.0);
                        m.set_muted(PREVIEW_MUTED.load(Ordering::SeqCst));
                    }
                    mf.connect_prepared_notify(|m| {
                        if m.is_prepared() {
                            kick_audio(m);
                            m.play();
                            let m = m.clone();
                            gtk::glib::timeout_add_local_once(
                                Duration::from_millis(250),
                                move || {
                                    // A selection change may have torn the
                                    // pipeline down in the meantime, only
                                    // kick if this file is still the one in
                                    // `state.media`.
                                    let still_current = registry::with(|h| {
                                        h.state
                                            .media
                                            .borrow()
                                            .as_ref()
                                            .map(|cur| cur == &m)
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false);
                                    if still_current && m.is_prepared() {
                                        kick_audio(&m);
                                    }
                                },
                            );
                        }
                    });
                    mf.connect_error_notify(|m| {
                        if let Some(err) = m.error() {
                            let msg = err.message().to_string();
                            registry::with(|h| h.apply(&PreviewContent::Error(msg)));
                        }
                    });
                    // `gtk::Video` loops by default and a looping stream never reports `ended`,
                    // so also watch the playback position: reaching the end, or wrapping around
                    // from it, means the file finished.
                    let last_ts = Rc::new(Cell::new(0_i64));
                    let fired = Rc::new(Cell::new(false));
                    mf.connect_timestamp_notify(move |m| {
                        let ts = m.timestamp();
                        let prev = last_ts.replace(ts);
                        let dur = m.duration();
                        if dur <= 0 || fired.get() {
                            return;
                        }
                        let near_end = ts >= dur - PLAYLIST_END_SLACK_US;
                        let wrapped = prev >= dur - PLAYLIST_WRAP_ZONE_US && ts + 1_000_000 < prev;
                        if !(near_end || wrapped) {
                            return;
                        }
                        let still_current = registry::with(|h| {
                            h.state.playlist.get()
                                && h.state
                                    .media
                                    .borrow()
                                    .as_ref()
                                    .map(|cur| cur == m)
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false);
                        if still_current {
                            fired.set(true);
                            registry::with(|h| h.playlist_advance());
                        }
                    });
                    mf.connect_ended_notify(|m| {
                        if !m.is_ended() {
                            return;
                        }
                        let current = registry::with(|h| {
                            h.state
                                .media
                                .borrow()
                                .as_ref()
                                .map(|cur| cur == m)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                        if current {
                            registry::with(|h| h.playlist_advance());
                        }
                    });
                    mf.set_file(Some(&gtk::gio::File::for_path(path)));
                    // Set the widget's loop before attaching, it applies it to the stream.
                    self.media_video.set_loop(!st.playlist.get());
                    self.media_video.set_media_stream(Some(&mf));
                    if st.playlist.get() {
                        mf.set_loop(false);
                    }
                    *self.state.media.borrow_mut() = Some(mf);
                    self.stack.set_visible_child_name("media-video");
                } else {
                    if AUDIO_MPV_PID.load(Ordering::SeqCst) != 0 {
                        stop_audio_mpv();
                    }
                    self.audio_reset_controls();
                    self.audio_art_stack
                        .set_visible_child_name(if cover.is_some() { "cover" } else { "icon" });
                    if let Some(tex) = cover {
                        self.media_cover.set_paintable(Some(tex));
                    }

                    if glib::find_program_in_path("mpv").is_none() {
                        self.audio_status.set_label(&tr(
                            "mpv is not installed. Install it to enable audio preview, or use Open.",
                        ));
                        self.audio_btn.set_sensitive(false);
                    } else {
                        self.audio_status.set_label(&tr("Ready."));
                        self.audio_btn.set_sensitive(true);
                    }
                    self.audio_btn.set_label(&tr("Play"));

                    self.stack.set_visible_child_name("media-audio");

                    // Short delay so a fast arrow-key pass does not spawn
                    // mpv for files the user is already scrolling past.
                    let token = st.token.get();
                    let h = self.clone();
                    gtk::glib::timeout_add_local_once(Duration::from_millis(300), move || {
                        if h.state.token.get() == token && h.stack.is_mapped() {
                            h.start_audio();
                        }
                    });
                }
            }

            PreviewContent::Metadata { info } => {
                self.show_info(info);
                self.set_mode(Mode::None);
                if info.extra.is_empty() {
                    self.nopreview
                        .set_icon_name(Some("text-x-generic-symbolic"));
                    self.nopreview.set_title(&tr("No preview available"));
                    self.nopreview.set_description(Some(&info.mime));
                    self.stack.set_visible_child_name("nopreview");
                } else {
                    clear_list(&self.meta_list);
                    for (k, v) in &info.extra {
                        push_row(&self.meta_list, k, v);
                    }
                    self.stack.set_visible_child_name("meta");
                }
            }

            PreviewContent::Error(msg) => {
                self.set_mode(Mode::None);
                self.footer.set_visible(false);
                self.switcher.set_visible(false);
                self.view_stack.set_visible_child_name("preview");
                self.nopreview.set_icon_name(Some("dialog-error-symbolic"));
                self.nopreview.set_title(&tr("Preview failed"));
                self.nopreview.set_description(Some(msg));
                self.stack.set_visible_child_name("nopreview");
            }
        }
    }
}
