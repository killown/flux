# Actions

Flux's context menus are defined by `menu.rs` (and `menus/menu-*.rs`) files in `~/.config/flux/`. Each line is:

```text
"Label" => "mime_types", "command"[, "toast"][, "no_command_dialog"]
```

`command` is either a shell command or one of the `builtin::*` actions listed below.

## Placeholders

Shell commands may contain the following placeholders. Each is substituted by `build_execution_command` before the command is run, and the substituted value is shell-quoted:

| Placeholder | Meaning |
|---|---|
| `%p` | Absolute path of the selected item |
| `%d` | Parent directory of the selected item |
| `%f` | Filename of the selected item |
| `%l` | Line number of the current content-search hit (0 otherwise) |
| `%cwd` | Current working directory |

## Builtin actions

The following actions are recognised when `command` starts with `builtin::`. They are handled directly by the application and never spawn a shell.

| Action | Scope | Description |
|---|---|---|
| `builtin::copy` | `all` | Copies the selected files or folders to the clipboard. |
| `builtin::cut` | `all` | Cuts the selected files or folders to the clipboard. |
| `builtin::paste` | `directory` | Pastes items from the clipboard into the current directory. |
| `builtin::open_new_tab` | `directory` | Opens the selected folder or current location in a new tab. |
| `builtin::rename` | `all` | Triggers inline filename renaming on the selected item. |
| `builtin::delete` | `all` | Moves the selected files or folders to the system trash. |
| `builtin::new_folder` | `directory` | Opens the new folder creation prompt. |
| `builtin::new_file` | `directory` | Opens the new file creation prompt. |
| `builtin::toggle_pin` | `directory` | Pins or unpins the target folder in the sidebar. |
| `builtin::add_to_quick_list` | `all` | Adds the selected folder or current directory to the Quick List. |
| `builtin::quick_list_transfer` | `all` | Opens a submenu to move or copy items directly to Quick List slots. |
| `builtin::tagfile` | `file` | Opens the file tag editor popover. |
| `builtin::inspect_dir` | `directory` | Opens the Directory Inspector with disk usage and largest files. |
| `builtin::install_fonts` | `font/all` | Installs the selected font files into ~/.local/share/fonts. |
| `builtin::open_with` | `file` | Populates a dynamic submenu of registered applications for the MIME type. |
| `builtin::open_with_dialog` | `file` | Opens the full system application chooser dialog. |
| `builtin::select_folder_icon` | `directory` | Opens the folder icon picker to assign a theme icon. |
| `builtin::set_custom_icon` | `all` | Opens a file chooser to assign a custom image as file/folder icon. |
| `builtin::reset_custom_icon` | `all` | Restores the default icon for the selected file or folder. |
| `builtin::set_extension_icon` | `file` | Sets a custom image icon for the target file's extension globally. |
| `builtin::reset_extension_icon` | `file` | Resets the custom icon for the target extension back to theme default. |
| `builtin::set_bg_window` | `image/all` | Sets the selected image as the main window background. |
| `builtin::set_bg_sidebar_left` | `image/all` | Sets the selected image as the left sidebar background. |
| `builtin::set_bg_sidebar_right` | `image/all` | Sets the selected image as the right panel background. |
| `builtin::clear_backgrounds` | `all` | Clears all custom background images and resets styling to default. |

## Worked examples

Flat action - an entry that appears in the top-level context menu:

```text
"Open in Terminal" => "directory", "alacritty --working-directory=%p", "no_command_dialog"
```

Submenu - the part before `>` becomes a submenu group:

```text
"Image Convert > To PNG" => "image/webp", "magick %p %p.png"
```

Placeholder plus toast - `%f` is substituted, then the toast is shown after the command exits:

```text
"Optimise Image" => "image/png", "oxipng %p", "Image optimised"
```
