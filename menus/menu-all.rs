# --- File Info & Utilities ---
"󰯦      Copy Absolute Path"          => "all", "echo -n %p | wl-copy", "Path copied to clipboard"
"󰯦      Copy Filename"               => "all", "basename %p | tr -d '\n' | wl-copy", "Filename copied"
"󰨞      Open Terminal Here"          => "directory", "alacritty --working-directory=%p", "", "no_command_dialog"
"󰛖      Calculate SHA256"            => "file", "sha256sum %p | cut -d' ' -f1 | wl-copy", "SHA256 copied to clipboard"
