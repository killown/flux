import sys
import os
import subprocess
import tempfile


def main():
    files = sys.argv[1:]

    video_extensions = (".mp4", ".mkv", ".mov", ".webm", ".avi", ".flv")
    vids = [os.path.abspath(f) for f in files if f.lower().endswith(video_extensions)]

    if len(vids) < 2:
        subprocess.run(
            ["notify-send", "Error", "Select at least 2 video files to join."]
        )
        return

    output_dir = os.path.dirname(vids[0])
    output_path = os.path.join(output_dir, "merged_videos.mp4")

    with tempfile.NamedTemporaryFile(
        mode="w", delete=False, suffix=".txt"
    ) as temp_list:
        for vid in vids:
            safe_vid = vid.replace("'", "'\\''")
            temp_list.write(f"file '{safe_vid}'\n")
        temp_list_path = temp_list.name

    cmd = [
        "ffmpeg",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        temp_list_path,
        "-c",
        "copy",
        "-y",
        output_path,
    ]

    try:
        subprocess.run(cmd, check=True)
        subprocess.run(["notify-send", "Success", "Videos merged successfully."])
    except subprocess.CalledProcessError:
        subprocess.run(
            ["notify-send", "Error", "Merge failed. Check if codecs/resolutions match."]
        )
    finally:
        os.remove(temp_list_path)


if __name__ == "__main__":
    main()
