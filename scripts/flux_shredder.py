import os
import subprocess
import sys
import threading
import gi

gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gtk, Gio, GLib


class ShredWorker(threading.Thread):
    def __init__(self, targets, iterations, log_callback, exit_callback):
        super().__init__()
        self.targets = targets
        self.iterations = iterations
        self.log_callback = log_callback
        self.exit_callback = exit_callback
        self.daemon = True

    def _shred_file(self, file_path):
        cmd = ["shred", "-u", "-v", "-n", str(self.iterations), "-z", file_path]
        process = subprocess.Popen(
            cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
        )
        if process.stdout:
            for line in process.stdout:
                GLib.idle_add(self.log_callback, line)
        process.wait()
        return process.returncode == 0

    def run(self):
        overall_success = True
        try:
            for target in self.targets:
                GLib.idle_add(self.log_callback, f"\n[+] Processing: {target}\n")

                if os.path.isfile(target) or os.path.islink(target):
                    if not self._shred_file(target):
                        overall_success = False

                elif os.path.isdir(target):
                    for root, dirs, files in os.walk(target, topdown=False):
                        for file in files:
                            f_path = os.path.join(root, file)
                            if not self._shred_file(f_path):
                                overall_success = False
                        for d in dirs:
                            d_path = os.path.join(root, d)
                            try:
                                os.rmdir(d_path)
                            except OSError as err:
                                GLib.idle_add(
                                    self.log_callback, f"Error removing dir: {err}\n"
                                )
                                overall_success = False
                    try:
                        os.rmdir(target)
                    except OSError as err:
                        GLib.idle_add(
                            self.log_callback, f"Error removing root dir: {err}\n"
                        )
                        overall_success = False

            GLib.idle_add(self.exit_callback, overall_success)
        except Exception as e:
            GLib.idle_add(self.log_callback, f"Fatal: {str(e)}\n")
            GLib.idle_add(self.exit_callback, False)


class ShredWindow(Adw.ApplicationWindow):
    def __init__(self, app, targets):
        super().__init__(application=app)
        self.targets = targets
        self.set_default_size(600, 480)
        self.set_title("FLUX SHREDDER")

        self.main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.set_content(self.main_box)

        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)

        # ── Setup Page ────────────────────────────────────────────────────────
        setup_page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        setup_page.append(Adw.HeaderBar())

        clamp = Adw.Clamp(maximum_size=500)
        vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=20)
        vbox.set_margin_top(30)
        vbox.set_margin_bottom(30)
        vbox.set_margin_start(30)
        vbox.set_margin_end(30)

        status_icon = Gtk.Image.new_from_icon_name("dialog-warning-symbolic")
        status_icon.set_pixel_size(64)
        status_icon.add_css_class("error")
        vbox.append(status_icon)

        lbl = Gtk.Label(label="PERMANENT DESTRUCTION")
        lbl.add_css_class("title-1")
        vbox.append(lbl)

        group = Adw.PreferencesGroup(title="Queue Information")
        count_row = Adw.ActionRow(
            title="Items to Shred", subtitle=str(len(self.targets))
        )
        self.iter_row = Adw.ComboRow(title="Security Iterations")
        self.iter_row.set_model(
            Gtk.StringList.new(["3 passes", "7 passes", "35 passes"])
        )
        group.add(count_row)
        group.add(self.iter_row)
        vbox.append(group)

        self.confirm_check = Gtk.CheckButton(
            label="I understand this data is unrecoverable"
        )
        self.confirm_check.set_halign(Gtk.Align.CENTER)
        self.confirm_check.connect("toggled", self._on_confirm_toggle)
        vbox.append(self.confirm_check)

        self.exec_btn = Gtk.Button(label="Shred All")
        self.exec_btn.add_css_class("destructive-action")
        self.exec_btn.add_css_class("pill")
        self.exec_btn.set_halign(Gtk.Align.CENTER)
        self.exec_btn.set_sensitive(False)
        self.exec_btn.connect("clicked", self._start_shredding)
        vbox.append(self.exec_btn)

        clamp.set_child(vbox)
        setup_page.append(clamp)
        self.stack.add_named(setup_page, "setup")

        # ── Progress Page ─────────────────────────────────────────────────────
        prog_page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        prog_page.append(Adw.HeaderBar())
        prog_vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=15)
        prog_vbox.set_margin_top(20)
        prog_vbox.set_margin_bottom(20)
        prog_vbox.set_margin_start(20)
        prog_vbox.set_margin_end(20)

        self.pbar = Gtk.ProgressBar()
        self.log_view = Gtk.TextView(editable=False)
        self.log_view.add_css_class("monospace")
        scroll = Gtk.ScrolledWindow(vexpand=True)
        scroll.set_child(self.log_view)

        self.close_btn = Gtk.Button(label="Close")
        self.close_btn.add_css_class("pill")
        self.close_btn.set_halign(Gtk.Align.CENTER)
        self.close_btn.set_visible(False)
        self.close_btn.connect("clicked", lambda _: self.close())

        prog_vbox.append(self.pbar)
        prog_vbox.append(scroll)
        prog_vbox.append(self.close_btn)
        prog_page.append(prog_vbox)
        self.stack.add_named(prog_page, "progress")

        self.main_box.append(self.stack)

    def _on_confirm_toggle(self, check):
        self.exec_btn.set_sensitive(check.get_active())

    def _start_shredding(self, _):
        if not self.confirm_check.get_active():
            return
        iters = [3, 7, 35][self.iter_row.get_selected()]
        self.stack.set_visible_child_name("progress")
        ShredWorker(self.targets, iters, self._log, self._done).start()

    def _log(self, text):
        buf = self.log_view.get_buffer()
        end_iter = buf.get_end_iter()
        buf.insert(end_iter, text)
        mark = buf.create_mark(None, buf.get_end_iter(), False)
        self.log_view.scroll_to_mark(mark, 0.0, False, 0.0, 1.0)
        self.pbar.pulse()
        return False

    def _done(self, success):
        self.pbar.set_fraction(1.0 if success else 0.0)
        banner = Adw.Banner(
            title="Queue Processed Successfully"
            if success
            else "Error in processing queue"
        )
        self.main_box.prepend(banner)
        banner.set_revealed(True)
        self.close_btn.set_visible(True)
        return False


def on_activate(app, targets):
    win = ShredWindow(app, targets)
    win.present()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(1)

    valid_targets = []
    for arg in sys.argv[1:]:
        abs_path = os.path.abspath(arg)
        if os.path.exists(abs_path):
            valid_targets.append(abs_path)

    if not valid_targets:
        sys.exit(1)

    app = Adw.Application(application_id=None, flags=Gio.ApplicationFlags.FLAGS_NONE)
    app.connect("activate", lambda a: on_activate(a, valid_targets))
    app.run(None)
