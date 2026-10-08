use super::template::get_or_create_template;
use std::fs;
use std::path::PathBuf;

/// Generates an SVG string using the template and injected parameters.
pub fn generate_mime_svg(
    extension: &str,
    accent_color: &str,
    body_color: &str,
    font_color: &str,
    base_font_size: f64,
) -> String {
    let clean_ext = extension.trim_start_matches('.').to_ascii_uppercase();
    let template = get_or_create_template();

    let font_size = match clean_ext.len() {
        0..=3 => base_font_size,
        4 => base_font_size * 0.83,
        5 => base_font_size * 0.69,
        _ => base_font_size * 0.55,
    };

    template
        .replace("{{ACCENT_COLOR}}", accent_color)
        .replace("{{BODY_COLOR}}", body_color)
        .replace("{{FONT_COLOR}}", font_color)
        .replace("{{FONT_SIZE}}", &format!("{:.1}", font_size))
        .replace("{{EXT}}", &clean_ext)
}

/// Writes an SVG icon into `~/.local/share/flux/icons/extensions/<subfolder>/<ext>.svg`.
///
/// The two public `save_*` helpers below differ only by `subfolder`, this is
/// the shared implementation.
fn save_extension_icon(
    extension: &str,
    subfolder: &str,
    accent_color: &str,
    body_color: &str,
    font_color: &str,
    base_font_size: f64,
) -> std::io::Result<PathBuf> {
    let clean_ext = extension.trim_start_matches('.').to_ascii_lowercase();
    let base_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux/icons/extensions")
        .join(subfolder);

    fs::create_dir_all(&base_dir)?;
    let target_path = base_dir.join(format!("{}.svg", clean_ext));

    let svg_content = generate_mime_svg(
        &clean_ext,
        accent_color,
        body_color,
        font_color,
        base_font_size,
    );
    fs::write(&target_path, svg_content)?;

    Ok(target_path)
}

/// Generates and writes the SVG icon to `~/.local/share/flux/icons/extensions/custom/<ext>.svg`.
#[allow(dead_code)]
pub fn save_custom_extension_icon(
    extension: &str,
    accent_color: &str,
    body_color: &str,
    font_color: &str,
    base_font_size: f64,
) -> std::io::Result<PathBuf> {
    save_extension_icon(
        extension,
        "custom",
        accent_color,
        body_color,
        font_color,
        base_font_size,
    )
}

/// Generates and writes the SVG icon to `~/.local/share/flux/icons/extensions/generated/<ext>.svg`.
pub fn save_generated_extension_icon(
    extension: &str,
    accent_color: &str,
    body_color: &str,
    font_color: &str,
    base_font_size: f64,
) -> std::io::Result<PathBuf> {
    save_extension_icon(
        extension,
        "generated",
        accent_color,
        body_color,
        font_color,
        base_font_size,
    )
}
