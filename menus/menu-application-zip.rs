# --- Archive Operations ---
"󰛖      Extract Here"                => "application/zip", "unzip -o %p -d %d", "Extracted archive"
"󰛖      Extract to Subfolder"        => "application/zip", "unzip -o %p -d %p_extracted", "Extracted to folder"
"󰛖      List Contents in Terminal"   => "application/zip", "alacritty -e sh -c 'unzip -l %p; read -n 1'", "", "no_command_dialog"
