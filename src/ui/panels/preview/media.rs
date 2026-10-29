//! Playback behaviour shared by the video and audio paths: mute, stop and
//! playlist mode.

use super::audio::{ipc_send, stop_audio_mpv, AUDIO_MPV_PID};
use super::handle::PreviewPanelHandle;
use crate::i18n::tr;
use crate::model::AppMsg;
use adw::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// After a playlist auto-advance, non-media items stay skippable for this long (µs).
/// Bounded so a manual selection later (e.g. after the last track) is never skipped.
const PLAYLIST_SKIP_WINDOW_US: i64 = 3_000_000;
/// A video counts as finished once its position is this close to its duration (µs).
pub(super) const PLAYLIST_END_SLACK_US: i64 = 300_000;
/// A backwards jump from this close to the end is a loop wrap-around, not a user seek (µs).
pub(super) const PLAYLIST_WRAP_ZONE_US: i64 = 1_500_000;

/// Mute toggle of the header button, applies to video and audio, reset when the panel closes.
pub(super) static PREVIEW_MUTED: AtomicBool = AtomicBool::new(false);

impl PreviewPanelHandle {
    /// Turns playlist mode on or off. A playing video stops looping so it can end.
    pub(super) fn set_playlist(&self, on: bool) {
        self.state.playlist.set(on);
        self.state.playlist_skip_until.set(0);
        // `gtk::Video` is built with `loop_(true)` and pushes that onto its stream,
        // so the widget property is what has to change for the video to end.
        self.media_video.set_loop(!on);
        if on {
            if let Some(mf) = self.state.media.borrow().as_ref() {
                mf.set_loop(false);
            }
        }
    }

    /// Called when the current video/audio reached its end: selects the next item.
    ///
    /// Non-media items that follow (covers, logs, folders) are skipped, see `apply`.
    /// Stepping clamps at the last item, so the playlist simply stops there.
    pub(super) fn playlist_advance(&self) {
        if !self.state.playlist.get() {
            return;
        }
        self.state
            .playlist_skip_until
            .set(glib::monotonic_time() + PLAYLIST_SKIP_WINDOW_US);
        if let Some(s) = crate::model::SENDER.get() {
            let _ = s.send(AppMsg::SelectionStep(1));
        }
    }

    pub fn stop_media(&self) {
        crate::hit!("stop_media");
        self.state.token.set(self.state.token.get() + 1);

        let mf = self.state.media.borrow_mut().take();
        if let Some(mf) = mf {
            mf.pause();
            self.media_video.set_media_stream(None::<&gtk::MediaStream>);
            mf.clear();
        } else {
            self.media_video.set_media_stream(None::<&gtk::MediaStream>);
        }

        if AUDIO_MPV_PID.load(Ordering::SeqCst) != 0 {
            stop_audio_mpv();
            self.audio_reset_controls();
            self.audio_status.set_label(&tr("Stopped."));
        }
    }

    /// Pushes the mute state to the playing video and the mpv instance.
    pub(super) fn apply_mute(&self) {
        let muted = PREVIEW_MUTED.load(Ordering::SeqCst);
        if let Some(mf) = self.state.media.borrow().as_ref() {
            mf.set_muted(muted);
            if !muted {
                // GStreamer's sink can stay silent after unmuting until the volume changes.
                mf.set_volume(0.99);
                mf.set_volume(1.0);
            }
        }
        ipc_send(&format!(r#"{{"command":["set_property","mute",{muted}]}}"#));
    }

    /// Unmutes, called when the preview panel closes.
    pub fn reset_mute(&self) {
        if self.mute_btn.is_active() {
            self.mute_btn.set_active(false);
        }
    }
}
