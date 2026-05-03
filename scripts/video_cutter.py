import sys
import os
import subprocess
import gi
import tempfile

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio


class VideoCutWindow(Adw.ApplicationWindow):
    def __init__(self, app, input_path):
        super().__init__(application=app, title="Flux Video Cutter (Remove Segment)")
        self.input_path = input_path
        self.set_default_size(400, -1)
        self.set_resizable(False)

        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.set_content(content_box)

        header = Adw.HeaderBar()
        content_box.append(header)

        inner_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        inner_box.set_margin_top(18)
        inner_box.set_margin_bottom(18)
        inner_box.set_margin_start(18)
        inner_box.set_margin_end(18)
        content_box.append(inner_box)

        status_page = Adw.StatusPage(
            title="Remove Segment",
            description=f"Cutting out part of: {os.path.basename(input_path)}",
            icon_name="video-x-generic-symbolic",
        )
        inner_box.append(status_page)

        group = Adw.PreferencesGroup()
        inner_box.append(group)

        self.start_entry = Gtk.Entry(placeholder_text="00:05:00")
        start_row = Adw.ActionRow(
            title="Start of Cut", subtitle="Segment to REMOVE starts at"
        )
        start_row.add_suffix(self.start_entry)
        group.add(start_row)

        self.end_entry = Gtk.Entry(placeholder_text="00:06:00")
        end_row = Adw.ActionRow(
            title="End of Cut", subtitle="Segment to REMOVE ends at"
        )
        end_row.add_suffix(self.end_entry)
        group.add(end_row)

        cut_button = Gtk.Button(label="Remove Segment & Merge")
        cut_button.add_css_class("suggested-action")
        cut_button.add_css_class("pill")
        cut_button.set_margin_top(12)
        cut_button.connect("clicked", self.on_cut_clicked)
        inner_box.append(cut_button)

    def get_duration(self, file_path):
        cmd = [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            file_path,
        ]
        result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        return float(result.stdout)

    def on_cut_clicked(self, button):
        start_cut = self.start_entry.get_text().strip()
        end_cut = self.end_entry.get_text().strip()

        if not start_cut or not end_cut:
            return

        base, ext = os.path.splitext(self.input_path)
        output_path = f"{base}-output{ext}"

        # Use temp directory for intermediate parts
        tmp_dir = tempfile.gettempdir()
        part1 = os.path.join(tmp_dir, f"part1{ext}")
        part2 = os.path.join(tmp_dir, f"part2{ext}")
        list_file = os.path.join(tmp_dir, "parts.txt")

        # 1. Create part before the cut
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-to",
                start_cut,
                "-i",
                self.input_path,
                "-codec",
                "copy",
                part1,
            ]
        )

        # 2. Create part after the cut
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-ss",
                end_cut,
                "-i",
                self.input_path,
                "-codec",
                "copy",
                part2,
            ]
        )

        # 3. Create concat list
        with open(list_file, "w") as f:
            f.write(f"file '{part1}'\n")
            f.write(f"file '{part2}'\n")

        # 4. Merge
        cmd_merge = [
            "ffmpeg",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            list_file,
            "-codec",
            "copy",
            output_path,
        ]

        try:
            subprocess.Popen(cmd_merge)
            self.get_application().quit()
        except Exception as e:
            print(f"Error: {e}")


def main():
    if len(sys.argv) < 2:
        sys.exit(1)

    app = Adw.Application(
        application_id="com.flux.VideoRemover", flags=Gio.ApplicationFlags.FLAGS_NONE
    )
    app.connect("activate", lambda a: VideoCutWindow(a, sys.argv[1]).present())
    app.run(None)


if __name__ == "__main__":
    main()
