# --- Cargo & Build Tools ---
"󱁤      Cargo > Check Code"          => "text/x-rust", "alacritty -e cargo check", "", "no_command_dialog"
"󱁤      Cargo > Run Tests"           => "text/x-rust", "alacritty -e cargo test", "", "no_command_dialog"
"󱁤      Cargo > Format (cargo fmt)"  => "text/x-rust", "cargo fmt -- %p", "Formatted Rust file"
"󱁤      Cargo > Run File (cargo run)"=> "text/x-rust", "alacritty -e cargo run", "", "no_command_dialog"

# --- Editors & Analysis ---
"󰨞      Open in VS Code"             => "text/x-rust", "code %p", "", "no_command_dialog"
"󰅩      Open in Neovim"              => "text/x-rust", "alacritty -e nvim %p", "", "no_command_dialog"

# --- Utilities ---
"󰯦      Copy Module Path"            => "text/x-rust", "echo -n %f | sed 's/\\.rs$//' | wl-copy", "Module name copied"
"󰯦      Count Lines"                 => "text/x-rust", "wc -l %p | wl-copy", "Line count copied"
