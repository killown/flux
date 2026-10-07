use crate::i18n::tr;
use crate::model::{CustomAction, MenuEntry};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub fn ensure_config_file() -> PathBuf {
    crate::hit!("ensure_config_file");
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    let config_path = config_dir.join("menu.rs");
    if !config_path.exists() {
        // Syntax: "Label" => "mime_types", "command", "Optional Toast Message"
        // - Use %p for file path, %d for directory, %f for filename, %l for line number (content search).
        // - The third argument (Toast) is optional and shows a notification after execution.

        let default_config = r#"
"      Open Line in Nvim" => "text/all", "alacritty -e nvim +%l %p", "no_command_dialog"
# --- Core Operations ---
"󰋼      Add to Quick List" => "directory", "builtin::add_to_quick_list"
"󰓩      Open in New Tab" => "directory", "builtin::open_new_tab"
"󰪶      Send to Quick List" => "all", "builtin::quick_list_transfer"
"󰱝      Other Application..." => "file", "builtin::open_with_dialog"
"󰉋      Inspect Directory" => "directory", "builtin::inspect_dir"
"󰆏      Copy" => "all", "builtin::copy"
"󰆐      Cut" => "all", "builtin::cut"
"󰏊      Paste" => "all", "builtin::paste"
"󰑕      Rename" => "all", "builtin::rename"
"󰓹      Edit Tags" => "file", "builtin::tagfile"
"󰩹      Delete" => "all", "builtin::delete"
"󰦬      Restore File" => "trash", "gio trash --restore %p"
"󰆴      Shred File (Permanent)" => "all", "python3 $HOME/.local/share/flux/scripts/flux_shredder.py %p", "no_command_dialog"

# --- Navigation & System ---
"      Open Terminal" => "directory", "alacritty --working-directory=%p", "no_command_dialog"
"󰋜      Toggle Pin" => "directory", "builtin::toggle_pin"
"󰨞      Open in VSCode" => "text/all, application/all", "code %p", "no_command_dialog"
"󰋽      File Properties" => "file", "flux-fm --file-properties %p", "no_command_dialog"
"󰋊      Folder Info" => "directory", "baobab %p", "no_command_dialog"
"󰉋      New Folder" => "directory", "builtin::new_folder"
"󰈔      New File" => "directory", "builtin::new_file"

# --- Icon Customization ---
"󰸉      Icons > Select Folder Icon" => "directory", "builtin::select_folder_icon"
"󰸉      Icons > Set File Icon" => "all", "builtin::set_custom_icon"
"󰸉      Icons > Reset File Icon" => "all", "builtin::reset_custom_icon"
"󰸉      Icons > Set Extension Icon" => "file", "builtin::set_extension_icon"
"󰸉      Icons > Reset Extension Icon" => "file", "builtin::reset_extension_icon"

# --- Archives & Compression ---
"󰛖      Compress > To ZIP" => "all", "/usr/bin/python $HOME/.local/share/flux/scripts/flux_simple_compressor.py %p"
"󰛖      Compress > To 7Z" => "all", "7z a -m0=lzma2 -mx=9 archive.7z %f"
"󰛖      Compress > To TAR.GZ" => "all", "/usr/bin/python $HOME/.local/share/flux/scripts/flux_simple_compressor.py %p"
"󰛖      Extract Here!" => "application/zip, application/x-7z-compressed, application/x-rar, application/x-tar", "7z x %p -o%p_extracted"

# --- Media Edit ---
"󰽰      Media Edit > Join Videos" => "video/all", "python3 $HOME/.local/share/flux/scripts/join_videos.py %p"
"󰽰      Media Edit > Cut Video" => "video/all", "python $HOME/.local/share/flux/scripts/video_cutter.py %p", "no_command_dialog"
"󰽰      Media Edit > Mix Audio" => "audio/", "python3 $HOME/.local/share/flux/scripts/mix_audio.py %p"
"󰽰      Media Edit > Merge Video + Audio" => "video/all+audio/all", "python3 $HOME/.local/share/flux/scripts/join_mp4_mp3.py %p"

# --- Media Convert ---
"󰽰      Media Convert > To MP4" => "video/all", "ffmpeg -i %p -codec copy %p.mp4"
"󰽰      Media Convert > To MKV" => "video/all", "ffmpeg -i %p -codec copy %p.mkv"
"󰽰      Media Convert > To WebM" => "video/all", "ffmpeg -i %p -codec copy %p.webm"
"󰽰      Media Convert > To MOV" => "video/all", "ffmpeg -i %p -codec copy %p.mov"
"󰝚      Media Convert > Audio to MP3" => "audio/x-opus+ogg, audio/vnd.wave, audio/ogg", "ffmpeg -i %p -vn -ab 192k -ar 44100 -y %p.mp3"

# --- Media Optimization & Extraction ---
"󰠝      Media Extract > MP3 from Video" => "video/all", "ffmpeg -i %p -vn -acodec libmp3lame -q:a 2 %p.mp3"
"󰕧      Media Optimize > Reduce Video Size" => "video/all", "ffmpeg -i %p -vcodec libx265 -crf 28 -tag:v hvc1 -preset faster %p_reduced.mp4"

# --- Images & Wallpaper ---
"󰸉      Image Wallpaper > Set (swww)" => "image/all", "swww img %p"
"󰸉      Image Wallpaper > Set (wbg)" => "image/all", "cp %p ~/Images/fav.jpg && (killall wbg 2>/dev/null; (wbg -s $HOME/Images/fav.jpg </dev/null >/dev/null 2>&1 &))", "no_command_dialog"
"󰸉      Image Convert > To PNG" => "image/webp", "magick %p %p.png"
"󰸉      Image Convert > To AVIF" => "image/all", "avifenc --jobs all -q 65 %p %p.avif"
"󰸉      Image Convert > To JPG" => "image/all", "magick %p -quality 75 -strip %p-output.jpg"
"󰏦      Convert to PDF" => "image/all, application/pdf, application/msword, application/vnd.openxmlformats-officedocument.wordprocessingml.document", "python3 $HOME/.local/share/flux/scripts/pdf_converter.py %p"

# --- Flux Backgrounds ---
"󰸉      Flux Background > Window Background"       => "image/all", "builtin::set_bg_window", "Window background updated!"
"󰸉      Flux Background > Left Sidebar Background" => "image/all", "builtin::set_bg_sidebar_left", "Sidebar background updated!"
"󰸉      Flux Background > Right Sidebar Background"=> "image/all", "builtin::set_bg_sidebar_right", "Right panel background updated!"
"󰸉      Flux Background > Reset All Backgrounds" => "all", "builtin::clear_backgrounds", "Backgrounds reset to default"

# --- Tools ---
"󰯦      Tools > Git Gui" => "directory", "git gui", "no_command_dialog"
"󰯦      Tools > Download Video (1080p)" => "directory", "cd %p && yt-dlp -f 'bv[height<=1080]+ba/b[height<=1080]' $(wl-paste)"
"󰯦      Tools > Copy Path" => "all", "echo -n %p | wl-copy"
"󰯦      Tools > Copy Name" => "all", "basename %p | tr -d '\n' | wl-copy"
"󰯦      Tools > Advanced Archive Manager" => "all", "/usr/bin/python $HOME/.local/share/flux/scripts/flux_compressor.py %p", "no_command_dialog"
"#;

        if let Ok(mut file) = fs::File::create(&config_path) {
            let _ = file.write_all(default_config.as_bytes());
        }
    }
    config_path
}

