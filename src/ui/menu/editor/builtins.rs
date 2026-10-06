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
