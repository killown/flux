# --- Conversions ---
"󰽰      Convert > To MP4"             => "video/all", "ffmpeg -i %p -codec copy %p.mp4", "Converting to MP4..."
"󰽰      Convert > To MKV"             => "video/all", "ffmpeg -i %p -codec copy %p.mkv", "Converting to MKV..."
"󰽰      Convert > To WebM"            => "video/all", "ffmpeg -i %p -c:v libvpx-vp9 -crf 30 -b:v 0 %p.webm", "Converting to WebM..."

# --- Audio & Compression ---
"󰠝      Extract Audio (MP3)"          => "video/all", "ffmpeg -i %p -vn -acodec libmp3lame -q:a 2 %p.mp3", "Extracting MP3..."
"󰕧      Compress (HEVC/x265)"         => "video/all", "ffmpeg -i %p -vcodec libx265 -crf 28 -tag:v hvc1 -preset faster %p_compressed.mp4", "Compressing video..."

# --- Players ---
"󰕼      Play in MPV"                 => "video/all", "mpv %p", "", "no_command_dialog"
"󰕼      Play in VLC"                 => "video/all", "vlc %p", "", "no_command_dialog"
