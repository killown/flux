#!/usr/bin/env python3
import os
import sys
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, Gtk, Pango

THEMES_DIR = os.path.expanduser("~/.local/share/flux/themes")

PRESETS = {
    "Default Dark": {
        # Base & Surfaces
        "window_bg": "#121212",
        "header_bg": "#181818",
        "sidebar_bg": "#141414",
        "card_bg": "#1e1e1e",
        "entry_bg": "#1e1e1e",
        "popover_bg": "#1a1a1a",
        "quick_panel_bg": "rgba(18, 18, 18, 0.75)",
        "status_bg": "rgba(18, 18, 18, 0.95)",
        # Foreground & Typography
        "fg_color": "#e5e5e5",
        "dim_fg": "#888888",
        "accent": "#007aff",
        "accent_fg": "#ffffff",
        # Hover & Active States
        "hover_bg": "rgba(255, 255, 255, 0.08)",
        "selected_bg": "#007aff",
        "selected_fg": "#ffffff",
        # Borders & Separators
        "border_color": "rgba(255, 255, 255, 0.08)",
        "card_border": "rgba(255, 255, 255, 0.08)",
        "separator_color": "rgba(255, 255, 255, 0.08)",
        # Interactive Controls
        "breadcrumb_bg": "rgba(255, 255, 255, 0.06)",
        "breadcrumb_active_bg": "#007aff",
        "breadcrumb_active_fg": "#ffffff",
        "button_bg": "rgba(255, 255, 255, 0.05)",
        "button_fg": "#e5e5e5",
        "progress_fill": "#007aff",
        "progress_trough": "rgba(255, 255, 255, 0.05)",
        "scrollbar_slider": "rgba(255, 255, 255, 0.2)",
        "scrollbar_slider_hover": "rgba(255, 255, 255, 0.4)",
        "slider_highlight": "#007aff",
        # Accents & Feedback
        "suggested_action_bg": "#007aff",
        "suggested_action_fg": "#ffffff",
        "destructive_action_bg": "rgba(255, 59, 48, 0.15)",
        "destructive_action_fg": "#ff3b30",
        "error_color": "#ff3b30",
        "warning_color": "#ff9500",
        "success_color": "#34c759",
        # Metrics & Fonts
        "card_radius": 12,
        "card_padding": 10,
        "letter_spacing": 0.2,
        "font_main": "Inter 10",
        "font_mono": "JetBrains Mono 10",
    },
    "Clean Light": {
        "window_bg": "#f5f5f7",
        "header_bg": "#ececec",
        "sidebar_bg": "#f0f0f2",
        "card_bg": "#ffffff",
        "entry_bg": "#ffffff",
        "popover_bg": "#ffffff",
        "quick_panel_bg": "rgba(245, 245, 247, 0.75)",
        "status_bg": "rgba(255, 255, 255, 0.9)",
        "fg_color": "#1d1d1f",
        "dim_fg": "#6e6e73",
        "accent": "#007aff",
        "accent_fg": "#ffffff",
        "hover_bg": "rgba(0, 0, 0, 0.06)",
        "selected_bg": "#007aff",
        "selected_fg": "#ffffff",
        "border_color": "rgba(0, 0, 0, 0.08)",
        "card_border": "rgba(0, 0, 0, 0.08)",
        "separator_color": "rgba(0, 0, 0, 0.08)",
        "breadcrumb_bg": "rgba(0, 0, 0, 0.04)",
        "breadcrumb_active_bg": "#007aff",
        "breadcrumb_active_fg": "#ffffff",
        "button_bg": "rgba(0, 0, 0, 0.03)",
        "button_fg": "#1d1d1f",
        "progress_fill": "#007aff",
        "progress_trough": "rgba(0, 0, 0, 0.05)",
        "scrollbar_slider": "rgba(0, 0, 0, 0.15)",
        "scrollbar_slider_hover": "rgba(0, 0, 0, 0.3)",
        "slider_highlight": "#007aff",
        "suggested_action_bg": "#007aff",
        "suggested_action_fg": "#ffffff",
        "destructive_action_bg": "rgba(255, 59, 48, 0.08)",
        "destructive_action_fg": "#ff3b30",
        "error_color": "#ff3b30",
        "warning_color": "#ff9500",
        "success_color": "#34c759",
        "card_radius": 14,
        "card_padding": 10,
        "letter_spacing": 0.2,
        "font_main": "Inter 10",
        "font_mono": "JetBrains Mono 10",
    },
    "Monochrome": {
        "window_bg": "#000000",
        "header_bg": "#000000",
        "sidebar_bg": "#050505",
        "card_bg": "#0a0a0a",
        "entry_bg": "#0a0a0a",
        "popover_bg": "#0a0a0a",
        "quick_panel_bg": "#080808",
        "status_bg": "#050505",
        "fg_color": "#ffffff",
        "dim_fg": "#777777",
        "accent": "#ffffff",
        "accent_fg": "#000000",
        "hover_bg": "#141414",
        "selected_bg": "#ffffff",
        "selected_fg": "#000000",
        "border_color": "#222222",
        "card_border": "#1a1a1a",
        "separator_color": "#222222",
        "breadcrumb_bg": "#0d0d0d",
        "breadcrumb_active_bg": "#ffffff",
        "breadcrumb_active_fg": "#000000",
        "button_bg": "#111111",
        "button_fg": "#ffffff",
        "progress_fill": "#ffffff",
        "progress_trough": "#1a1a1a",
        "scrollbar_slider": "#333333",
        "scrollbar_slider_hover": "#555555",
        "slider_highlight": "#ffffff",
        "suggested_action_bg": "#ffffff",
        "suggested_action_fg": "#000000",
        "destructive_action_bg": "#000000",
        "destructive_action_fg": "#ffffff",
        "error_color": "#ffffff",
        "warning_color": "#888888",
        "success_color": "#ffffff",
        "card_radius": 8,
        "card_padding": 10,
        "letter_spacing": 0.2,
        "font_main": "Inter 10",
        "font_mono": "JetBrains Mono 10",
    },
    "Cyberpunk Neon": {
        "window_bg": "#0f051d",
        "header_bg": "#19083b",
        "sidebar_bg": "#130624",
        "card_bg": "#220b4d",
        "entry_bg": "#220b4d",
        "popover_bg": "#19083b",
        "quick_panel_bg": "rgba(25, 8, 59, 0.85)",
        "status_bg": "#0f051d",
        "fg_color": "#00f0ff",
        "dim_fg": "#ff007f",
        "accent": "#ffe600",
        "accent_fg": "#000000",
        "hover_bg": "rgba(0, 240, 255, 0.15)",
        "selected_bg": "#ffe600",
        "selected_fg": "#000000",
        "border_color": "rgba(0, 240, 255, 0.25)",
        "card_border": "rgba(0, 240, 255, 0.3)",
        "separator_color": "rgba(255, 0, 127, 0.3)",
        "breadcrumb_bg": "rgba(0, 240, 255, 0.08)",
        "breadcrumb_active_bg": "#ffe600",
        "breadcrumb_active_fg": "#000000",
        "button_bg": "#2a0d5c",
        "button_fg": "#00f0ff",
        "progress_fill": "#ffe600",
        "progress_trough": "#19083b",
        "scrollbar_slider": "rgba(0, 240, 255, 0.3)",
        "scrollbar_slider_hover": "rgba(0, 240, 255, 0.6)",
        "slider_highlight": "#ffe600",
        "suggested_action_bg": "#ffe600",
        "suggested_action_fg": "#000000",
        "destructive_action_bg": "rgba(255, 0, 127, 0.2)",
        "destructive_action_fg": "#ff007f",
        "error_color": "#ff007f",
        "warning_color": "#ffe600",
        "success_color": "#00f0ff",
        "card_radius": 6,
        "card_padding": 10,
        "letter_spacing": 0.4,
        "font_main": "Fira Code 10",
        "font_mono": "JetBrains Mono 10",
    },
}

