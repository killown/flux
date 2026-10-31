//! Colour and font theming for the terminal widget.

use super::palette::default_ansi_palette;
use super::Terminal;
use crate::domain::model::TerminalConfig;
use adw;
use gtk::prelude::*;

/// Resolved theme colours, as `#rrggbb` strings, safe to hand to child
/// processes (`FLUX_THEME_*` env vars) and to write into a fish `conf.d`
/// snippet. `palette` has 16 entries in xterm order (0-7 normal, 8-15 bright).
#[derive(Debug, Clone)]
pub(crate) struct ThemeEnv {
    pub fg: String,
    pub bg: String,
    pub accent: String,
    pub palette: [String; 16],
    /// `"dark"` or `"light"`.
    pub mode: String,
}

/// Formats a `gdk::RGBA` as `#rrggbb`, dropping alpha. Alpha is not
/// meaningful for child-process colour env vars.
#[inline]
fn rgba_hex(c: &gtk::gdk::RGBA) -> String {
    let r = (c.red() * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (c.green() * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (c.blue() * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

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
    // `GtkStyleContext::lookup_color` is deprecated since GTK 4.10 with no direct
    // replacement for named theme colors (`@window_bg_color`, `@accent_bg_color`, ...).
    #[allow(deprecated)]
    pub fn apply_theme(&self, config: &TerminalConfig) {
        crate::hit!("apply_theme");
        let widget = self.drawing_area.upcast_ref::<gtk::Widget>();

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

        // --- publish env + fish snippet -----------------------------------
        // Snapshot the resolved colours so `spawn_async` can export them as
        // `FLUX_THEME_*`, and regenerate the fish `conf.d` snippet.
        //
        // This runs after every field has been resolved above, so `ThemeEnv`
        // always matches what the terminal widget is actually rendering.
        let accent_rgba = accent.unwrap_or_else(|| gtk::gdk::RGBA::new(0.071, 0.451, 0.698, 1.0));
        let palette_hex: [String; 16] = palette.map(|c| rgba_hex(&c));
        let env = ThemeEnv {
            fg: rgba_hex(&fg),
            bg: rgba_hex(&bg),
            accent: rgba_hex(&accent_rgba),
            palette: palette_hex,
            mode: if adw::StyleManager::default().is_dark() {
                "dark".into()
            } else {
                "light".into()
            },
        };
        if let Ok(mut slot) = self.theme_env.lock() {
            *slot = Some(env.clone());
        }
        super::fish_theme::write_fish_theme_snippet(&env);
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
