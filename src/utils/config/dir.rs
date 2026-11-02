use std::path::PathBuf;
use std::sync::OnceLock;

/// Returns the directory that holds the `flux/` config folder.
///
/// Normally this is `dirs::config_dir()`. Inside a Flatpak, `XDG_CONFIG_HOME`
/// points at `~/.var/app/<id>/config`, so the host's `~/.config` would never be
/// read, there it is `$HOME/.config` instead. `HOME` is still the real home
/// directory, and the sandbox needs access to `~/.config/flux` (e.g.
/// `--filesystem=xdg-config/flux`) for it to be visible.
pub fn base_config_dir() -> Option<PathBuf> {
    static IN_FLATPAK: OnceLock<bool> = OnceLock::new();
    let in_flatpak = *IN_FLATPAK.get_or_init(|| std::path::Path::new("/.flatpak-info").exists());
    if in_flatpak {
        return dirs::home_dir().map(|home| home.join(".config"));
    }
    dirs::config_dir()
}
