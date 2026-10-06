use crate::i18n::tr;
use crate::model::{AppMsg, Config};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("System"),
            set_icon_name: Some("applications-system-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("Performance & Engine"),
                set_description: Some(&tr("Configure background worker limits and streaming chunk sizes")),
                add = &adw::ActionRow {
                    set_title: &tr("UI Batch Streaming Size"),
                    set_subtitle: &tr("Number of items appended per frame when loading directories"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.loader_batch_size as f64,
                            10.0,
                            500.0,
                            10.0,
                            50.0,
                            0.0,
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetLoaderBatchSize(spin.value() as usize));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Thumbnail Workers"),
                    set_subtitle: &tr("Concurrent background threads used to render thumbnails"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.thumbnail_threads as f64,
                            1.0,
                            16.0,
                            1.0,
                            2.0,
                            0.0,
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetThumbnailThreads(spin.value() as usize));
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Memory & Limits"),
                set_description: Some(&tr("Tune caching thresholds and search result cutoffs")),
                add = &adw::ActionRow {
                    set_title: &tr("Fast File Indexing"),
                    set_subtitle: &tr("Build an SQLite FTS5 database to enable sub-millisecond filename searches"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.enable_file_indexing,
                        set_valign: gtk::Align::Center,
                        connect_state_set => |_, state| {
                            let mut cfg = crate::utils::load_config();
                            cfg.ui.enable_file_indexing = state;
                            crate::utils::save_config(&cfg);

                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetEnableFileIndexing(state));
                            }

                            if state {
                                crate::services::indexer::build_home_index_async();
                            }
                            glib::Propagation::Proceed
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Folder Cache Capacity"),
                    set_subtitle: &tr("Maximum number of recently visited folders kept in RAM (0 to disable)"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.folder_cache_capacity as f64,
                            0.0,
                            30.0,
                            1.0,
                            5.0,
                            0.0,
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetFolderCacheCapacity(spin.value() as usize));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Max Search Results"),
                    set_subtitle: &tr("Maximum files indexed per filename/extension search run"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.max_search_results as f64,
                            50.0,
                            50000.0,
                            50.0,
                            500.0,
                            0.0,
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetMaxSearchResults(spin.value() as usize));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Max Content Search Results"),
                    set_subtitle: &tr("Maximum number of matches to return (higher values may be slower)"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.max_content_search_results as f64,
                            10.0, 5000.0, 10.0, 100.0, 0.0
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetMaxContentSearchResults(spin.value() as usize));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Navigation History Depth"),
                    set_subtitle: &tr("Maximum number of backward/forward directory jumps saved"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.max_history as f64,
                            10.0,
                            500.0,
                            10.0,
                            50.0,
                            0.0,
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetMaxHistory(spin.value() as usize));
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Video Extraction (FFmpeg)"),
                set_description: Some(&tr("Configure video frame extraction and decoding options")),
                add = &adw::ActionRow {
                    set_title: &tr("Initial Frame Offset"),
                    set_subtitle: &tr("Timestamp in seconds into the video to capture thumbnail"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.ffmpeg_seek_seconds,
                            0.0,
                            60.0,
                            0.5,
                            5.0,
                            0.0,
                        ),
                        set_digits: 1,
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetFfmpegSeekSeconds(spin.value()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("FFmpeg Decoder Threads"),
                    set_subtitle: &tr("Threads allocated to each video frame capture task"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.ffmpeg_threads as f64,
                            1.0,
                            8.0,
                            1.0,
                            2.0,
                            0.0,
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetFfmpegThreads(spin.value() as usize));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Auto-Rotate Videos"),
                    set_subtitle: &tr("Rotate thumbnails based on container orientation metadata"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.ffmpeg_auto_rotate,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetFfmpegAutoRotate(switch.is_active()));
                            }
                        }
                    }
                },
            },
        }
    }

    page
}
