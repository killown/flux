use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static EXT_MIME_CACHE: OnceLock<HashMap<String, String>> = OnceLock::new();

const EMBEDDED_DEFAULT_TEMPLATE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" version="1.1">
  <!-- Outer soft shadow -->
  <path style="opacity:0.2" d="M 12.75,5 C 11.2265,5 10,6.2488 10,7.8 v 50.4 c 0,1.55008 1.2265,2.8 2.75,2.8 h 38.5 C 52.7724,61 54,59.75008 54,58.2 V 7.8 C 54,6.2488 52.7724,5 51.25,5 Z"/>

  <!-- Page body -->
  <path style="fill:{{BODY_COLOR}}" d="M 12.75,4 C 11.2265,4 10,5.2488 10,6.8 v 50.4 c 0,1.55008 1.2265,2.8 2.75,2.8 h 38.5 C 52.7724,60 54,58.75008 54,57.2 V 6.8 C 54,5.2488 52.7724,4 51.25,4 Z"/>

  <!-- Top edge highlight -->
  <path style="opacity:0.2;fill:#ffffff" d="M 12.75,4 C 11.2265,4 10,5.24958 10,6.80078 L 10,7.80078 C 10,6.24958 11.2265,5 12.75,5 h 38.5 C 52.7724,5 54,6.24958 54,7.80078 L 54,6.80078 C 54,5.24958 52.7724,4 51.25,4 Z"/>

  <!-- Content lines (subtle, above the accent) -->
  <path style="opacity:0.5" d="m 20,20 v 2.5 h 24 v -2.5 z m 0,5 v 2.5 h 24 v -2.5 z m 0,5 v 2.5 h 24 v -2.5 z m 0,5 v 2.5 h 15 v -2.5 z"/>

  <!-- Accent block at the bottom with subtle 3D -->
  <path style="fill:{{ACCENT_COLOR}}" d="M 10,42 H 54 V 57.2 C 54,58.75008 52.7724,60 51.25,60 H 12.75 C 11.2265,60 10,58.75008 10,57.2 Z"/>
  <path style="opacity:0.15;fill:#ffffff" d="M 10,42 H 54 V 43.2 H 10 Z"/>
  <path style="opacity:0.15;fill:#000000" d="M 10,58.8 H 54 V 57.2 C 54,58.75008 52.7724,60 51.25,60 H 12.75 C 11.2265,60 10,58.75008 10,57.2 Z"/>

  <!-- Extension text, nudged below center of the accent block -->
  <text x="32" y="53.5"
        font-family="-apple-system, BlinkMacSystemFont, 'Helvetica Neue', Helvetica, Arial, sans-serif"
        font-size="{{FONT_SIZE}}"
        font-weight="700"
        letter-spacing="0.3"
        fill="{{FONT_COLOR}}"
        text-anchor="middle"
        dominant-baseline="central"
        textLength="38"
        lengthAdjust="spacingAndGlyphs">{{EXT}}</text>
</svg>"##;

/// Resolves `~/.local/share/flux/icons/template.svg`, auto-populating it with the default if absent.
fn get_or_create_template() -> String {
    let template_path = dirs::data_dir()
        .map(|d| d.join("flux/icons/template.svg"))
        .unwrap_or_else(|| PathBuf::from("template.svg"));

    if let Ok(content) = fs::read_to_string(&template_path) {
        return content;
    }

    if let Some(parent) = template_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&template_path, EMBEDDED_DEFAULT_TEMPLATE);

    EMBEDDED_DEFAULT_TEMPLATE.to_string()
}

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

/// Generates and writes the SVG icon to `~/.local/share/flux/icons/extensions/custom/<ext>.svg`.
#[allow(dead_code)]
pub fn save_custom_extension_icon(
    extension: &str,
    accent_color: &str,
    body_color: &str,
    font_color: &str,
    base_font_size: f64,
) -> std::io::Result<PathBuf> {
    let clean_ext = extension.trim_start_matches('.').to_ascii_lowercase();
    let base_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux/icons/extensions/custom");

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

/// Generates and writes the SVG icon to `~/.local/share/flux/icons/extensions/generated/<ext>.svg`.
pub fn save_generated_extension_icon(
    extension: &str,
    accent_color: &str,
    body_color: &str,
    font_color: &str,
    base_font_size: f64,
) -> std::io::Result<PathBuf> {
    let clean_ext = extension.trim_start_matches('.').to_ascii_lowercase();
    let base_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux/icons/extensions/generated");

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

/// Returns the system-registered MIME type for any extension using the FreeDesktop database.
pub fn lookup_system_extension_mime(ext: &str) -> Option<String> {
    if ext.is_empty() {
        return None;
    }
    let ext_lower = ext.to_ascii_lowercase();

    let table = EXT_MIME_CACHE.get_or_init(|| {
        let mut map = HashMap::with_capacity(4096);

        // Standard Freedesktop shared-mime-info globs locations
        let candidate_paths = [
            "/usr/share/mime/globs2",
            "/usr/local/share/mime/globs2",
            "/usr/share/mime/globs",
            "/usr/local/share/mime/globs",
        ];

        for path in &candidate_paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with('#') || line.is_empty() {
                        continue;
                    }
                    // globs2: "weight:mime/type:*.ext:flags" or "weight:mime/type:*.ext"
                    // globs:  "mime/type:*.ext"
                    let cols: Vec<&str> = line.split(':').collect();
                    let (mime, glob) = if cols.len() >= 3 {
                        (cols[1].trim(), cols[2].trim())
                    } else if cols.len() == 2 {
                        (cols[0].trim(), cols[1].trim())
                    } else {
                        continue;
                    };

                    if let Some(clean_ext) = glob.strip_prefix("*.") {
                        if !clean_ext.contains('*') && !clean_ext.contains('?') {
                            map.entry(clean_ext.to_ascii_lowercase())
                                .or_insert_with(|| mime.to_string());
                        }
                    }
                }
                if !map.is_empty() {
                    break;
                }
            }
        }

        // Add user-specific local definitions if present (~/.local/share/mime/globs2)
        if let Some(user_globs) = dirs::data_local_dir().map(|d| d.join("mime/globs2")) {
            if let Ok(content) = std::fs::read_to_string(user_globs) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with('#') || line.is_empty() {
                        continue;
                    }
                    let cols: Vec<&str> = line.split(':').collect();
                    if cols.len() >= 3 {
                        let mime = cols[1].trim();
                        let glob = cols[2].trim();
                        if let Some(clean_ext) = glob.strip_prefix("*.") {
                            if !clean_ext.contains('*') && !clean_ext.contains('?') {
                                map.insert(clean_ext.to_ascii_lowercase(), mime.to_string());
                            }
                        }
                    }
                }
            }
        }

        map
    });

    table.get(&ext_lower).cloned()
}
