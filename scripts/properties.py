#!/usr/bin/env python3
import hashlib
import pathlib
import math
import subprocess
import sys
import datetime
import struct
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk, Gio


def format_size(size_bytes):
    if size_bytes == 0:
        return "0B"
    units = ("B", "KB", "MB", "GB", "TB", "PB")
    i = int(math.floor(math.log(size_bytes, 1024)))
    return f"{round(size_bytes / math.pow(1024, i), 2)} {units[i]}"


def get_shannon_entropy(data):
    if not data:
        return 0
    entropy = 0
    for x in range(256):
        p_x = float(data.count(x)) / len(data)
        if p_x > 0:
            entropy += -p_x * math.log(p_x, 2)
    return round(entropy, 4)


def get_git_info(file_path):
    try:
        cmd = ["git", "log", "-1", "--format=%h|%ai|%s", "--", str(file_path)]
        res = subprocess.check_output(cmd, stderr=subprocess.DEVNULL).decode().strip()
        if res:
            h, d, s = res.split("|")
            return {"Hash": h, "Date": d, "Subject": s}
    except:
        return None


def get_elf_info(path):
    try:
        with open(path, "rb") as f:
            if f.read(4) != b"\x7fELF":
                return None
            f.seek(16)
            e_type = struct.unpack("<H", f.read(2))[0]
            e_machine = struct.unpack("<H", f.read(2))[0]
            types = {1: "Relocatable", 2: "Executable", 3: "Shared", 4: "Core"}
            machines = {0x3E: "x86_64", 0x03: "x86", 0x28: "ARM", 0xB7: "AArch64"}
            return {
                "Type": types.get(e_type, "Unknown"),
                "Arch": machines.get(e_machine, "Unknown"),
                "Endian": "Little" if f.read(1) == b"\x01" else "Big",
            }
    except:
        return None


def get_all_metadata(file_input):
    path = pathlib.Path(file_input).resolve()
    if not path.exists():
        return None
    stats = path.stat()

    gfile = Gio.File.new_for_path(str(path))
    try:
        info = gfile.query_info(
            "standard::content-type", Gio.FileQueryInfoFlags.NONE, None
        )
        mime_type = info.get_content_type()
    except:
        mime_type = "application/octet-stream"

    with open(path, "rb") as f:
        raw = f.read(1024 * 1024)

    data = {
        "File Identity": {
            "Filename": path.name,
            "Extension": path.suffix or "None",
            "MIME Type": mime_type,
        },
        "System & Disk": {
            "Full Path": str(path),
            "Size": f"{format_size(stats.st_size)} ({stats.st_size} B)",
            "Disk Usage": f"{format_size(stats.st_blocks * 512)}",
            "Inode": stats.st_ino,
            "Device ID": stats.st_dev,
        },
        "Temporal": {
            "Created": datetime.datetime.fromtimestamp(stats.st_ctime).strftime(
                "%Y-%m-%d %H:%M:%S"
            ),
            "Modified": datetime.datetime.fromtimestamp(stats.st_mtime).strftime(
                "%Y-%m-%d %H:%M:%S"
            ),
            "Accessed": datetime.datetime.fromtimestamp(stats.st_atime).strftime(
                "%Y-%m-%d %H:%M:%S"
            ),
        },
        "Security": {
            "Permissions": oct(stats.st_mode)[-3:],
            "UID/GID": f"{stats.st_uid}:{stats.st_gid}",
            "Entropy": get_shannon_entropy(raw),
            "SHA256": hashlib.sha256(raw).hexdigest(),
        },
    }

    if "executable" in mime_type or "sharedlib" in mime_type:
        elf = get_elf_info(path)
        if elf:
            data["Executable Analysis"] = elf

    is_text = any(x in mime_type for x in ["text", "javascript", "json", "xml"])
    if is_text or path.suffix in [".py", ".rs", ".toml", ".yaml", ".sh", ".md", ".txt"]:
        try:
            content = raw.decode(errors="ignore")
            lines = content.splitlines()
            data["Content Metrics"] = {
                "Line Count": len(lines),
                "Word Count": sum(len(l.split()) for l in lines),
                "TODOs": sum(1 for l in lines if "TODO" in l.upper()),
                "Line Endings": "LF"
                if "\n" in content and "\r\n" not in content
                else "CRLF",
            }
        except:
            pass

    git = get_git_info(path)
    if git:
        data["Git History"] = git
    return data


