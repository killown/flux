# --- Editors ---
"󰨞      Open in VS Code"             => "text/all", "code %p", "", "no_command_dialog"
"󰅩      Open in Neovim"              => "text/all", "alacritty -e nvim %p", "", "no_command_dialog"
"󰏫      Open in Gedit"               => "text/all", "gedit %p", "", "no_command_dialog"

# --- Text Utilities ---
"󰯦      Copy Path to Clipboard"      => "text/all", "echo -n %p | wl-copy", "Path copied"
"󰯦      Copy Line Count"             => "text/all", "wc -l %p | cut -d' ' -f1 | tr -d '\n' | wl-copy", "Line count copied"
"󰏦      Convert Markdown to HTML"    => "text/markdown", "pandoc %p -o %p.html", "Converted to HTML"
