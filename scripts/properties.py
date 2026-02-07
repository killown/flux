import hashlib
import pathlib
import math
import subprocess
import sys
import datetime
import os
import struct

try:
    import magic
    import gi

    gi.require_version("Gtk", "4.0")
    gi.require_version("Adw", "1")
    from gi.repository import Adw, Gtk, Gio  # pyright: ignore
except ImportError:
    subprocess.check_call(
        [sys.executable, "-m", "pip", "install", "python-magic", "PyGObject"]
    )
    import magic
    from gi.repository import Adw, Gtk, Gio  # pyright: ignore


def format_size(size_bytes):
    if size_bytes == 0:
        return "0B"
    units = ("B", "KB", "MB", "GB", "TB")
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
            magic_bytes = f.read(4)
            if magic_bytes != b"\x7fELF":
                return None
            f.seek(16)
            e_type = struct.unpack("<H", f.read(2))[0]
            e_machine = struct.unpack("<H", f.read(2))[0]
            types = {1: "Relocatable", 2: "Executable", 3: "Shared", 4: "Core"}
            machines = {0x3E: "x86_64", 0x03: "x86", 0x28: "ARM", 0xB7: "AArch64"}
            return {
                "Binary Type": types.get(e_type, "Unknown"),
                "Architecture": machines.get(e_machine, "Unknown"),
                "Stripped": "No" if b".symtab" in f.read() else "Yes",
            }
    except:
        return None


def get_media_info(path):
    try:
        cmd = [
            "ffprobe",
            "-v",
            "quiet",
            "-print_format",
            "csv=p=0",
            "-show_entries",
            "stream=width,height,codec_name,duration",
            str(path),
        ]
        res = subprocess.check_output(cmd, stderr=subprocess.DEVNULL).decode().strip()
        if res:
            parts = res.split(",")
            return {
                "Resolution": f"{parts[0]}x{parts[1]}" if parts[0] != "" else "N/A",
                "Codec": parts[2],
                "Duration": f"{round(float(parts[3]), 2)}s"
                if parts[3] != ""
                else "N/A",
            }
    except:
        return None


def get_xattrs(path):
    try:
        attrs = {}
        for name in os.listxattr(path):
            val = os.getxattr(path, name)
            attrs[name] = val.decode(errors="replace")
        return attrs
    except:
        return None


def get_all_metadata(file_input):
    path = pathlib.Path(file_input).resolve()
    if not path.exists():
        return None
    stats = path.stat()
    with open(path, "rb") as f:
        raw = f.read(1024 * 1024)
    mime_type = magic.from_buffer(raw, mime=True)
    data = {
        "File Identity": {
            "Filename": path.name,
            "Extension": path.suffix or "None",
            "MIME Type": mime_type,
            "Description": magic.from_buffer(raw),
        },
        "System Metadata": {
            "Full Path": str(path),
            "Size": f"{format_size(stats.st_size)} ({stats.st_size} B)",
            "Inode": stats.st_ino,
            "Device ID": stats.st_dev,
            "Links": stats.st_nlink,
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
        "Security & Ownership": {
            "Permissions": oct(stats.st_mode)[-3:],
            "Owner UID/GID": f"{stats.st_uid}:{stats.st_gid}",
            "Entropy": get_shannon_entropy(raw),
            "SHA256": hashlib.sha256(raw).hexdigest(),
        },
    }
    xattrs = get_xattrs(path)
    if xattrs:
        data["Extended Attributes (xattr)"] = xattrs
    if (
        mime_type == "application/x-executable"
        or mime_type == "application/x-sharedlib"
    ):
        elf = get_elf_info(path)
        if elf:
            data["Executable Analysis"] = elf
    if mime_type.startswith(("video/", "audio/", "image/")):
        media = get_media_info(path)
        if media:
            data["Media Properties"] = media
    if "text" in mime_type or path.suffix in [
        ".py",
        ".rs",
        ".toml",
        ".yaml",
        ".json",
        ".sh",
        ".md",
    ]:
        try:
            content = raw.decode(errors="ignore")
            lines = content.splitlines()
            data["Content Analysis"] = {
                "Line Count": len(lines),
                "Word Count": sum(len(l.split()) for l in lines),  # pyright: ignore
                "TODOs": sum(1 for l in lines if "TODO" in l.upper()),  # pyright: ignore
                "Line Endings": "CRLF" if "\r\n" in content[:2000] else "LF",
            }
            if content.startswith("#!"):
                data["File Identity"]["Interpreter"] = content.splitlines()[0]
        except:  # pyright: ignore
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
        self.set_default_size(520, 800)
        self.metadata = metadata
        self.mime_type = metadata["File Identity"]["MIME Type"]
        header = Adw.HeaderBar()
        header.set_decoration_layout(":close")
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
                copy_img = Gtk.Image.new_from_icon_name("edit-copy-symbolic")
                row.add_suffix(copy_img)
                row.connect("activated", self.on_row_activated, str(val))
                group.add(row)
        esc_controller = Gtk.ShortcutController()
        esc_trigger = Gtk.ShortcutTrigger.parse_string("Escape")
        esc_action = Gtk.CallbackAction.new(lambda *_: self.close())
        esc_shortcut = Gtk.Shortcut.new(esc_trigger, esc_action)
        esc_controller.add_shortcut(esc_shortcut)
        self.add_controller(esc_controller)

    def create_association_group(self, page):
        group = Adw.PreferencesGroup(
            title="Application Assignment",
            description=f"Global handler for {self.mime_type}",
        )
        page.add(group)
        all_apps = Gio.AppInfo.get_all()
        self.app_list = sorted(all_apps, key=lambda x: (x.get_name() or "").lower())
        self.default_app = Gio.AppInfo.get_default_for_type(self.mime_type, False)
        self.app_model = Gtk.StringList()
        default_index = 0
        for i, app_info in enumerate(self.app_list):
            name = app_info.get_name() or "Unknown"
            self.app_model.append(name)
            if self.default_app and app_info.equal(self.default_app):
                default_index = i
        self.combo_row = Adw.ComboRow(
            title="Default Provider",
            model=self.app_model,
            selected=default_index,
            enable_search=True,
        )
        group.add(self.combo_row)
        set_btn = Gtk.Button(
            label="Update Default Handler",
            halign=Gtk.Align.END,
            valign=Gtk.Align.CENTER,
        )
        set_btn.add_css_class("suggested-action")
        set_btn.set_margin_top(12)
        set_btn.connect("clicked", self.on_set_default_clicked)
        group.add(set_btn)

    def on_set_default_clicked(self, btn):
        selected_idx = self.combo_row.get_selected()
        if selected_idx == Gtk.INVALID_LIST_POSITION or selected_idx >= len(
            self.app_list
        ):
            return
        selected_app = self.app_list[selected_idx]
        try:
            selected_app.set_as_default_for_type(self.mime_type)
            self.toast_overlay.add_toast(
                Adw.Toast(title=f"Set {selected_app.get_name()} as default")
            )
        except Exception as e:
            self.toast_overlay.add_toast(Adw.Toast(title=f"Error: {str(e)}"))

    def on_row_activated(self, row, val):
        self.get_clipboard().set(str(val))
        self.toast_overlay.add_toast(Adw.Toast(title="Value copied"))


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
