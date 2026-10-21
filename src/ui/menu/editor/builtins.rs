pub(super) struct BuiltinDef {
    pub(super) name: &'static str,
    pub(super) scope: &'static str,
    pub(super) desc: &'static str,
}

pub(super) const BUILTINS: &[BuiltinDef] = &[
    BuiltinDef {
        name: "builtin::copy",
        scope: "all",
        desc: "Copies the selected files or folders to the clipboard.",
    },
    BuiltinDef {
        name: "builtin::cut",
        scope: "all",
        desc: "Cuts the selected files or folders to the clipboard.",
    },
    BuiltinDef {
        name: "builtin::paste",
        scope: "directory",
        desc: "Pastes items from the clipboard into the current directory.",
    },
    BuiltinDef {
        name: "builtin::open_new_tab",
        scope: "directory",
        desc: "Opens the selected folder or current location in a new tab.",
    },
    BuiltinDef {
        name: "builtin::rename",
        scope: "all",
        desc: "Triggers inline filename renaming on the selected item.",
    },
    BuiltinDef {
        name: "builtin::delete",
        scope: "all",
        desc: "Moves the selected files or folders to the system trash.",
    },
    BuiltinDef {
        name: "builtin::new_folder",
        scope: "directory",
        desc: "Opens the new folder creation prompt.",
    },
    BuiltinDef {
        name: "builtin::new_file",
        scope: "directory",
        desc: "Opens the new file creation prompt.",
    },
    BuiltinDef {
        name: "builtin::toggle_pin",
        scope: "directory",
        desc: "Pins or unpins the target folder in the sidebar.",
    },
    BuiltinDef {
        name: "builtin::add_to_quick_list",
        scope: "all",
        desc: "Adds the selected folder or current directory to the Quick List.",
    },
    BuiltinDef {
        name: "builtin::quick_list_transfer",
        scope: "all",
        desc: "Opens a submenu to move or copy items directly to Quick List slots.",
    },
    BuiltinDef {
        name: "builtin::tagfile",
        scope: "file",
        desc: "Opens the file tag editor popover.",
    },
    BuiltinDef {
        name: "builtin::inspect_dir",
        scope: "directory",
        desc: "Opens the Directory Inspector with disk usage and largest files.",
    },
    BuiltinDef {
        name: "builtin::install_fonts",
        scope: "font/all",
        desc: "Installs the selected font files into ~/.local/share/fonts.",
    },
    BuiltinDef {
        name: "builtin::open_with",
        scope: "file",
        desc: "Populates a dynamic submenu of registered applications for the MIME type.",
    },
    BuiltinDef {
        name: "builtin::open_with_dialog",
        scope: "file",
        desc: "Opens the full system application chooser dialog.",
    },
    BuiltinDef {
        name: "builtin::select_folder_icon",
        scope: "directory",
        desc: "Opens the folder icon picker to assign a theme icon.",
    },
    BuiltinDef {
        name: "builtin::set_custom_icon",
        scope: "all",
        desc: "Opens a file chooser to assign a custom image as file/folder icon.",
    },
    BuiltinDef {
        name: "builtin::reset_custom_icon",
        scope: "all",
        desc: "Restores the default icon for the selected file or folder.",
    },
    BuiltinDef {
        name: "builtin::set_extension_icon",
        scope: "file",
        desc: "Sets a custom image icon for the target file's extension globally.",
    },
    BuiltinDef {
        name: "builtin::reset_extension_icon",
        scope: "file",
        desc: "Resets the custom icon for the target extension back to theme default.",
    },
    BuiltinDef {
        name: "builtin::set_bg_window",
        scope: "image/all",
        desc: "Sets the selected image as the main window background.",
    },
    BuiltinDef {
        name: "builtin::set_bg_sidebar_left",
        scope: "image/all",
        desc: "Sets the selected image as the left sidebar background.",
    },
    BuiltinDef {
        name: "builtin::set_bg_sidebar_right",
        scope: "image/all",
        desc: "Sets the selected image as the right panel background.",
    },
    BuiltinDef {
        name: "builtin::clear_backgrounds",
        scope: "all",
        desc: "Clears all custom background images and resets styling to default.",
    },
];

// ── Documentation generator ─────────────────────────────────────────────────

/// Renders the `builtin::*` reference that ships as `docs/actions.md`.
///
/// Kept next to `BUILTINS` so the table can never drift from the source:
/// `doc_is_in_sync` fails if the two disagree.
pub fn render_actions_markdown() -> String {
    use std::fmt::Write as _;

    let mut out = String::new();

    out.push_str("# Actions\n\n");
    out.push_str(
        "Flux's context menus are defined by `menu.rs` (and `menus/menu-*.rs`) \
         files in `~/.config/flux/`. Each line is:\n\n\
         ```text\n\
         \"Label\" => \"mime_types\", \"command\"[, \"toast\"][, \"no_command_dialog\"]\n\
         ```\n\n\
         `command` is either a shell command or one of the `builtin::*` actions \
         listed below.\n\n",
    );

    out.push_str("## Placeholders\n\n");
    out.push_str(
        "Shell commands may contain the following placeholders. Each is \
         substituted by `build_execution_command` before the command is run, \
         and the substituted value is shell-quoted:\n\n",
    );
    out.push_str("| Placeholder | Meaning |\n");
    out.push_str("|---|---|\n");
    out.push_str("| `%p` | Absolute path of the selected item |\n");
    out.push_str("| `%d` | Parent directory of the selected item |\n");
    out.push_str("| `%f` | Filename of the selected item |\n");
    out.push_str("| `%l` | Line number of the current content-search hit (0 otherwise) |\n");
    out.push_str("| `%cwd` | Current working directory |\n");
    out.push('\n');

    out.push_str("## Builtin actions\n\n");
    out.push_str(
        "The following actions are recognised when `command` starts with \
         `builtin::`. They are handled directly by the application and never \
         spawn a shell.\n\n",
    );
    out.push_str("| Action | Scope | Description |\n");
    out.push_str("|---|---|---|\n");

    for b in BUILTINS {
        let desc = b.desc.replace('|', "\\|");
        let _ = writeln!(out, "| `{}` | `{}` | {} |", b.name, b.scope, desc);
    }
    out.push('\n');

    out.push_str("## Worked examples\n\n");
    out.push_str(
        "Flat action - an entry that appears in the top-level context menu:\n\n\
         ```text\n\
         \"Open in Terminal\" => \"directory\", \"alacritty --working-directory=%p\", \"no_command_dialog\"\n\
         ```\n\n\
         Submenu - the part before `>` becomes a submenu group:\n\n\
         ```text\n\
         \"Image Convert > To PNG\" => \"image/webp\", \"magick %p %p.png\"\n\
         ```\n\n\
         Placeholder plus toast - `%f` is substituted, then the toast is \
         shown after the command exits:\n\n\
         ```text\n\
         \"Optimise Image\" => \"image/png\", \"oxipng %p\", \"Image optimised\"\n\
         ```\n",
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_is_in_sync() {
        let generated = render_actions_markdown();
        let committed = std::fs::read_to_string("docs/actions.md")
            .expect("docs/actions.md is missing; create it from render_actions_markdown()");
        assert_eq!(
            generated, committed,
            "\ndocs/actions.md is stale.\n\
             Run `cargo run --bin gen-docs` to regenerate it."
        );
    }
}