class MetadataWindow(Adw.ApplicationWindow):
    def __init__(self, app, metadata):
        super().__init__(
            application=app,
            title=f"Properties — {metadata['File Identity']['Filename']}",
        )
        self.set_default_size(540, 850)
        self.metadata = metadata
        self.mime_type = metadata["File Identity"]["MIME Type"]

        header = Adw.HeaderBar()
        self.toast_overlay = Adw.ToastOverlay()
        view = Adw.ToolbarView()
        view.add_top_bar(header)
        view.set_content(self.toast_overlay)
        self.set_content(view)

        page = Adw.PreferencesPage()
        self.toast_overlay.set_child(page)

        self.create_association_group(page)

        for section, items in metadata.items():
            group = Adw.PreferencesGroup(title=section)
            page.add(group)
            for key, val in items.items():
                row = Adw.ActionRow(title=key, subtitle=str(val))
                row.set_activatable(True)
                row.add_suffix(Gtk.Image.new_from_icon_name("edit-copy-symbolic"))
                row.connect("activated", lambda r, v=val: self.copy_to_clipboard(v))
                group.add(row)

        esc = Gtk.ShortcutController()
        esc.add_shortcut(
            Gtk.Shortcut.new(
                Gtk.ShortcutTrigger.parse_string("Escape"),
                Gtk.CallbackAction.new(lambda *_: self.close()),
            )
        )
        self.add_controller(esc)

    def create_association_group(self, page):
        group = Adw.PreferencesGroup(
            title="System Handler",
            description=f"Default application for {self.mime_type}",
        )
        page.add(group)

        apps = sorted(Gio.AppInfo.get_all(), key=lambda x: (x.get_name() or "").lower())
        self.app_list = apps
        default = Gio.AppInfo.get_default_for_type(self.mime_type, False)

        self.model = Gtk.StringList()
        selected_idx = Gtk.INVALID_LIST_POSITION
        for i, app in enumerate(apps):
            name = app.get_name() or "Unknown"
            self.model.append(name)
            if default and app.equal(default):
                selected_idx = i

        expression = Gtk.PropertyExpression.new(Gtk.StringObject, None, "string")
        self.combo = Adw.ComboRow(
            title="Open With",
            model=self.model,
            selected=selected_idx,
            enable_search=True,
            expression=expression,
        )
        self.combo.connect("notify::selected", self.on_combo_changed)
        group.add(self.combo)

    def on_combo_changed(self, combo, pspec):
        idx = combo.get_selected()
        if idx == Gtk.INVALID_LIST_POSITION:
            return
        app = self.app_list[idx]
        try:
            app.set_as_default_for_type(self.mime_type)
            desktop_id = app.get_id()
            if desktop_id:
                subprocess.run(
                    ["xdg-mime", "default", desktop_id, self.mime_type], check=False
                )
            self.toast_overlay.add_toast(
                Adw.Toast(title=f"Set {app.get_name()} as default")
            )
        except Exception as e:
            self.toast_overlay.add_toast(Adw.Toast(title=f"Error: {str(e)}"))

    def copy_to_clipboard(self, value):
        self.get_clipboard().set(str(value))
        self.toast_overlay.add_toast(Adw.Toast(title="Copied to clipboard"))


class App(Adw.Application):
    def __init__(self, metadata):
        super().__init__(application_id="com.neo.file-props")
        self.metadata = metadata

    def do_activate(self):
        win = MetadataWindow(self, self.metadata)
        win.present()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(1)
    meta = get_all_metadata(sys.argv[1])
    if meta:
        app = App(meta)
        app.run(None)