CSS_TEMPLATE = """\
:root {{
  --accent-color: {accent};
  --font-main: "{font_main_family}", system-ui, sans-serif;
  --font-mono: "{font_mono_family}", monospace;
  letter-spacing: {letter_spacing:.2f}px;
}}

window {{
  background-color: {window_bg};
  color: {fg_color};
}}

headerbar {{
  background-color: {header_bg};
  border-bottom: 1px solid {border_color};
  padding: 4px 12px;
  min-height: 38px;
  font-family: var(--font-main);
}}

headerbar label {{
  color: {fg_color};
  letter-spacing: {header_spacing:.2f}px;
  font-weight: 700;
  text-transform: uppercase;
}}

headerbar button {{
  background-color: transparent;
  color: {fg_color};
  border-radius: 6px;
}}

headerbar button:hover {{
  background-color: {hover_bg};
}}

.sidebar {{
  background-color: {sidebar_bg};
  border: none;
  box-shadow: inset -1px 0 0 0 {border_color};
}}

.sidebar-row {{
  margin: 2px 10px;
  padding: 6px 12px;
  border-radius: 8px;
  color: {fg_color};
  transition: background-color 150ms ease;
}}

.sidebar-row label {{
  color: {fg_color};
  font-family: var(--font-main);
}}

.sidebar-row:hover:not(:selected) {{
  background-color: {hover_bg};
}}

.sidebar-row:selected {{
  background-color: {selected_bg};
  color: {selected_fg};
}}

.sidebar-row:selected label {{
  color: {selected_fg};
  font-weight: bold;
}}

.sidebar-section {{
  margin-top: 8px;
  margin-bottom: 2px;
  padding: 10px 12px 6px 12px;
  min-height: 0px;
  background: transparent;
}}

.sidebar-section .sidebar-section-label {{
  font-size: 0.65rem;
  font-weight: 800;
  text-transform: uppercase;
  color: {dim_fg};
}}

gridview,
listview {{
  background-color: transparent;
  padding: 16px 20px;
}}

gridview child {{
  padding: 2px;
  margin: 0px;
}}

.flux-card {{
  background-color: {card_bg};
  border-radius: {card_radius}px;
  padding: {card_padding}px;
  margin: 4px;
  border: 1px solid {card_border};
  transition: background-color 150ms ease, border-color 150ms ease;
}}

.flux-card:hover {{
  background-color: {hover_bg};
  border-color: {accent};
}}

.flux-label {{
  color: {fg_color};
  font-family: var(--font-main);
  font-size: 10pt;
}}

gridview > child:selected .flux-card {{
  background-color: {selected_bg};
  border-color: {accent};
}}

gridview > child:selected .flux-label {{
  color: {selected_fg};
  font-weight: bold;
}}

/* ── Breadcrumb Navigation ── */
button.breadcrumb-btn {{
  background-color: {breadcrumb_bg};
  border: 1px solid {border_color};
  border-right-width: 0px;
  border-radius: 0px;
  margin: 0px;
  padding: 4px 14px;
  font-weight: 500;
  color: {fg_color};
  font-family: var(--font-mono);
  font-size: 0.9em;
}}

button.breadcrumb-btn:first-child {{
  border-top-left-radius: 16px;
  border-bottom-left-radius: 16px;
}}

button.breadcrumb-btn:last-child {{
  border-top-right-radius: 16px;
  border-bottom-right-radius: 16px;
  border-right-width: 1px;
  background-color: {breadcrumb_active_bg};
  border-color: {breadcrumb_active_bg};
  color: {breadcrumb_active_fg};
  font-weight: 600;
  padding: 4px 18px;
}}

button.breadcrumb-btn:hover:not(:last-child) {{
  background-color: {hover_bg};
}}

button.breadcrumb-btn:active:not(:last-child) {{
  transform: translateY(1px);
}}

button.breadcrumb-btn label {{
  color: inherit;
  font-family: var(--font-mono);
  font-size: 0.9em;
}}

button.breadcrumb-btn:last-child label {{
  color: {breadcrumb_active_fg};
  font-weight: 700;
}}

/* ── Input Entries ── */
entry {{
  background-color: {entry_bg};
  border: 1px solid {border_color};
  border-radius: 8px;
  padding: 4px 12px;
  color: {fg_color};
}}

entry:focus {{
  border-color: {accent};
  box-shadow: 0 0 0 1px {accent};
}}

/* ── Popovers & Menus ── */
popover.menu contents {{
  background-color: {popover_bg};
  border: 1px solid {border_color};
  border-radius: 10px;
  padding: 4px;
}}

popover.menu modelbutton {{
  color: {fg_color};
  border-radius: 6px;
  padding: 6px 10px;
}}

popover.menu modelbutton:hover {{
  background-color: {selected_bg};
  color: {selected_fg};
}}

popover.menu > contents separator {{
  background-color: {separator_color};
  min-height: 1px;
  margin: 4px 0px;
}}

/* ── Status Bar & Progress ── */
.selection-status {{
  background-color: {status_bg};
  border-top: 1px solid {border_color};
  padding: 4px 16px;
  min-height: 28px;
}}

.selection-status label.caption {{
  color: {dim_fg};
  font-family: var(--font-mono);
  font-size: 8.5pt;
}}

progressbar trough {{
  background-color: {progress_trough};
  border-radius: 4px;
}}

progressbar progress {{
  background-color: {progress_fill};
  border-radius: 4px;
  min-height: 4px;
}}

/* ── Scrollbars ── */
scrollbar {{
  background: transparent;
}}

scrollbar slider {{
  background-color: {scrollbar_slider};
  border-radius: 10px;
}}

scrollbar slider:hover {{
  background-color: {scrollbar_slider_hover};
}}

/* ── Sliders & Scales ── */
scale highlight {{
  background-color: {slider_highlight};
}}

/* ── Buttons & Action States ── */
.suggested-action {{
  background-color: {suggested_action_bg};
  color: {suggested_action_fg};
  border: 1px solid {suggested_action_bg};
  border-radius: 8px;
  padding: 6px 18px;
  font-weight: bold;
}}

.destructive-action {{
  background-color: {destructive_action_bg};
  color: {destructive_action_fg};
  border-radius: 8px;
  padding: 8px 16px;
  font-weight: bold;
}}

.error {{
  border: 1px solid {error_color};
  color: {error_color};
}}

.warning {{
  border: 1px solid {warning_color};
  color: {warning_color};
}}

.success {{
  color: {success_color};
}}

/* ── Quick Panel ── */
.quick-panel-scroll {{
  background-color: {quick_panel_bg};
  border-top: 1px solid {border_color};
}}

.quick-panel button {{
  background-color: {button_bg};
  color: {button_fg};
  border: 1px solid {border_color};
  border-radius: 6px;
  padding: 4px 12px;
}}

.quick-panel button:hover {{
  background-color: {hover_bg};
  border-color: {accent};
}}

.quick-panel button:checked,
.quick-panel button.suggested-action {{
  background-color: {selected_bg};
  border-color: {selected_bg};
  color: {selected_fg};
}}

.quick-panel button:checked label,
.quick-panel button.suggested-action label {{
  color: {selected_fg};
  font-weight: bold;
}}
"""


class ThemeBuilderWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app)
        self.set_title("Flux Theme Studio")
        self.set_default_size(720, 780)

        os.makedirs(THEMES_DIR, exist_ok=True)
        self.color_widgets = {}

        # Wrap in ToastOverlay for auto-dismissing notifications
        self.toast_overlay = Adw.ToastOverlay()
        self.set_content(self.toast_overlay)

        root_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.toast_overlay.set_child(root_box)

        # HeaderBar
        header = Adw.HeaderBar()
        save_btn = Gtk.Button(label="Export Theme")
        save_btn.add_css_class("suggested-action")
        save_btn.connect("clicked", self._on_export)
        header.pack_end(save_btn)
        root_box.append(header)

        # Navigation View Stack
        view_stack = Adw.ViewStack()
        view_stack.set_vexpand(True)

        # Pages
        page_surfaces = self._build_surfaces_page()
        view_stack.add_titled_with_icon(
            page_surfaces, "surfaces", "Surfaces", "applications-graphics-symbolic"
        )

        page_states = self._build_states_page()
        view_stack.add_titled_with_icon(
            page_states, "states", "States / Accents", "emblem-favorite-symbolic"
        )

        page_widgets = self._build_widgets_page()
        view_stack.add_titled_with_icon(
            page_widgets, "widgets", "Widgets", "view-grid-symbolic"
        )

        page_typography = self._build_typography_page()
        view_stack.add_titled_with_icon(
            page_typography, "typography", "Typography", "font-x-generic-symbolic"
        )

        page_metrics = self._build_metrics_page()
        view_stack.add_titled_with_icon(
            page_metrics, "metrics", "Metrics", "preferences-system-windows-symbolic"
        )

        switcher = Adw.ViewSwitcherBar()
        switcher.set_stack(view_stack)
        switcher.set_reveal(True)

        root_box.append(view_stack)
        root_box.append(switcher)

    def _create_color_row(self, group, title, default_val, key):
        rgba = Gdk.RGBA()
        rgba.parse(default_val)
        dialog = Gtk.ColorDialog(with_alpha=True)
        btn = Gtk.ColorDialogButton(dialog=dialog, rgba=rgba)
        btn.set_valign(Gtk.Align.CENTER)

        row = Adw.ActionRow(title=title)
        row.add_suffix(btn)
        group.add(row)

        self.color_widgets[key] = btn
        return btn

    def _build_surfaces_page(self):
        page = Adw.PreferencesPage()

        # Preset Selector
        preset_group = Adw.PreferencesGroup(title="Theme Presets")
        preset_names = list(PRESETS.keys())
        self.preset_row = Adw.ComboRow(title="Base Preset")
        self.preset_row.set_model(Gtk.StringList.new(preset_names))
        self.preset_row.connect("notify::selected", self._on_preset_selected)
        preset_group.add(self.preset_row)
        page.add(preset_group)

        # Identity
        id_group = Adw.PreferencesGroup(title="Theme Identity")
        self.name_entry = Gtk.Entry(text="custom-theme", hexpand=True)
        name_row = Adw.ActionRow(
            title="Theme Identifier", activatable_widget=self.name_entry
        )
        name_row.add_suffix(self.name_entry)
        id_group.add(name_row)
        page.add(id_group)

        # Primary Surfaces
        surf_group = Adw.PreferencesGroup(title="Window &amp; Structural Surfaces")
        self._create_color_row(surf_group, "Window Background", "#121212", "window_bg")
        self._create_color_row(
            surf_group, "HeaderBar Background", "#181818", "header_bg"
        )
        self._create_color_row(
            surf_group, "Sidebar Background", "#141414", "sidebar_bg"
        )
        self._create_color_row(surf_group, "Card Surface", "#1e1e1e", "card_bg")
        self._create_color_row(
            surf_group, "Input Entry Background", "#1e1e1e", "entry_bg"
        )
        self._create_color_row(
            surf_group, "Popover / Menu Background", "#1a1a1a", "popover_bg"
        )
        self._create_color_row(
            surf_group,
            "Quick Panel Background",
            "rgba(18, 18, 18, 0.75)",
            "quick_panel_bg",
        )
        self._create_color_row(
            surf_group, "Status Bar Background", "rgba(18, 18, 18, 0.95)", "status_bg"
        )
        page.add(surf_group)

        # Borders & Separators
        border_group = Adw.PreferencesGroup(title="Borders &amp; Dividers")
        self._create_color_row(
            border_group, "Global Border", "rgba(255, 255, 255, 0.08)", "border_color"
        )
        self._create_color_row(
            border_group, "Card Border", "rgba(255, 255, 255, 0.08)", "card_border"
        )
        self._create_color_row(
            border_group,
            "Separator Color",
            "rgba(255, 255, 255, 0.08)",
            "separator_color",
        )
        page.add(border_group)

        return page

    def _build_states_page(self):
        page = Adw.PreferencesPage()

        # Text Colors
        fg_group = Adw.PreferencesGroup(title="Text &amp; Foreground")
        self._create_color_row(fg_group, "Primary Text Color", "#e5e5e5", "fg_color")
        self._create_color_row(
            fg_group, "Subtext / Secondary Color", "#888888", "dim_fg"
        )
        page.add(fg_group)

        # Accent & Selection
        accent_group = Adw.PreferencesGroup(title="Accents &amp; Focus")
        self._create_color_row(
            accent_group, "Accent / Focus Color", "#007aff", "accent"
        )
        self._create_color_row(
            accent_group, "Accent Foreground", "#ffffff", "accent_fg"
        )
        self._create_color_row(
            accent_group, "Hover Background", "rgba(255, 255, 255, 0.08)", "hover_bg"
        )
        self._create_color_row(
            accent_group, "Selection Background", "#007aff", "selected_bg"
        )
        self._create_color_row(
            accent_group, "Selection Foreground", "#ffffff", "selected_fg"
        )
        page.add(accent_group)

        # Semantic Feedback
        feedback_group = Adw.PreferencesGroup(title="Semantic Indicators")
        self._create_color_row(
            feedback_group, "Success Color", "#34c759", "success_color"
        )
        self._create_color_row(
            feedback_group, "Warning Color", "#ff9500", "warning_color"
        )
        self._create_color_row(feedback_group, "Error Color", "#ff3b30", "error_color")
        page.add(feedback_group)

        return page

    def _build_widgets_page(self):
        page = Adw.PreferencesPage()

        # Breadcrumbs
        bc_group = Adw.PreferencesGroup(title="Breadcrumb Bar")
        self._create_color_row(
            bc_group, "Segment Background", "rgba(255, 255, 255, 0.06)", "breadcrumb_bg"
        )
        self._create_color_row(
            bc_group, "Active Segment Background", "#007aff", "breadcrumb_active_bg"
        )
        self._create_color_row(
            bc_group, "Active Segment Text", "#ffffff", "breadcrumb_active_fg"
        )
        page.add(bc_group)

        # Buttons & Controls
        btn_group = Adw.PreferencesGroup(title="Buttons &amp; Actions")
        self._create_color_row(
            btn_group,
            "Default Button Background",
            "rgba(255, 255, 255, 0.05)",
            "button_bg",
        )
        self._create_color_row(btn_group, "Default Button Text", "#e5e5e5", "button_fg")
        self._create_color_row(
            btn_group, "Suggested Action Background", "#007aff", "suggested_action_bg"
        )
        self._create_color_row(
            btn_group, "Suggested Action Text", "#ffffff", "suggested_action_fg"
        )
        self._create_color_row(
            btn_group,
            "Destructive Action Background",
            "rgba(255, 59, 48, 0.15)",
            "destructive_action_bg",
        )
        self._create_color_row(
            btn_group, "Destructive Action Text", "#ff3b30", "destructive_action_fg"
        )
        page.add(btn_group)

        # Progress & Sliders
        prog_group = Adw.PreferencesGroup(title="Progress, Sliders &amp; Scrollbars")
        self._create_color_row(prog_group, "Progress Fill", "#007aff", "progress_fill")
        self._create_color_row(
            prog_group,
            "Progress Trough",
            "rgba(255, 255, 255, 0.05)",
            "progress_trough",
        )
        self._create_color_row(
            prog_group,
            "Scrollbar Slider",
            "rgba(255, 255, 255, 0.2)",
            "scrollbar_slider",
        )
        self._create_color_row(
            prog_group,
            "Scrollbar Slider (Hover)",
            "rgba(255, 255, 255, 0.4)",
            "scrollbar_slider_hover",
        )
        self._create_color_row(
            prog_group, "Scale / Slider Highlight", "#007aff", "slider_highlight"
        )
        page.add(prog_group)

        return page

    def _build_typography_page(self):
        page = Adw.PreferencesPage()
        group = Adw.PreferencesGroup(title="Font Configuration")

        font_dialog = Gtk.FontDialog()
        self.font_main_btn = Gtk.FontDialogButton(dialog=font_dialog)
        self.font_main_btn.set_font_desc(Pango.FontDescription.from_string("Inter 10"))
        row_main = Adw.ActionRow(
            title="Interface Font", subtitle="File labels, navigation, titles"
        )
        row_main.add_suffix(self.font_main_btn)
        group.add(row_main)

        self.font_mono_btn = Gtk.FontDialogButton(dialog=font_dialog)
        self.font_mono_btn.set_font_desc(
            Pango.FontDescription.from_string("JetBrains Mono 10")
        )
        row_mono = Adw.ActionRow(
            title="Monospace Font", subtitle="Terminal, status bar metadata"
        )
        row_mono.add_suffix(self.font_mono_btn)
        group.add(row_mono)
        page.add(group)

        spacing_group = Adw.PreferencesGroup(title="Letter Spacing")
        self.spacing_spin = Adw.SpinRow.new_with_range(0.0, 3.0, 0.1)
        self.spacing_spin.set_title("Global Letter Spacing (px)")
        self.spacing_spin.set_value(0.2)
        spacing_group.add(self.spacing_spin)
        page.add(spacing_group)

        return page

    def _build_metrics_page(self):
        page = Adw.PreferencesPage()
        group = Adw.PreferencesGroup(title="Card &amp; Widget Geometry")

        self.radius_spin = Adw.SpinRow.new_with_range(0, 32, 1)
        self.radius_spin.set_title("Card Border Radius (px)")
        self.radius_spin.set_value(12)
        group.add(self.radius_spin)

        self.padding_spin = Adw.SpinRow.new_with_range(4, 28, 1)
        self.padding_spin.set_title("Card Internal Padding (px)")
        self.padding_spin.set_value(10)
        group.add(self.padding_spin)

        page.add(group)
        return page

    def _on_preset_selected(self, row, _):
        selected_name = list(PRESETS.keys())[row.get_selected()]
        preset = PRESETS[selected_name]

        for k, btn in self.color_widgets.items():
            if k in preset:
                rgba = Gdk.RGBA()
                rgba.parse(preset[k])
                btn.set_rgba(rgba)

        self.radius_spin.set_value(preset["card_radius"])
        self.padding_spin.set_value(preset["card_padding"])
        self.spacing_spin.set_value(preset["letter_spacing"])
        self.font_main_btn.set_font_desc(
            Pango.FontDescription.from_string(preset["font_main"])
        )
        self.font_mono_btn.set_font_desc(
            Pango.FontDescription.from_string(preset["font_mono"])
        )

    def _collect_config(self):
        font_main = self.font_main_btn.get_font_desc().get_family() or "Inter"
        font_mono = self.font_mono_btn.get_font_desc().get_family() or "JetBrains Mono"

        config = {
            k: btn.get_rgba().to_string() for k, btn in self.color_widgets.items()
        }
        config.update(
            {
                "font_main_family": font_main,
                "font_mono_family": font_mono,
                "letter_spacing": self.spacing_spin.get_value(),
                "header_spacing": self.spacing_spin.get_value() * 4.0,
                "card_radius": int(self.radius_spin.get_value()),
                "card_padding": int(self.padding_spin.get_value()),
            }
        )
        return config

    def _on_export(self, _):
        theme_name = self.name_entry.get_text().strip().lower().replace(" ", "-")
        if not theme_name:
            theme_name = "custom-theme"

        config = self._collect_config()
        css_data = CSS_TEMPLATE.format(**config)
        out_path = os.path.join(THEMES_DIR, f"{theme_name}.css")

        try:
            with open(out_path, "w") as f:
                f.write(css_data)
            toast = Adw.Toast.new(f"Theme saved to {out_path}")
            toast.set_timeout(3)
            self.toast_overlay.add_toast(toast)
        except Exception as e:
            toast = Adw.Toast.new(f"Failed to export theme: {e}")
            toast.set_timeout(4)
            self.toast_overlay.add_toast(toast)


def on_activate(app):
    win = ThemeBuilderWindow(app)
    win.present()


if __name__ == "__main__":
    app = Adw.Application(application_id=None, flags=Gio.ApplicationFlags.FLAGS_NONE)
    app.connect("activate", on_activate)
    app.run(None)
