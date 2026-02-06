#!/usr/bin/env python3
import hashlib
import pathlib
import math
import subprocess
import sys

try:
    import magic
    import gi

    gi.require_version("Gtk", "4.0")
    gi.require_version("Adw", "1")
    from gi.repository import Adw, Gtk
except ImportError:
    subprocess.check_call(
        [sys.executable, "-m", "pip", "install", "python-magic", "PyGObject"]
    )
    import magic
    from gi.repository import Adw, Gtk


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


def get_all_metadata(file_input):
    path = pathlib.Path(file_input).resolve()
    if not path.exists():
        return None
    stats = path.stat()
    with open(path, "rb") as f:
        raw = f.read(1024 * 1024)

    data = {
        "File Identity": {
            "Filename": path.name,
            "Full Path": str(path),
            "MIME Type": magic.from_buffer(raw, mime=True),
            "Description": magic.from_buffer(raw),
        },
        "Filesystem": {
            "Size": f"{format_size(stats.st_size)} ({stats.st_size} B)",
            "Inode": stats.st_ino,
            "Permissions": oct(stats.st_mode),
            "Owner": f"{stats.st_uid}:{stats.st_gid}",
            "Hardlinks": stats.st_nlink,
        },
        "Deep Analysis": {
            "Entropy": get_shannon_entropy(raw),
            "SHA256": hashlib.sha256(raw).hexdigest(),
            "MD5": hashlib.md5(raw).hexdigest(),
            "Magic Header": raw[:16].hex(" ").upper(),
        },
    }

    if "text" in data["File Identity"]["MIME Type"] or path.suffix in [
        ".py",
        ".rs",
        ".toml",
    ]:
        try:
            lines = raw.decode(errors="ignore").splitlines()
            data["Code Metrics"] = {
                "Total Lines": len(lines),
                "Word Count": sum(len(l.split()) for l in lines),
                "TODOs": sum(1 for l in lines if "TODO" in l.upper()),
            }
        except:
            pass

    git = get_git_info(path)
    if git:
        data["Git Integration"] = git
    return data


class MetadataWindow(Adw.ApplicationWindow):
    def __init__(self, app, metadata):
        super().__init__(application=app, title="Properties")
        self.set_default_size(460, 600)

        # GNOME HIG: Utility windows usually don't need min/max
        header = Adw.HeaderBar()
        header.set_decoration_layout(
            ":"
        )  # Removes min/max/close. Use ":close" if you want only X.

        view = Adw.ToolbarView()
        view.add_top_bar(header)
        self.set_content(view)

        page = Adw.PreferencesPage()
        view.set_content(page)

        # Add an Escape shortcut to close the window
        esc_controller = Gtk.ShortcutController()
        esc_trigger = Gtk.ShortcutTrigger.parse_string("Escape")
        esc_action = Gtk.CallbackAction.new(lambda *_: self.close())
        esc_shortcut = Gtk.Shortcut.new(esc_trigger, esc_action)
        esc_controller.add_shortcut(esc_shortcut)
        self.add_controller(esc_controller)

        for section, items in metadata.items():
            group = Adw.PreferencesGroup(title=section)
            page.add(group)

            for key, val in items.items():
                row = Adw.ActionRow(title=key, subtitle=str(val))
                row.set_activatable(True)

                # Copy to clipboard on click
                copy_img = Gtk.Image.new_from_icon_name("edit-copy-symbolic")
                row.add_suffix(copy_img)
                row.connect("activated", self.on_row_activated, str(val))

                group.add(row)

    def on_row_activated(self, row, val):
        clipboard = self.get_clipboard()
        clipboard.set(val)
        # Optional: You could add a 'Toast' here to say "Copied!"


class App(Adw.Application):
    def __init__(self, metadata):
        super().__init__(application_id="com.neo.file-props")
        self.metadata = metadata

    def do_activate(self):
        win = MetadataWindow(self, self.metadata)
        win.present()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: file-props <file>")
        sys.exit(1)

    meta = get_all_metadata(sys.argv[1])
    if meta:
        app = App(meta)
        app.run(None)
