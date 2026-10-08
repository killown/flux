use gtk::gio;

/// Resolves the first existing icon name in the fallback chain for `base_name`.
///
/// If none of the variants exist in the theme, returns the first variant
/// anyway - GIO can still resolve it through its own fallback machinery.
pub fn resolve_folder_icon_with_fallbacks(
    theme: &gtk::IconTheme,
    base_name: &str,
) -> Option<gio::Icon> {
    let variants: &[&str] = match base_name {
        "folder-download" => &[
            "folder-download",
            "folder-downloads",
            "folder-download-symbolic",
        ],
        "folder-documents" => &[
            "folder-documents",
            "folder-document",
            "folder-documents-symbolic",
        ],
        "folder-pictures" => &[
            "folder-pictures",
            "folder-picture",
            "folder-images",
            "folder-pictures-symbolic",
        ],
        "folder-videos" => &["folder-videos", "folder-video", "folder-videos-symbolic"],
        "folder-music" => &[
            "folder-music",
            "folder-audio",
            "folder-sound",
            "folder-music-symbolic",
        ],
        "user-desktop" => &["user-desktop", "folder-desktop", "user-desktop-symbolic"],
        "user-home" => &["user-home", "folder-home", "user-home-symbolic"],
        "folder-publicshare" => &[
            "folder-publicshare",
            "folder-public",
            "folder-publicshare-symbolic",
        ],
        "folder-templates" => &[
            "folder-templates",
            "folder-template",
            "folder-templates-symbolic",
        ],
        other => &[other],
    };

    for candidate in variants {
        if theme.has_icon(candidate) {
            if let Ok(icon) = gio::Icon::for_string(candidate) {
                return Some(icon);
            }
        }
    }

    gio::Icon::for_string(variants[0]).ok()
}
