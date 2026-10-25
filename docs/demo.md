# Panel Showcase

Flux features a suite of right-side panels to enhance your workflow without leaving the file manager. This page provides a visual guide to each panel, showing how to toggle them and what they are used for.

Each panel can be opened and closed independently, and opening one will close any other that is currently active to maximize screen space.

## Contextual Actions

The **Contextual Actions** panel provides a list of user-defined commands relevant to the currently selected file type. It reads actions from `menu-*.rs` templates in your `~/.config/flux/menus/` directory.

- **Toggle Shortcut:** `Ctrl+Shift+A`
- **Use Cases:** Run custom scripts, open files in specific applications, or perform complex file operations with a single click. You can add new actions directly from the panel.

<video src="https://raw.githubusercontent.com/killown/flux/main/screenshots/actions.mp4" controls muted playsinline width="100%"></video>

## Quick List

The **Quick List** panel gives you instant access to a "dock" of your most frequently used folders. Add the current directory or selected folders to the list, and use it to jump between projects or locations quickly.

- **Toggle Shortcut:** `Ctrl+Shift+Q`
- **Use Cases:** Keep your current project's directories, important download folders, or reference material just one click away. The list persists across sessions.

<video src="https://raw.githubusercontent.com/killown/flux/main/screenshots/quick_list.mp4" controls muted playsinline width="100%"></video>

## Tag Navigator

Flux allows you to organize files with tags using the `user.xdg.tags` extended attribute. The **Tag Navigator** panel lets you browse all tagged files, filter the current directory by tags, and manage your tags.

- **Toggle Shortcut:** `Ctrl+Shift+T`
- **Use Cases:** Create a cross-directory organization system for projects, photos, or documents. Click a tag to filter the view, or use the search bar to find a specific tag.

<video src="https://raw.githubusercontent.com/killown/flux/main/screenshots/tags.mp4" controls muted playsinline width="100%"></video>

## Embedded Terminal

The **Embedded Terminal** panel is a fully functional terminal right where you need it. It's powered by the same terminal engine as the bottom pane, but docks on the right.

- **Toggle Shortcut:** `F4` (when `ui.terminal.position` is set to `"sidebar"`)
- **Use Cases:** Run scripts on files you're browsing, use command-line tools like `git`, or perform complex file operations without leaving the Flux window. Press `Tab` for shell autocompletion, just as you would in a normal terminal.

<video src="https://raw.githubusercontent.com/killown/flux/main/screenshots/terminal.mp4" controls muted playsinline width="100%"></video>

## Peek Preview

The **Peek Preview** panel shows a rich preview of the selected file. It can render images, video, PDFs, Markdown, tables, and syntax-highlighted source code. It can also show the contents of folders and archives.

- **Toggle Shortcut:** `Ctrl+Space` (or just `Space` from the grid)
- **Use Cases:** Quickly inspect files without opening them in a separate application. Arrow through images or code, check the contents of a zip, or read a PDF.

<video src="https://raw.githubusercontent.com/killown/flux/main/screenshots/preview.mp4" controls muted playsinline width="100%"></video>

## Search Panel

The **Search** panel provides a powerful, advanced search interface. It allows you to perform a recursive search for files by name, extension, or even content, with filters for size and modification date.

- **Toggle Shortcut:** `Ctrl+F`
- **Use Cases:** Find all `.log` files in a subdirectory, search for a specific piece of text inside multiple files, or perform a case-sensitive regex search.

<video src="https://raw.githubusercontent.com/killown/flux/main/screenshots/search.mp4" controls muted playsinline width="100%"></video>
