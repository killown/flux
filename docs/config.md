# Configuration

Flux reads `~/.config/flux/config.toml`. The file is created on first run and reloaded while Flux is running. If it fails to parse, Flux prints the error to stderr and runs with built-in fallback values until the file is fixed.

Every key is optional. **Default** below is the value used when a key is missing from a file that has a `[ui]` table. The file written on first run sets some keys differently, so a fresh install can differ from this table.

## Reference

<!-- BEGIN GENERATED: keys -->
### Top level

| Key | Default | Description |
|---|---|---|
| `default_list_mode` | `false` | Open folders in list mode instead of grid mode. |
| `network_bookmarks` | `[]` | Saved network locations; see [Array tables](#array-tables). |
| `sidebar` | `[]` | Custom sidebar entries; see [Array tables](#array-tables). |

### `[shortcuts]`

| Key | Default | Description |
|---|---|---|
| `back` | `unset` | Go back in navigation history. |
| `change_icon` | `unset` | Open the folder icon picker. |
| `close_tab` | `unset` | Close the current tab. |
| `copy_path` | `unset` | Copy the absolute paths of the selected items. |
| `create_hardlink` | `unset` | Create hardlinks from clipboard paths. Directories are not supported. |
| `create_symlink` | `unset` | Create symlinks from clipboard paths, auto-renaming conflicts. |
| `cycle_sort` | `unset` | Rotate through the sort fields. |
| `delete` | `unset` | Move the selected items to the trash. |
| `forward` | `unset` | Go forward in navigation history. |
| `home` | `unset` | Go to the home directory. |
| `menu_editor` | `unset` | Open the menu editor. |
| `new_tab` | `unset` | Open a new tab. |
| `next_tab` | `unset` | Switch to the next tab. |
| `open` | `unset` | Open the selected file or enter the directory. |
| `open_properties` | `unset` | Open the properties window. |
| `prev_tab` | `unset` | Switch to the previous tab. |
| `quit` | `unset` | Quit the application. |
| `refresh` | `unset` | Reload the current directory. |
| `rename` | `unset` | Start an inline rename. |
| `reset_icon` | `unset` | Reset a folder icon to the default. |
| `root` | `unset` | Go to the root directory. |
| `search` | `unset` | Focus the search interface. |
| `settings` | `unset` | Open the settings window. |
| `toggle_folders_first` | `unset` | Toggle folder grouping for the active folder. |
| `toggle_header` | `unset` | Show or hide the header bar. |
| `toggle_hidden` | `unset` | Show or hide hidden files. |
| `toggle_sort_order` | `unset` | Toggle between ascending and descending order. |

### `[ui]`

| Key | Default | Description |
|---|---|---|
| `actions_panel_width` | `380` | Width in pixels of the right-side contextual-actions panel. |
| `archive_thumbnail_cache` | `false` | Store thumbnails of files inside zip archives in a hidden `.flux-cache/` folder in the zip itself. |
| `ascending` | `true` | Sort listings in ascending order. |
| `auto_generate_mime_icons` | `true` | Generate and cache an SVG document icon when the icon theme has none. |
| `auto_mime_accent_color` | `"#1273b2"` | Accent color of auto-generated document icons. |
| `auto_mime_body_color` | `"#e4e4e4"` | Hex color of the page body of auto-generated document icons. |
| `auto_mime_font_color` | `"#ffffff"` | Hex color of the extension label inside auto-generated document icons. |
| `auto_mime_font_size` | `13.0` | Base font size of auto-generated document icons. |
| `auto_show_diff` | `true` | Open the diff panel when a modified git file is selected. |
| `autoplay_video_previews` | `true` | Play a muted loop preview when a single video card is selected. |
| `bg_alpha_sidebar_left` | `0.65` | Tint opacity (0.0 to 1.0) over the left sidebar background image. |
| `bg_alpha_sidebar_right` | `0.65` | Tint opacity (0.0 to 1.0) over the right panel background image. |
| `bg_alpha_window` | `0.65` | Tint opacity (0.0 to 1.0) over the main window background image. |
| `content_search_max_file_mb` | `128` | Skip files larger than this many MiB during content search. `0` means no cap. |
| `current_folders_first` | `{}` | Per-folder override of `folders_first`, keyed by absolute path. |
| `default_icon_size` | `0` | Default pixel size of file and folder icons. |
| `default_sort` | `"Name"` | Fallback sort field: `Name`, `Date`, `Size` or `Type`. |
| `device_renames` | `{}` | Display names and icons for devices and partitions; see [Array tables](#array-tables). |
| `diff_editor` | `"nvim"` | Editor command used when opening a diff hunk location. |
| `diff_panel_width` | `420` | Width in pixels of the right-side git diff panel. |
| `disable_drag_and_drop` | `false` | Disable drag and drop of files and folders. |
| `enable_file_indexing` | `false` | Enable the FTS5 filename indexer. |
| `expand_labels` | `false` | Wrap filenames over multiple lines instead of truncating them. |
| `ffmpeg_auto_rotate` | `false` | Let FFmpeg rotate portrait video thumbnails. |
| `ffmpeg_seek_seconds` | `5.0` | Seek position in seconds when grabbing a video thumbnail frame. |
| `ffmpeg_threads` | `1` | Threads per FFmpeg process when extracting video frames. |
| `file_icons` | `{}` | Per-file custom icon image, keyed by absolute path. Managed by `--set-icon`. |
| `fish_theme` | `false` | Generate a fish `conf.d` snippet so the embedded shell's colors and prompt follow the GTK theme. Off by default. |
| `folder_cache_capacity` | `3` | Number of recently visited directory snapshots kept in memory. |
| `folder_grid_thumbnails` | `true` | Show image thumbnails in the folder grid of the preview panel. |
| `folder_icon_size` | `{}` | Per-folder icon size override, keyed by absolute path. |
| `folder_icons` | `{}` | Per-folder custom icon image, keyed by absolute path. Managed by `--set-icon`. |
| `folder_sort` | `{}` | Per-folder sort field override, keyed by absolute path. |
| `folders_first` | `true` | Group directories above files. |
| `grid_spacing` | `10` | Pixel spacing between items in the file grid. |
| `header_visible` | `true` | Show the header bar. Persisted across sessions. |
| `hidden_extensions` | `[]` | Extensions to hide from grid labels, for example `["desktop", "AppImage"]`. |
| `lazy_thumbnails` | `false` | Generate thumbnails only for items scrolled into view. |
| `list_icon_size` | `24` | Pixel size of icons in list mode. |
| `loader_batch_size` | `10` | Items dispatched per chunk into the grid while a folder loads. |
| `max_content_search_results` | `0` | Maximum results returned by content search. |
| `max_history` | `100` | Back and forward history entries kept. |
| `max_search_results` | `100` | Maximum matches during recursive filename and extension search. |
| `max_width_chars` | `20` | Characters shown in a file label before truncation. |
| `preview_panel_width` | `400` | Width in pixels of the right-side preview panel. |
| `preview_syntax_highlighting` | `true` | Syntax-highlight source files in the preview panel. |
| `recents_row` | `0` | Zero-based position of the Recents row among the `[[sidebar]]` entries. No effect when `show_recents` is `false`. |
| `scale_font_with_icons` | `false` | Scale label font size together with the grid icons. |
| `search_panel_width` | `350` | Width in pixels of the right-side search panel. |
| `show_csd` | `false` | Draw client-side window decorations. |
| `show_empty_dir_emblem` | `false` | Show an emblem on folders that contain no visible items. |
| `show_hidden_by_default` | `false` | Show hidden files at startup. |
| `show_recents` | `true` | Show the Recents location in the sidebar. |
| `show_symlink_emblem` | `true` | Show a badge and target hint on symbolic links. |
| `show_thumbnails` | `true` | Master switch for thumbnail generation; overrides `thumbnail_types`. |
| `show_xdg_dirs` | `false` | Show the XDG user directories, such as Documents and Downloads, in the sidebar. |
| `sidebar_visible` | `true` | Show the sidebar. Persisted across sessions. |
| `sidebar_width` | `0` | Width in pixels of the left sidebar. |
| `single_click` | `false` | Activate items with a single click. |
| `start_maximized` | `false` | Open the window maximized. |
| `startup_window_height` | `0` | Initial window height in pixels. |
| `startup_window_width` | `0` | Initial window width in pixels. |
| `tag_panel_width` | `350` | Width in pixels of the right-side tag panel. |
| `terminal_in_sidebar` | `false` |  |
| `theme` | `unset` | Name of the theme to apply, such as `default`, `os-dark` or `os-light`. |
| `thumbnail_size` | `256` | Target size in pixels of generated thumbnails. |
| `thumbnail_threads` | `4` | Maximum concurrent thumbnail workers. |
| `ui_scale` | `1.0` | Global UI scale multiplier. |
| `wide_preview` | `false` | Open the preview panel in wide mode, with the sidebar hidden. |
| `window_controls_left` | `false` | Put window controls on the left instead of the right. |

### `[ui.terminal]`

| Key | Default | Description |
|---|---|---|
| `bg_color` | `"#1A1A1A"` | Terminal background color. |
| `fg_color` | `"#E5E5E5"` | Terminal foreground color. |
| `font` | `"JetBrains Mono 13"` | Terminal font as a Pango font description. |
| `height` | `30` | Terminal pane height. |
| `shell` | `unset` | Shell started in the terminal pane; the user's login shell when unset. |

### `[ui.thumbnail_types]`

| Key | Default | Description |
|---|---|---|
| `audio` | `true` | Thumbnails and embedded artwork for audio files. |
| `executables` | `false` | Thumbnails for Windows `.exe` files. |
| `fonts` | `true` | Thumbnails for font files. |
| `images` | `true` | Thumbnails for image files. |
| `pdfs` | `true` | Thumbnails for PDF documents. |
| `videos` | `true` | Thumbnails for video files. |

<!-- END GENERATED: keys -->

## Array tables

`[[sidebar]]` adds a sidebar entry. `path` accepts a leading `~`. Set `kind = "label"` to make the entry a section heading instead of a location.

```toml
[[sidebar]]
name = "Projects"
icon = "folder-symbolic"
path = "~/Projects"
```

`[[network_bookmarks]]` saves a network location. `icon` is optional.

```toml
[[network_bookmarks]]
name = "NAS"
uri = "smb://nas.local/share"
```

`[ui.device_renames]` maps a device path to a display name and an optional icon:

```toml
[ui.device_renames]
"/path/to/device/" = { name = "Storage", icon = "drive-harddisk-solid-symbolic" }
```

## Default configuration

The same defaults as one file, as Flux would serialize it. Keys that are unset, such as `ui.theme` and the `[shortcuts]` entries, are omitted.

<!-- BEGIN GENERATED: default-toml -->
```toml
sidebar = []
network_bookmarks = []
default_list_mode = false

[ui]
terminal_in_sidebar = false
diff_editor = "nvim"
preview_panel_width = 400
diff_panel_width = 420
auto_show_diff = true
ui_scale = 1.0
content_search_max_file_mb = 128
enable_file_indexing = false
bg_alpha_window = 0.65
bg_alpha_sidebar_left = 0.65
bg_alpha_sidebar_right = 0.65
scale_font_with_icons = false
hidden_extensions = []
tag_panel_width = 350
search_panel_width = 350
actions_panel_width = 380
show_symlink_emblem = true
header_visible = true
auto_mime_body_color = "#e4e4e4"
auto_mime_font_color = "#ffffff"
auto_mime_font_size = 13.0
auto_generate_mime_icons = true
auto_mime_accent_color = "#1273b2"
autoplay_video_previews = true
thumbnail_size = 256
window_controls_left = false
show_empty_dir_emblem = false
ffmpeg_threads = 1
ffmpeg_seek_seconds = 5.0
ffmpeg_auto_rotate = false
loader_batch_size = 10
folder_cache_capacity = 3
thumbnail_threads = 4
max_search_results = 100
max_history = 100
max_content_search_results = 0
default_icon_size = 0
list_icon_size = 24
sidebar_width = 0
show_xdg_dirs = false
single_click = false
default_sort = "Name"
folders_first = true
show_hidden_by_default = false
show_csd = false
start_maximized = false
startup_window_width = 0
startup_window_height = 0
max_width_chars = 20
grid_spacing = 10
ascending = true
expand_labels = false
sidebar_visible = true
show_recents = true
recents_row = 0
show_thumbnails = true
lazy_thumbnails = false
disable_drag_and_drop = false
preview_syntax_highlighting = true
wide_preview = false
folder_grid_thumbnails = true
archive_thumbnail_cache = false
fish_theme = false

[ui.file_icons]

[ui.terminal]
height = 30
fg_color = "#E5E5E5"
bg_color = "#1A1A1A"
font = "JetBrains Mono 13"

[ui.current_folders_first]

[ui.folder_sort]

[ui.folder_icon_size]

[ui.device_renames]

[ui.folder_icons]

[ui.thumbnail_types]
images = true
videos = true
fonts = true
pdfs = true
executables = false
audio = true

[shortcuts]
```
<!-- END GENERATED: default-toml -->

## Renamed and removed keys

No keys have been renamed or removed yet.
