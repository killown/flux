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
    BuiltinDef {
        name: "builtin::forget_archive_password",
        scope: "all",
        desc: "Deletes the archive password saved in the system keyring for the target archive.",
    },
];

/// `(placeholder, one target, several targets)` rows of the placeholder table in `docs/actions.md`.
const PLACEHOLDERS: &[(&str, &str, &str)] = &[
    (
        "%p",
        "Absolute path of the item",
        "All the paths, shell-quoted and separated by spaces",
    ),
    (
        "%d",
        "Parent directory of the item",
        "The current directory",
    ),
    ("%f", "File name of the item", "Not substituted"),
    (
        "%l",
        "Line number of the content-search hit, `0` when the item is not a hit",
        "`0`",
    ),
];

// ── Documentation generator ─────────────────────────────────────────────────

/// Renders the `builtin::*` reference that ships as `docs/actions.md`.
///
/// Kept next to `BUILTINS` so the table can never drift from the source:
/// `doc_is_in_sync` fails if the two disagree.
#[allow(dead_code)] // used by `src/bin/gen-docs.rs`, not linked by the `flux-fm` binary alone
pub fn render_actions_markdown() -> String {
    use std::fmt::Write as _;

    let mut out = String::new();

    out.push_str("# Actions\n\n");
    out.push_str(
        "The right-click menu is defined by `~/.config/flux/menu.rs`. \
         Ctrl+right-click and the contextual actions panel read a `menu-*.rs` \
         template instead, chosen by the MIME type of the item: \
         `menu-<category>-<subtype>.rs`, else `menu-<category>-all.rs`, else \
         `menu-all.rs`. Templates are looked up in `~/.config/flux/menus/` \
         first, then in `~/.config/flux/`. Every line of these files is:\n\n\
         ```text\n\
         \"Label\" => \"mime_types\", \"command\"[, \"toast\"][, \"no_command_dialog\"]\n\
         ```\n\n\
         `command` is either a shell command or one of the `builtin::*` actions \
         listed below.\n\n",
    );

    out.push_str("## Placeholders\n\n");
    out.push_str(
        "Shell commands may contain the following placeholders. Which one is \
         used depends on how many items the command targets: the selection, \
         or the item the menu was opened on when it is not selected, or the \
         current directory when there is neither.\n\n",
    );
    out.push_str("| Placeholder | One target | Several targets |\n");
    out.push_str("|---|---|---|\n");
    for (token, one, many) in PLACEHOLDERS {
        let _ = writeln!(out, "| `{}` | {} | {} |", token, one, many);
    }
    out.push_str(
        "\nEvery substituted path is wrapped in single quotes, with any `'` \
         inside it escaped. A `\"%p\"` or `'%p'` written in the command is \
         replaced together with its quotes, so don't add your own. If a path \
         contains a newline, a carriage return or a NUL byte, the command is \
         not run and Flux shows a toast.\n\n\
         A menu entry whose command contains `%l` is shown only for a \
         content-search hit, and never in the contextual actions panel.\n\n\
         The toast of a `menu.rs` shell command is shown after it exits, and \
         a failed command without a toast shows `Command failed`. An entry \
         from a `menu-*.rs` template shows its toast as soon as it is \
         clicked.\n\n",
    );

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
         Placeholder plus toast - `%p` is substituted, then the toast is \
         shown after the command exits:\n\n\
         ```text\n\
         \"Optimise Image\" => \"image/png\", \"oxipng %p\", \"Image optimised\"\n\
         ```\n",
    );

    out
}
