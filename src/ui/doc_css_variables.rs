//! Renderer for `docs/css-variables.md`.

#[cfg(test)]
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The built-in stylesheet, the single source for every generated row.
const STYLE_CSS: &str = include_str!("style.css");

/// Purpose of each Flux custom property declared in `style.css`.
const CUSTOM_PROPERTY_NOTES: &[(&str, &str)] = &[
    (
        "--border-color",
        "Hairline borders on the sidebar, list rows, inputs, tooltips and toasts.",
    ),
    (
        "--disabled-opacity",
        "Opacity of disabled controls and of items being dragged.",
    ),
    (
        "--font-main",
        "Font family for interface text; `inherit` follows the system font.",
    ),
    (
        "--font-mono",
        "Monospace font family for list info, key caps, git badges and status text.",
    ),
    (
        "--shadow",
        "Drop-shadow color of cards, popover menus and the selected sidebar row.",
    ),
    ("--shadow-hover", "Drop-shadow color of a hovered card."),
];

/// GTK and libadwaita named colors Flux reads, with the role each plays.
const GTK_COLORS: &[(&str, &str)] = &[
    (
        "@accent_bg_color",
        "Accent background for selected and active elements.",
    ),
    (
        "@accent_fg_color",
        "Text and icons drawn on top of `@accent_bg_color`.",
    ),
    (
        "@accent_color",
        "Accent for text, icons and borders on a plain background.",
    ),
    ("@window_bg_color", "Window background."),
    ("@window_fg_color", "Window text and icons."),
    (
        "@theme_fg_color",
        "Default foreground; also the base for translucent text, borders and hover tints.",
    ),
    ("@headerbar_bg_color", "Header bar background."),
    ("@headerbar_fg_color", "Header bar text and icons."),
    (
        "@popover_bg_color",
        "Popover background; the quick panel uses it.",
    ),
    (
        "@popover_fg_color",
        "Popover text; also the base for translucent tints on popovers.",
    ),
    ("@sidebar_bg_color", "Sidebar background."),
    ("@card_bg_color", "Background of file cards."),
];

/// GNOME palette colors Flux reads, with the role each plays.
const PALETTE_COLORS: &[(&str, &str)] = &[
    (
        "@red_3",
        "Errors and destructive actions: broken symlinks, delete buttons, window close.",
    ),
    (
        "@orange_3",
        "Warnings and restricted items: locked files, cut items, git conflicts.",
    ),
    ("@green_4", "Success: full level bars and git added."),
    ("@blue_3", "Git modified and mixed."),
    ("@purple_3", "Git staged."),
    ("@dark_2", "Git untracked."),
];

/// A custom property declaration found in a stylesheet.
struct CustomProperty {
    name: String,
    value: String,
    selector: String,
}

/// Removes `/* ... */` comments, leaving a space in their place.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = ' ';
            for n in chars.by_ref() {
                if prev == '*' && n == '/' {
                    break;
                }
                prev = n;
            }
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Joins all whitespace runs into single spaces.
fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Records `buf` as a custom property when it is a `--name: value` declaration.
fn push_declaration(buf: &str, selector: &str, props: &mut Vec<CustomProperty>) {
    let Some(rest) = buf.trim().strip_prefix("--") else {
        return;
    };
    let Some((name, value)) = rest.split_once(':') else {
        return;
    };
    let name = name.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return;
    }
    props.push(CustomProperty {
        name: format!("--{name}"),
        value: collapse(value),
        selector: selector.to_string(),
    });
}

/// Returns every `--name: value;` declaration in `css`, sorted by name.
fn extract_custom_properties(css: &str) -> Vec<CustomProperty> {
    let css = strip_comments(css);
    let mut props = Vec::new();
    let mut selectors: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    let mut parens = 0usize;

    for c in css.chars() {
        if let Some(q) = quote {
            buf.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                buf.push(c);
            }
            '(' => {
                parens += 1;
                buf.push(c);
            }
            ')' => {
                parens = parens.saturating_sub(1);
                buf.push(c);
            }
            '{' if parens == 0 => {
                selectors.push(collapse(&buf));
                buf.clear();
            }
            '}' if parens == 0 => {
                let selector = selectors.last().map_or("", String::as_str);
                push_declaration(&buf, selector, &mut props);
                buf.clear();
                selectors.pop();
            }
            ';' if parens == 0 => {
                let selector = selectors.last().map_or("", String::as_str);
                push_declaration(&buf, selector, &mut props);
                buf.clear();
            }
            _ => buf.push(c),
        }
    }

    props.sort_by(|a, b| a.name.cmp(&b.name));
    props
}

