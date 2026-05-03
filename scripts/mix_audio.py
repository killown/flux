import sys
import os
import subprocess


def main():
    # Capture all arguments passed by Flux (%p)
    files = sys.argv[1:]

    if len(files) < 2:
        subprocess.run(["notify-send", "Error", "Select one video and one audio file."])
        return

    # Expanded extension lists
    video_extensions = (".mp4", ".mkv", ".mov", ".webm", ".avi")
    audio_extensions = (".mp3", ".wav", ".m4a", ".flac", ".ogg")

    # The script finds the first match for each type, regardless of selection order
    vids = [f for f in files if f.lower().endswith(video_extensions)]
    auds = [f for f in files if f.lower().endswith(audio_extensions)]

    if not vids or not auds:
        subprocess.run(
            ["notify-send", "Error", "Need 1 video and 1 audio (WAV/MP3/etc)."]
        )
        return

    video_in = os.path.abspath(vids[0])
    audio_in = os.path.abspath(auds[0])

    output_dir = os.path.dirname(video_in)
    output_path = os.path.join(output_dir, "merged_output.mp4")

    # Command optimized for looping video to match audio length
    cmd = [
        "ffmpeg",
        "-stream_loop",
        "-1",
        "-i",
        video_in,
        "-i",
        audio_in,
        "-map",
        "0:v:0",
        "-map",
        "1:a:0",
        "-c:v",
        "copy",
        "-c:a",
        "aac",
        "-shortest",
        "-y",
        output_path,
    ]

    try:
        subprocess.run(cmd, check=True)
        subprocess.run(
            ["notify-send", "Success", f"Created: {os.path.basename(output_path)}"]
        )
    except subprocess.CalledProcessError:
        subprocess.run(["notify-send", "Error", "FFmpeg merge failed."])


if __name__ == "__main__":
    main()
