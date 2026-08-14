# --- Execution & Linting ---
"      Run in Terminal"             => "text/x-python", "alacritty -e python3 %p", "", "no_command_dialog"
"      Format with Black"          => "text/x-python", "black %p", "Formatted with Black"
"      Lint with Ruff"              => "text/x-python", "alacritty -e ruff check %p", "", "no_command_dialog"

# --- Virtual Environment ---
"      Create Venv Here"            => "directory", "python3 -m venv %p/venv", "Virtual environment created"

# --- Editors ---
"󰨞      Open in VS Code"             => "text/x-python", "code %p", "", "no_command_dialog"
"󰅩      Open in Neovim"              => "text/x-python", "alacritty -e nvim %p", "", "no_command_dialog"
