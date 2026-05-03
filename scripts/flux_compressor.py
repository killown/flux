import sys
import os
import subprocess
import threading
import gi

gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, Gio, GLib


class ArchiveWorker(threading.Thread):
    def __init__(self, cmd, log_callback, exit_callback):
        super().__init__()
        self.cmd = cmd
        self.log_callback = log_callback
        self.exit_callback = exit_callback
        self.daemon = True

    def run(self):
        try:
            process = subprocess.Popen(
                self.cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
            )
            for line in process.stdout:
                GLib.idle_add(self.log_callback, line)
            process.wait()
            GLib.idle_add(self.exit_callback, process.returncode == 0)
        except Exception as e:
            GLib.idle_add(self.log_callback, str(e))
            GLib.idle_add(self.exit_callback, False)


class FluxCompressor(Adw.Application):
    def __init__(self, **kwargs):
        super().__init__(
            application_id="me.matrix.FluxCompressor",
            flags=Gio.ApplicationFlags.HANDLES_OPEN,
            **kwargs,
        )
        self.paths = []

    def do_activate(self):
        Adw.StyleManager.get_default().set_color_scheme(Adw.ColorScheme.PREFER_DARK)
        self.win = FluxWindow(application=self, paths=self.paths)
        self.win.present()

    def do_open(self, files, n_files, hint):
        self.paths = [f.get_path() for f in files]
        self.activate()


class FluxWindow(Adw.ApplicationWindow):
    def __init__(self, *args, **kwargs):
        self.paths = kwargs.pop("paths", [])
        super().__init__(*args, **kwargs)
        self.set_default_size(800, 600)
        self.set_title("FLUX ENGINE")

        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.SLIDE_LEFT_RIGHT)

        setup_page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        setup_page.append(Adw.HeaderBar())

        clamp = Adw.Clamp(maximum_size=600)
        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=20)
        vbox.set_margin_top(30)

        group = Adw.PreferencesGroup(title="Compression Settings")
        self.name_row = Adw.EntryRow(title="Archive Name", text="flux_archive")
        self.fmt_row = Adw.ComboRow(title="Format")
        self.fmt_row.set_model(Gtk.StringList.new(["7z", "Zip", "Tar.xz"]))
        self.pass_row = Adw.PasswordEntryRow(title="Password")

        group.add(self.name_row)
        group.add(self.fmt_row)
        group.add(self.pass_row)
        vbox.append(group)

        list_group = Adw.PreferencesGroup(title=f"Selections: {len(self.paths)}")
        scroll = Gtk.ScrolledWindow(
            min_content_height=200, propagate_natural_height=True
        )
        lbox = Gtk.ListBox()
        lbox.add_css_class("boxed-list")
        for p in self.paths:
            lbox.append(Adw.ActionRow(title=os.path.basename(p), subtitle=p))
        scroll.set_child(lbox)
        list_group.add(scroll)
        vbox.append(list_group)

        btn = Gtk.Button(label="Execute", halign=Gtk.Align.CENTER)
        btn.set_margin_top(20)
        btn.add_css_class("suggested-action")
        btn.add_css_class("pill")
        btn.connect("clicked", self._start)
        vbox.append(btn)

        clamp.set_child(vbox)
        setup_page.append(clamp)
        self.stack.add_named(setup_page, "setup")

        prog_page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        prog_page.append(Adw.HeaderBar())

        prog_vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=20)
        prog_vbox.set_margin_start(30)
        prog_vbox.set_margin_end(30)
        prog_vbox.set_margin_top(30)
        prog_vbox.set_margin_bottom(30)

        self.pbar = Gtk.ProgressBar()
        self.pbar.set_margin_top(20)

        self.log_view = Gtk.TextView(editable=False)
        self.log_view.add_css_class("monospace")
        scroll_log = Gtk.ScrolledWindow(vexpand=True)
        scroll_log.set_child(self.log_view)

        prog_vbox.append(self.pbar)
        prog_vbox.append(scroll_log)
        prog_page.append(prog_vbox)
        self.stack.add_named(prog_page, "progress")

        self.set_content(self.stack)

    def _start(self, _):
        name = self.name_row.get_text()
        fmt = self.fmt_row.get_selected()
        pw = self.pass_row.get_text()
        self.stack.set_visible_child_name("progress")

        exts = [".7z", ".zip", ".tar.xz"]
        out = f"{name}{exts[fmt]}"

        if fmt == 0:
            cmd = ["7z", "a", "-mx=9", out]
            if pw:
                cmd.append(f"-p{pw}")
        elif fmt == 1:
            cmd = ["zip", "-r9", out]
            if pw:
                cmd.extend(["-P", pw])
        else:
            cmd = ["tar", "-cJvf", out]

        cmd.extend(self.paths)
        ArchiveWorker(cmd, self._log, self._done).start()

    def _log(self, text):
        buf = self.log_view.get_buffer()
        buf.insert(buf.get_end_iter(), text)
        self.pbar.pulse()
        return False

    def _done(self, success):
        self.pbar.set_fraction(1.0 if success else 0.0)
        return False


if __name__ == "__main__":
    app = FluxCompressor()
    app.run(sys.argv)