/// Parses the right-hand side of a menu config line into (mime, command, optional_toast).
pub fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>, bool)> {
    let input = input.trim();

    let remainder = input.strip_prefix('"')?;
    let (mime, rest) = remainder.split_once('"')?;

    let second_part = rest.trim().strip_prefix(',')?.trim();

    let cmd_inner = second_part.strip_prefix('"')?;
    let (cmd, after_cmd) = cmd_inner.split_once('"')?;

    // Parse remaining optional tokens: toast and/or "no_command_dialog" (in any order)
    let mut toast: Option<String> = None;
    let mut no_command_dialog = false;

    let mut remainder = after_cmd.trim();
    while let Some(stripped) = remainder.strip_prefix(',') {
        let stripped = stripped.trim();
        if let Some(inner) = stripped.strip_prefix('"') {
            if let Some((token, rest)) = inner.split_once('"') {
                if token == "no_command_dialog" {
                    no_command_dialog = true;
                } else {
                    toast = Some(token.to_string());
                }
                remainder = rest.trim();
                continue;
            }
        }
        break;
    }

    Some((mime.to_string(), cmd.to_string(), toast, no_command_dialog))
}

pub fn load_menu_config() -> Vec<CustomAction> {
    crate::hit!("load_menu_config");
    let config_path = ensure_config_file();
    let content = std::fs::read_to_string(config_path).unwrap_or_default();

    let mut actions = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        if let Some((left, right)) = line.split_once("=>") {
            let raw_label = left.trim().trim_matches('"');

            // Separate icon/spacing prefix from the actual text content
            let (main_icon_prefix, text_content) = if let Some(idx) =
                raw_label.find(|c: char| c.is_alphanumeric() || c == '(' || c == '>')
            {
                raw_label.split_at(idx)
            } else {
                ("", raw_label)
            };

            let translated_text = if text_content.contains(" > ") {
                let parts: Vec<&str> = text_content.splitn(2, " > ").collect();
                let raw_sub = parts[0];

                let (sub_icon, sub_pure) =
                    if let Some(idx) = raw_sub.find(|c: char| c.is_alphanumeric() || c == '(') {
                        raw_sub.split_at(idx)
                    } else {
                        ("", raw_sub)
                    };

                let cat = match sub_pure.trim() {
                    "Icons" => tr("Icons"),
                    "Compress" => tr("Compress"),
                    "Media Edit" => tr("Media Edit"),
                    "Media Convert" => tr("Media Convert"),
                    "Media Extract" => tr("Media Extract"),
                    "Media Optimize" => tr("Media Optimize"),
                    "Image Wallpaper" => tr("Image Wallpaper"),
                    "Image Convert" => tr("Image Convert"),
                    "Flux Background" => tr("Flux Background"),
                    "Tools" => tr("Tools"),
                    other => other.to_string(),
                };

                let raw_act = parts[1];
                let (act_icon, act_pure) =
                    if let Some(idx) = raw_act.find(|c: char| c.is_alphanumeric() || c == '(') {
                        raw_act.split_at(idx)
                    } else {
                        ("", raw_act)
                    };

                let act = match act_pure.trim() {
                    "Set File Icon" => tr("Set File Icon"),
                    "Reset File Icon" => tr("Reset File Icon"),
                    "Set Extension Icon" => tr("Set Extension Icon"),
                    "Reset Extension Icon" => tr("Reset Extension Icon"),
                    "To ZIP" => tr("To ZIP"),
                    "To 7Z" => tr("To 7Z"),
                    "To TAR.GZ" => tr("To TAR.GZ"),
                    "Join Videos" => tr("Join Videos"),
                    "Cut Video" => tr("Cut Video"),
                    "Mix Audio" => tr("Mix Audio"),
                    "Merge Video + Audio" => tr("Merge Video + Audio"),
                    "To MP4" => tr("To MP4"),
                    "To MKV" => tr("To MKV"),
                    "To WebM" => tr("To WebM"),
                    "To MOV" => tr("To MOV"),
                    "Audio to MP3" => tr("Audio to MP3"),
                    "MP3 from Video" => tr("MP3 from Video"),
                    "Reduce Video Size" => tr("Reduce Video Size"),
                    "Set (swww)" => tr("Set (swww)"),
                    "Set (wbg)" => tr("Set (wbg)"),
                    "To PNG" => tr("To PNG"),
                    "To AVIF" => tr("To AVIF"),
                    "To JPG" => tr("To JPG"),
                    "Window Background" => tr("Window Background"),
                    "Left Sidebar Background" => tr("Left Sidebar Background"),
                    "Right Sidebar Background" => tr("Right Sidebar Background"),
                    "Reset All Backgrounds" => tr("Reset All Backgrounds"),
                    "Git Gui" => tr("Git Gui"),
                    "Download Video (1080p)" => tr("Download Video (1080p)"),
                    "Copy Path" => tr("Copy Path"),
                    "Copy Name" => tr("Copy Name"),
                    "Advanced Archive Manager" => tr("Advanced Archive Manager"),
                    other => other.to_string(),
                };

                let full_sub = format!("{}{}", sub_icon, cat);
                let full_act = format!("{}{}", act_icon, act);

                format!("{} > {}", full_sub, full_act)
            } else {
                match text_content.trim() {
                    "Add to Quick List" => tr("Add to Quick List"),
                    "Open in New Tab" | "Abrir em Nova Aba" => tr("Open in New Tab"),
                    "Other Application..." => tr("Other Application..."),
                    "Inspect Directory" => tr("Inspect Directory"),
                    "Copy" => tr("Copy"),
                    "Cut" => tr("Cut"),
                    "Paste" => tr("Paste"),
                    "Rename" => tr("Rename"),
                    "Edit Tags" => tr("Edit Tags"),
                    "Delete" => tr("Delete"),
                    "Restore File" => tr("Restore File"),
                    "Shred File (Permanent)" => tr("Shred File (Permanent)"),
                    "Open Terminal" => tr("Open Terminal"),
                    "Toggle Pin" => tr("Toggle Pin"),
                    "Open in VSCode" => tr("Open in VSCode"),
                    "File Properties" => tr("File Properties"),
                    "Folder Info" => tr("Folder Info"),
                    "New Folder" => tr("New Folder"),
                    "New File" => tr("New File"),
                    "Extract Here!" => tr("Extract Here!"),
                    "Convert to PDF" => tr("Convert to PDF"),
                    "Open Line in Nvim" => tr("Open Line in Nvim"),
                    other => other.to_string(),
                }
            };

            let final_full_label = format!("{}{}", main_icon_prefix, translated_text);

            let (submenu, label) = if final_full_label.contains(" > ") {
                let parts: Vec<&str> = final_full_label.splitn(2, " > ").collect();
                (
                    Some(parts[0].trim().to_string()),
                    parts[1].trim().to_string(),
                )
            } else {
                (None, final_full_label)
            };
            if let Some((mimes_part, cmd_part, toast, no_command_dialog)) = split_mime_cmd(right) {
                let mime_types: Vec<String> = mimes_part
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();

                actions.push(CustomAction {
                    label,
                    submenu,
                    action_name: format!("custom_{}", i),
                    command: cmd_part,
                    mime_types,
                    toast,
                    no_command_dialog,
                });
            }
        }
    }
    actions
}

/// Writes the current menu actions to `~/.config/flux/menu.rs`
/// in the same DSL format expected by `load_menu_config`.
pub fn save_menu_config(actions: &[CustomAction]) -> std::io::Result<()> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");
    std::fs::create_dir_all(&config_dir)?;
    let path = config_dir.join("menu.rs");

    let mut content = String::new();
    for action in actions {
        // Convert CustomAction to MenuEntry for consistent serialization
        let entry = MenuEntry {
            label: action.label.clone(),
            submenu: action.submenu.clone(),
            mime_types: action.mime_types.join(", "),
            command: action.command.clone(),
            toast: action.toast.clone(),
            no_command_dialog: action.no_command_dialog,
        };
        content.push_str(&entry.to_config_line());
        content.push('\n');
    }
    std::fs::write(path, content)?;
    Ok(())
}
