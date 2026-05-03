import sys
import os
import subprocess
import getpass
import gi

gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, Gio, GLib


class ChownWindow(Adw.ApplicationWindow):
    def __init__(self, app, targets):
        super().__init__(application=app)
        self.targets = targets
        self.set_default_size(500, 400)
        self.set_title("FLUX PERMISSIONS")

        self.main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.set_content(self.main_box)

        setup_page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        setup_page.append(Adw.HeaderBar())

        clamp = Adw.Clamp(maximum_size=400)
        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=20)
        vbox.set_margin_top(20)
        vbox.set_margin_bottom(20)
        vbox.set_margin_start(20)
        vbox.set_margin_end(20)

        group = Adw.PreferencesGroup(title="Ownership Transfer")

        self.user_entry = Adw.EntryRow(title="Owner User")
        self.user_entry.set_text(getpass.getuser())

        self.group_entry = Adw.EntryRow(title="Owner Group")
        self.group_entry.set_text(getpass.getuser())

        self.recursive_switch = Adw.SwitchRow(title="Recursive (-R)")
        self.recursive_switch.set_active(any(os.path.isdir(t) for t in self.targets))

        self.password_entry = Adw.PasswordEntryRow(title="Sudo Password")

        group.add(self.user_entry)
        group.add(self.group_entry)
        group.add(self.recursive_switch)
        group.add(self.password_entry)
        vbox.append(group)

        self.exec_btn = Gtk.Button(label="Apply Ownership")
        self.exec_btn.add_css_class("suggested-action")
        self.exec_btn.add_css_class("pill")
        self.exec_btn.set_halign(Gtk.Align.CENTER)
        self.exec_btn.connect("clicked", self._run_chown)
        vbox.append(self.exec_btn)

        clamp.set_child(vbox)
        setup_page.append(clamp)
        self.main_box.append(setup_page)

    def _run_chown(self, _):
        user = self.user_entry.get_text()
        group = self.group_entry.get_text()
        password = self.password_entry.get_text()
        recursive = self.recursive_switch.get_active()

        owner_str = f"{user}:{group}"

        cmd = ["sudo", "-S", "chown"]
        if recursive:
            cmd.append("-R")
        cmd.append(owner_str)
        cmd.extend(self.targets)

        try:
            subprocess.run(
                cmd, input=password.encode(), check=True, capture_output=True
            )
            self._close_with_status("Success")
        except subprocess.CalledProcessError:
            self._close_with_status("Error: Check Password/Paths")

    def _close_with_status(self, msg):
        banner = Adw.Banner(title=msg)
        self.main_box.prepend(banner)
        banner.set_revealed(True)
        GLib.timeout_add(1500, self.destroy)


def on_activate(app, targets):
    win = ChownWindow(app, targets)
    win.present()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(1)
    valid_targets = [
        os.path.abspath(arg) for arg in sys.argv[1:] if os.path.exists(arg)
    ]
    if not valid_targets:
        sys.exit(1)

    app = Adw.Application(application_id=None, flags=Gio.ApplicationFlags.FLAGS_NONE)
    app.connect("activate", lambda a: on_activate(a, valid_targets))
    app.run(None)
