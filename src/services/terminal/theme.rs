//! Colour and font theming for the terminal widget.

use super::palette::default_ansi_palette;
use super::Terminal;
use crate::model::TerminalConfig;
use adw;
use gtk::prelude::*;

impl Terminal {
    pub fn set_color_foreground(&self, color: &gtk::gdk::RGBA) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.fg_color = *color;
        state.current_fg = *color;
        self.drawing_area.queue_draw();
    }

    pub fn set_color_background(&self, color: &gtk::gdk::RGBA) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.bg_color = *color;
        state.current_bg = *color;
        self.drawing_area.queue_draw();
    }

    pub fn set_font(&self, font_desc: Option<&pango::FontDescription>) {
        if let Some(fd) = font_desc {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.font_desc = fd.clone();
            self.drawing_area.queue_draw();
        }
    }

    /// Registers a callback invoked on the PTY reader thread whenever fish
    /// reports a working-directory change via OSC 7.
    ///
    /// The callback receives the decoded, absolute path of the new directory.
    /// Register this before the widget is realized to avoid missing the first
    /// prompt. The callback must be `Send` because it is called from the PTY
    /// reader thread, use a channel (e.g. `relm4::Sender`) rather than touching
    /// GTK objects directly.
    ///
    /// # Example
    ///
    /// ```ignore
    /// terminal.set_cwd_callback({
    ///     let sender = app_sender.clone(),
    ///     move |path| {
    ///         let _ = sender.send(AppMsg::NavigateTo(path)),
    ///     }
    /// }),
    /// ```
    /// Resolves terminal colors and font from the active GTK/libadwaita theme,
    /// then applies them, but only for config fields that are empty/default,
    /// preserving any explicit user overrides in `config.toml`.
    ///
    /// Call this once after construction and again whenever the system theme
    /// changes (see [`connect_theme_changes`]).
    ///
    /// Resolution order per field (first non-empty wins):
    /// - **fg_color**: config hex → `@theme_fg_color` → fallback `#E5E5E5 / #1A1A1A`
    /// - **bg_color**: config hex → `@window_bg_color` → fallback `#1A1A1A / #FAFAFA`
    /// - **font**: config string (non-default) → system monospace → `"monospace 13"`
    pub fn apply_theme(&self, config: &TerminalConfig) {
        let widget = self.drawing_area.upcast_ref::<gtk::Widget>();

        // --- colors -------------------------------------------------------
        let style = widget.style_context();

        let theme_fg = style.lookup_color("theme_fg_color");
        let window_bg = style.lookup_color("window_bg_color");
        let accent = style.lookup_color("accent_bg_color");

        // fg: config hex → GTK theme_fg_color → hardcoded fallback
        let fg = if !config.fg_color.is_empty() {
            config.fg_color.parse::<gtk::gdk::RGBA>().ok()
        } else {
            None
        }
        .or(theme_fg)
        .unwrap_or_else(|| {
            let dark = adw::StyleManager::default().is_dark();
            if dark {
                gtk::gdk::RGBA::new(0.898, 0.898, 0.898, 1.0)
            } else {
                gtk::gdk::RGBA::new(0.133, 0.133, 0.133, 1.0)
            }
        });

        // bg: config hex → GTK window_bg_color → hardcoded fallback
        let bg = if !config.bg_color.is_empty() {
            config.bg_color.parse::<gtk::gdk::RGBA>().ok()
        } else {
            None
        }
        .or(window_bg)
        .unwrap_or_else(|| {
            let dark = adw::StyleManager::default().is_dark();
            if dark {
                gtk::gdk::RGBA::new(0.102, 0.106, 0.149, 1.0)
            } else {
                gtk::gdk::RGBA::new(0.980, 0.980, 0.980, 1.0)
            }
        });

        self.set_color_foreground(&fg);
        self.set_color_background(&bg);

        // Resolve the 16-color ANSI palette from the libadwaita named palette.
        // Each GTK variable maps to an xterm ANSI slot, missing variables fall
        // back to the compiled-in xterm defaults for that slot only.
        let palette_vars: [(&str, usize); 14] = [
            ("green_3", 2),
            ("green_5", 10),
            ("yellow_3", 3),
            ("yellow_5", 11),
            ("blue_3", 4),
            ("blue_5", 12),
            ("purple_3", 5),
            ("purple_5", 13),
            ("cyan", 6),
            ("blue_4", 14),
            ("red_3", 1),
            ("red_5", 9),
            ("dark_2", 8),
            ("light_5", 15),
        ];
        let mut palette = default_ansi_palette();
        for (var, slot) in palette_vars {
            if let Some(c) = style.lookup_color(var) {
                palette[slot] = c;
            }
        }

        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.ansi_palette = palette;
            // cursor / selection tint from accent color
            if let Some(acc) = accent {
                state.accent_color = Some(acc);
            }
        }

        // --- font ---------------------------------------------------------
        // Only substitute the theme monospace font when the config value is
        // the compiled-in default (user hasn't customised it).
        const DEFAULT_FONT: &str = "JetBrains Mono 13";
        let font_str = if config.font.is_empty() || config.font == DEFAULT_FONT {
            // Ask GTK settings for the system monospace font, then append the
            // size from the default so it looks reasonable out of the box.
            gtk::Settings::default()
                .and_then(|s| s.gtk_font_name())
                .map(|_| {
                    // Use "Geist Mono" from the Flux CSS if available,
                    // otherwise fall back to the GTK monospace font.
                    let families = ["Geist Mono", "JetBrains Mono", "monospace"];
                    let pango_ctx = widget.pango_context();
                    let available: Vec<String> = pango_ctx
                        .font_map()
                        .map(|fm| {
                            fm.list_families()
                                .iter()
                                .map(|f| f.name().to_string())
                                .collect()
                        })
                        .unwrap_or_default();
                    let chosen = families
                        .iter()
                        .find(|&&f| available.iter().any(|a| a == f))
                        .copied()
                        .unwrap_or("monospace");
                    format!("{} 13", chosen)
                })
                .unwrap_or_else(|| DEFAULT_FONT.to_string())
        } else {
            config.font.clone()
        };

        self.set_font(Some(&pango::FontDescription::from_string(&font_str)));
    }

    /// Connects to `adw::StyleManager::dark` property changes so the terminal
    /// re-applies the resolved theme whenever the user switches between light
    /// and dark mode at runtime. Only color fields that are empty in `config`
    /// will be updated, user-overridden values are preserved.
    pub fn connect_theme_changes(&self, config: TerminalConfig) {
        let term = self.clone();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            term.apply_theme(&config);
        });
    }
}