/// Counts `var(--name)` reads of `name`, ignoring longer names with the same prefix.
fn count_var_uses(css: &str, name: &str) -> usize {
    let needle = format!("var({name}");
    css.match_indices(&needle)
        .filter(|(i, _)| {
            !css[i + needle.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .count()
}

/// Returns every `@snake_case` color reference in `css`, at-rules excluded.
#[cfg(test)]
fn extract_color_refs(css: &str) -> BTreeSet<String> {
    css.match_indices('@')
        .filter_map(|(i, _)| {
            let ident: String = css[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            ident.contains('_').then(|| format!("@{ident}"))
        })
        .collect()
}

/// Counts reads of the named color `name` (with its `@`), ignoring longer names.
fn count_color_uses(css: &str, name: &str) -> usize {
    css.match_indices(name)
        .filter(|(i, _)| {
            !css[i + name.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .count()
}

/// Escapes characters that would break a Markdown table cell.
fn cell(s: &str) -> String {
    s.replace('|', "\\|")
}

/// Renders one hand-written color table with generated use counts.
fn render_color_table(out: &mut String, css: &str, colors: &[(&str, &str)]) {
    out.push_str("| Color | Role | Uses |\n");
    out.push_str("|---|---|---|\n");
    for (name, role) in colors {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            name,
            role,
            count_color_uses(css, name)
        );
    }
}

/// Returns the complete `docs/css-variables.md`, prose and generated tables.
#[allow(dead_code)] // used by `src/bin/gen-docs.rs`, not linked by the `flux-fm` binary alone
pub fn render_css_variables_markdown() -> String {
    render(STYLE_CSS)
}

/// Renders the page for the given stylesheet text.
fn render(raw_css: &str) -> String {
    let css = strip_comments(raw_css);
    let mut out = String::new();

    out.push_str("# CSS variables\n\n");
    out.push_str(
        "Flux styles its interface with a built-in stylesheet \
         (`src/ui/style.css`). Two kinds of variables are available to it, and \
         to your own CSS:\n\n\
         - **Flux variables** - custom properties such as `--shadow`, declared \
         once and read with `var(--name)`.\n\
         - **GTK and Adwaita colors** - named colors such as \
         `@accent_bg_color`, provided by the toolkit and read with `@name`.\n\n",
    );

    out.push_str("## Overriding\n\n");
    out.push_str(
        "Put overrides in `~/.config/flux/style.css`. It loads on top of the \
         active theme, always wins, and is reloaded when the file changes. To \
         retune a Flux variable, redeclare it in `:root`:\n\n\
         ```css\n\
         :root {\n\
         \x20 --font-mono: \"Fira Code\", monospace;\n\
         \x20 --shadow: alpha(black, 0.4);\n\
         }\n\
         ```\n\n",
    );

    out.push_str("## Flux variables\n\n");
    out.push_str(
        "Declared in the built-in stylesheet. **Uses** counts the `var()` \
         reads in that file.\n\n",
    );
    out.push_str("<!-- BEGIN GENERATED: custom-properties -->\n");
    out.push_str("| Variable | Default | Declared in | Uses | Purpose |\n");
    out.push_str("|---|---|---|---|---|\n");
    for prop in extract_custom_properties(&css) {
        let note = CUSTOM_PROPERTY_NOTES
            .iter()
            .find(|(name, _)| *name == prop.name)
            .map_or("", |(_, note)| *note);
        let selector = if prop.selector.is_empty() {
            "top level".to_string()
        } else {
            format!("`{}`", cell(&prop.selector))
        };
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} | {} | {} |",
            prop.name,
            cell(&prop.value),
            selector,
            count_var_uses(&css, &prop.name),
            note
        );
    }
    out.push_str("<!-- END GENERATED: custom-properties -->\n\n");

    out.push_str("## GTK and Adwaita colors\n\n");
    out.push_str(
        "These follow the system light or dark preference and accent color, so \
         prefer them over hard-coded colors. **Uses** counts the references in \
         the built-in stylesheet.\n\n",
    );

    out.push_str("### Named colors\n\n");
    out.push_str("<!-- BEGIN GENERATED: gtk-colors -->\n");
    render_color_table(&mut out, &css, GTK_COLORS);
    out.push_str("<!-- END GENERATED: gtk-colors -->\n\n");

    out.push_str("### Palette colors\n\n");
    out.push_str(
        "Fixed GNOME palette colors. Unlike the named colors above, they do \
         not change with the theme.\n\n",
    );
    out.push_str("<!-- BEGIN GENERATED: palette-colors -->\n");
    render_color_table(&mut out, &css, PALETTE_COLORS);
    out.push_str("<!-- END GENERATED: palette-colors -->\n");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_is_in_sync() {
        let generated = render_css_variables_markdown();
        let committed = std::fs::read_to_string("docs/css-variables.md").expect(
            "docs/css-variables.md is missing; create it from render_css_variables_markdown()",
        );
        assert_eq!(
            generated, committed,
            "\ndocs/css-variables.md is stale.\n\
             Run `cargo run --bin gen-docs` to regenerate it."
        );
    }

    #[test]
    fn extracts_declarations_with_their_selector() {
        let css = ":root { --a: 1; --b: \"x; y\", mono; }\n.card { --c: alpha(@fg, 0.5) }";
        let props = extract_custom_properties(css);
        let got: Vec<_> = props
            .iter()
            .map(|p| (p.name.as_str(), p.value.as_str(), p.selector.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("--a", "1", ":root"),
                ("--b", "\"x; y\", mono", ":root"),
                ("--c", "alpha(@fg, 0.5)", ".card"),
            ]
        );
    }

    #[test]
    fn ignores_comments_and_var_reads() {
        let css = "/* --ghost: 1; */ .a { color: var(--a); margin: 0; }";
        assert!(extract_custom_properties(css).is_empty());
    }

    #[test]
    fn var_use_count_ignores_longer_names() {
        let css = ".a { x: var(--shadow); y: var(--shadow-hover); z: var(--shadow, 0); }";
        assert_eq!(count_var_uses(css, "--shadow"), 2);
        assert_eq!(count_var_uses(css, "--shadow-hover"), 1);
    }

    #[test]
    fn color_refs_skip_at_rules() {
        let css = "@media (x) { .a { color: @accent_bg_color; fill: @red_3; } } @keyframes k {}";
        let refs: Vec<_> = extract_color_refs(css).into_iter().collect();
        assert_eq!(refs, ["@accent_bg_color", "@red_3"]);
    }

    #[test]
    fn every_custom_property_has_a_note() {
        let props = extract_custom_properties(STYLE_CSS);
        for prop in &props {
            assert!(
                CUSTOM_PROPERTY_NOTES.iter().any(|(n, _)| *n == prop.name),
                "{} is declared in style.css but has no entry in CUSTOM_PROPERTY_NOTES",
                prop.name
            );
        }
        for (name, _) in CUSTOM_PROPERTY_NOTES {
            assert!(
                props.iter().any(|p| p.name == *name),
                "{name} is in CUSTOM_PROPERTY_NOTES but not declared in style.css"
            );
        }
    }

    #[test]
    fn color_tables_match_the_stylesheet() {
        let css = strip_comments(STYLE_CSS);
        let documented: BTreeSet<&str> = GTK_COLORS
            .iter()
            .chain(PALETTE_COLORS)
            .map(|(name, _)| *name)
            .collect();

        for used in extract_color_refs(&css) {
            assert!(
                documented.contains(used.as_str()),
                "{used} is used in style.css but missing from GTK_COLORS or PALETTE_COLORS"
            );
        }
        for name in &documented {
            assert!(
                count_color_uses(&css, name) > 0,
                "{name} is documented but no longer used in style.css"
            );
        }
    }
}
