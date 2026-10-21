# CSS variables

Flux styles its interface with a built-in stylesheet (`src/ui/style.css`). Two kinds of variables are available to it, and to your own CSS:

- **Flux variables** - custom properties such as `--shadow`, declared once and read with `var(--name)`.
- **GTK and Adwaita colors** - named colors such as `@accent_bg_color`, provided by the toolkit and read with `@name`.

## Overriding

Put overrides in `~/.config/flux/style.css`. It loads on top of the active theme, always wins, and is reloaded when the file changes. To retune a Flux variable, redeclare it in `:root`:

```css
:root {
  --font-mono: "Fira Code", monospace;
  --shadow: alpha(black, 0.4);
}
```

## Flux variables

Declared in the built-in stylesheet. **Uses** counts the `var()` reads in that file.

<!-- BEGIN GENERATED: custom-properties -->
| Variable | Default | Declared in | Uses | Purpose |
|---|---|---|---|---|
| `--border-color` | `alpha(@theme_fg_color, 0.1)` | `:root` | 17 | Hairline borders on the sidebar, list rows, inputs, tooltips and toasts. |
| `--disabled-opacity` | `0.4` | `:root` | 4 | Opacity of disabled controls and of items being dragged. |
| `--font-main` | `inherit` | `:root` | 8 | Font family for interface text; `inherit` follows the system font. |
| `--font-mono` | `"JetBrains Mono", monospace` | `:root` | 9 | Monospace font family for list info, key caps, git badges and status text. |
| `--shadow` | `alpha(black, 0.25)` | `:root` | 3 | Drop-shadow color of cards, popover menus and the selected sidebar row. |
| `--shadow-hover` | `alpha(black, 0.35)` | `:root` | 1 | Drop-shadow color of a hovered card. |
<!-- END GENERATED: custom-properties -->

## GTK and Adwaita colors

These follow the system light or dark preference and accent color, so prefer them over hard-coded colors. **Uses** counts the references in the built-in stylesheet.

### Named colors

<!-- BEGIN GENERATED: gtk-colors -->
| Color | Role | Uses |
|---|---|---|
| `@accent_bg_color` | Accent background for selected and active elements. | 25 |
| `@accent_fg_color` | Text and icons drawn on top of `@accent_bg_color`. | 37 |
| `@accent_color` | Accent for text, icons and borders on a plain background. | 47 |
| `@window_bg_color` | Window background. | 6 |
| `@window_fg_color` | Window text and icons. | 13 |
| `@theme_fg_color` | Default foreground; also the base for translucent text, borders and hover tints. | 61 |
| `@headerbar_bg_color` | Header bar background. | 2 |
| `@headerbar_fg_color` | Header bar text and icons. | 9 |
| `@popover_bg_color` | Popover background; the quick panel uses it. | 1 |
| `@popover_fg_color` | Popover text; also the base for translucent tints on popovers. | 12 |
| `@sidebar_bg_color` | Sidebar background. | 1 |
| `@card_bg_color` | Background of file cards. | 1 |
<!-- END GENERATED: gtk-colors -->

### Palette colors

Fixed GNOME palette colors. Unlike the named colors above, they do not change with the theme.

<!-- BEGIN GENERATED: palette-colors -->
| Color | Role | Uses |
|---|---|---|
| `@red_3` | Errors and destructive actions: broken symlinks, delete buttons, window close. | 22 |
| `@orange_3` | Warnings and restricted items: locked files, cut items, git conflicts. | 9 |
| `@green_4` | Success: full level bars and git added. | 4 |
| `@blue_3` | Git modified and mixed. | 2 |
| `@purple_3` | Git staged. | 2 |
| `@dark_2` | Git untracked. | 2 |
<!-- END GENERATED: palette-colors -->
