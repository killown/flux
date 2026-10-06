use crate::i18n::tr;
use crate::model::{AppMsg, Config};
use adw::prelude::*;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("Thumbnails"),
            set_icon_name: Some("image-x-generic-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("Preview Settings"),
                set_description: Some(&tr("Enable or disable thumbnail generation for different file types")),
                add = &adw::ActionRow {
                    set_title: &tr("Enable Thumbnails"),
                    set_subtitle: &tr("Show previews for supported file types"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShowThumbnails(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Thumbnail Size"),
                    set_subtitle: &tr("Target resolution for media previews (higher values increase memory usage)"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::DropDown {
                        set_valign: gtk::Align::Center,
                        set_model: Some(&{
                            let sl = gtk::StringList::new(&[]);
                            for sz in [16, 24, 32, 48, 64, 96, 128, 144, 160, 192, 256, 384, 512, 768] {
                                sl.append(&format!("{} px", sz));
                            }
                            sl
                        }),
                        set_selected: {
                            const SIZES: &[i32] = &[16, 24, 32, 48, 64, 96, 128, 144, 160, 192, 256, 384, 512, 768];
                            SIZES.iter()
                                .position(|&sz| sz == config.ui.thumbnail_size)
                                .unwrap_or(10) as u32
                        },
                        connect_selected_notify => move |drop| {
                            const SIZES: &[i32] = &[16, 24, 32, 48, 64, 96, 128, 144, 160, 192, 256, 384, 512, 768];
                            let idx = drop.selected() as usize;
                            if let Some(&size) = SIZES.get(idx) {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetThumbnailSize(size));
                                }
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Lazy Thumbnails"),
                    set_subtitle: &tr("Generate thumbnails only for items in view"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.lazy_thumbnails && config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetLazyThumbnails(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Autoplay Video Previews"),
                    set_subtitle: &tr("Play a muted preview when a single video card is selected"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_valign: gtk::Align::Center,
                        set_active: config.ui.autoplay_video_previews,
                        connect_state_set => move |_, state| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetAutoplayVideoPreviews(state));
                            }
                            gtk::glib::Propagation::Proceed
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Images"),
                    set_subtitle: &tr("PNG, JPG, GIF, WebP, AVIF, HEIC, BMP, TIFF, SVG"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.thumbnail_types.images && config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetThumbnailType {
                                    type_name: "images".to_string(),
                                    enabled: switch.is_active()
                                });
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Videos"),
                    set_subtitle: &tr("MP4, MKV, WebM, AVI, MOV, FLV, WMV"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.thumbnail_types.videos && config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetThumbnailType {
                                    type_name: "videos".to_string(),
                                    enabled: switch.is_active()
                                });
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Fonts"),
                    set_subtitle: &tr("TTF, OTF, WOFF, WOFF2, TTC"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.thumbnail_types.fonts && config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetThumbnailType {
                                    type_name: "fonts".to_string(),
                                    enabled: switch.is_active()
                                });
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("PDF Documents"),
                    set_subtitle: &tr("Portable Document Format files"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.thumbnail_types.pdfs && config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetThumbnailType {
                                    type_name: "pdfs".to_string(),
                                    enabled: switch.is_active()
                                });
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Windows Executables"),
                    set_subtitle: &tr("EXE"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.thumbnail_types.executables && config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetThumbnailType {
                                    type_name: "executables".to_string(),
                                    enabled: switch.is_active()
                                });
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Audio"),
                    set_subtitle: &tr("MP3, FLAC, M4A, OGG, WAV (embedded album art)"),
                    set_sensitive: config.ui.show_thumbnails,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.thumbnail_types.audio && config.ui.show_thumbnails,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetThumbnailType {
                                    type_name: "audio".to_string(),
                                    enabled: switch.is_active()
                                });
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Preview Examples"),
                set_description: Some(&tr("How thumbnails will appear in the file grid")),
                add = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::Center,
                    set_spacing: 24,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Center,
                        set_spacing: 6,
                        gtk::Image {
                            set_icon_name: Some("image-x-generic-symbolic"),
                            set_pixel_size: 64,
                            set_opacity: if config.ui.thumbnail_types.images && config.ui.show_thumbnails { 1.0 } else { 0.3 },
                        },
                        gtk::Label {
                            set_label: &tr("Image"),
                            set_css_classes: &["caption", "dim-label"],
                        },
                        gtk::Label {
                            set_label: if config.ui.thumbnail_types.images && config.ui.show_thumbnails { "✓" } else { "✗" },
                            set_css_classes: &["caption"],
                            set_halign: gtk::Align::Center,
                            add_css_class: if config.ui.thumbnail_types.images && config.ui.show_thumbnails { "success" } else { "dim-label" },
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Center,
                        set_spacing: 6,
                        gtk::Image {
                            set_icon_name: Some("video-x-generic-symbolic"),
                            set_pixel_size: 64,
                            set_opacity: if config.ui.thumbnail_types.videos && config.ui.show_thumbnails { 1.0 } else { 0.3 },
                        },
                        gtk::Label {
                            set_label: &tr("Video"),
                            set_css_classes: &["caption", "dim-label"],
                        },
                        gtk::Label {
                            set_label: if config.ui.thumbnail_types.videos && config.ui.show_thumbnails { "✓" } else { "✗" },
                            set_css_classes: &["caption"],
                            set_halign: gtk::Align::Center,
                            add_css_class: if config.ui.thumbnail_types.videos && config.ui.show_thumbnails { "success" } else { "dim-label" },
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Center,
                        set_spacing: 6,
                        gtk::Image {
                            set_icon_name: Some("font-x-generic-symbolic"),
                            set_pixel_size: 64,
                            set_opacity: if config.ui.thumbnail_types.fonts && config.ui.show_thumbnails { 1.0 } else { 0.3 },
                        },
                        gtk::Label {
                            set_label: &tr("Font"),
                            set_css_classes: &["caption", "dim-label"],
                        },
                        gtk::Label {
                            set_label: if config.ui.thumbnail_types.fonts && config.ui.show_thumbnails { "✓" } else { "✗" },
                            set_css_classes: &["caption"],
                            set_halign: gtk::Align::Center,
                            add_css_class: if config.ui.thumbnail_types.fonts && config.ui.show_thumbnails { "success" } else { "dim-label" },
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Center,
                        set_spacing: 6,
                        gtk::Image {
                            set_icon_name: Some("application-pdf-symbolic"),
                            set_pixel_size: 64,
                            set_opacity: if config.ui.thumbnail_types.pdfs && config.ui.show_thumbnails { 1.0 } else { 0.3 },
                        },
                        gtk::Label {
                            set_label: &tr("PDF"),
                            set_css_classes: &["caption", "dim-label"],
                        },
                        gtk::Label {
                            set_label: if config.ui.thumbnail_types.pdfs && config.ui.show_thumbnails { "✓" } else { "✗" },
                            set_css_classes: &["caption"],
                            set_halign: gtk::Align::Center,
                            add_css_class: if config.ui.thumbnail_types.pdfs && config.ui.show_thumbnails { "success" } else { "dim-label" },
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Center,
                        set_spacing: 6,
                        gtk::Image {
                            set_icon_name: Some("application-x-executable-symbolic"),
                            set_pixel_size: 64,
                            set_opacity: if config.ui.thumbnail_types.executables && config.ui.show_thumbnails { 1.0 } else { 0.3 },
                        },
                        gtk::Label {
                            set_label: &tr("EXE"),
                            set_css_classes: &["caption", "dim-label"],
                        },
                        gtk::Label {
                            set_label: if config.ui.thumbnail_types.executables && config.ui.show_thumbnails { "✓" } else { "✗" },
                            set_css_classes: &["caption"],
                            set_halign: gtk::Align::Center,
                            add_css_class: if config.ui.thumbnail_types.executables && config.ui.show_thumbnails { "success" } else { "dim-label" },
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Center,
                        set_spacing: 6,
                        gtk::Image {
                            set_icon_name: Some("audio-x-generic-symbolic"),
                            set_pixel_size: 64,
                            set_opacity: if config.ui.thumbnail_types.audio && config.ui.show_thumbnails { 1.0 } else { 0.3 },
                        },
                        gtk::Label {
                            set_label: &tr("Audio"),
                            set_css_classes: &["caption", "dim-label"],
                        },
                        gtk::Label {
                            set_label: if config.ui.thumbnail_types.audio && config.ui.show_thumbnails { "✓" } else { "✗" },
                            set_css_classes: &["caption"],
                            set_halign: gtk::Align::Center,
                            add_css_class: if config.ui.thumbnail_types.audio && config.ui.show_thumbnails { "success" } else { "dim-label" },
                        },
                    },
                },
            },
        }
    }

    page
}
