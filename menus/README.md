# Flux Menu System Architecture

Flux features a dual-level, DSL-driven context menu system designed for extreme speed and modularity.

1. **Primary Menu (`menu.rs`)** - Opened via **Right-Click**. Contains global actions, built-ins, and standard context operations.
2. **Secondary Menu (`menus/menu-*.rs`)** - Opened via **Ctrl + Right-Click**. Loaded dynamically based on the target file's **MIME type**.

## 1. Directory & File Structure

Menus are stored in `~/.config/flux/` (or `~/.config/flux/menus/`):

```text
~/.config/flux/
├── menu.rs                     # Primary context menu (Right-Click)
└── menus/                      # Secondary contextual menus (Ctrl + Right-Click)
    ├── menu-text-rust.rs       # Specific to text/x-rust (.rs)
    ├── menu-image-png.rs       # Specific to image/png (.png)
    ├── menu-image-all.rs       # Wildcard fallback for any image/* type
    ├── menu-text-all.rs        # Wildcard fallback for any text/* type
    └── menu-all.rs             # Global fallback for secondary menu
```

## 2. Dynamic Priority Resolution

When you trigger a secondary menu (**Ctrl + Right-Click**) on a file, Flux inspects its MIME type-for example, `image/png`-and searches `~/.config/flux/menus/` in strict priority order:

| Priority        | Pattern                        | Example for `image/png` | Description                             |
| --------------- | ------------------------------ | ----------------------- | --------------------------------------- |
| **1 (Highest)** | `menu-{category}-{subtype}.rs` | `menu-image-png.rs`     | Targets exact MIME subtype              |
| **2**           | `menu-{category}-all.rs`       | `menu-image-all.rs`     | Targets all subtypes under the category |
| **3 (Lowest)**  | `menu-all.rs`                  | `menu-all.rs`           | Global fallback secondary menu          |

Flux loads and executes the **first matching file** found on disk.

## 3. Menu DSL Syntax

All menu configuration files use a clean, concise Domain-Specific Language (DSL):

```text
"Label" => "MIME_FILTER", "COMMAND", "TOAST_NOTIFICATION", "FLAGS"
```

### Basic Command with Toast

```text
"󰸉      Set as Wallpaper" => "image/all", "swww img %p", "Wallpaper set!"
```

### Submenu Grouping (`Parent > Child`)

```text
"󰸉      Convert > To WebP" => "image/all", "magick %p -quality 80 %p.webp", "Converted to WebP"
"󰸉      Convert > To AVIF" => "image/all", "avifenc --jobs all -q 65 %p %p.avif", "Converted to AVIF"
```

Flux uses the `Parent > Child` syntax to group related commands into nested submenus.

### Detached GUI App

Use the `no_command_dialog` flag when launching GUI applications that should open without an execution dialog:

```text
"󰨞      Open in VS Code" => "text/all", "code %p", "", "no_command_dialog"
```

## 4. Parameter Placeholders

When executing a command, Flux automatically resolves the following placeholders:

| Placeholder | Expands To                        | Example                       |
| ----------- | --------------------------------- | ----------------------------- |
| `%p`        | Full absolute path to target file | `/home/user/Images/photo.png` |
| `%f`        | Filename with extension           | `photo.png`                   |
| `%d`        | Parent directory path             | `/home/user/Images`           |

This allows menu commands to operate directly on the file or directory selected by the user without requiring hard-coded paths.

## 5. Flags & Special Modifiers

### `no_command_dialog`

Detaches the command from Flux's process tracker.

This is particularly useful for GUI applications such as:

- GIMP
- Inkscape
- VS Code

These applications can open immediately without triggering a background transfer or progress dialog in Flux.

### Built-in Functions

Commands can also use native Flux primitives instead of external shell binaries:

```text
builtin::copy
builtin::cut
builtin::paste
builtin::rename
builtin::delete
builtin::new_folder
builtin::new_file
builtin::open_with
```

These built-ins allow common file-management operations to remain integrated with Flux rather than depending on external command-line utilities.

## Summary

Flux's menu architecture separates **global context actions** from **MIME-aware contextual actions**:

- **Right-Click** → Primary menu from `menu.rs`
- **Ctrl + Right-Click** → Secondary MIME-specific menu
- **Exact MIME match** → Highest priority
- **Category wildcard** → Fallback
- **`menu-all.rs`** → Global fallback
- **DSL-driven configuration** → Compact and extensible
- **Placeholders** → Dynamic file and directory targeting
- **Built-ins** → Native Flux operations
- **Flags** → Fine-grained command execution behavior

This design keeps the core menu system lightweight while allowing users to extend contextual actions without modifying Flux itself.
