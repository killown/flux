//! The built-in SVG template and the on-disk template loader.
//!
//! The embedded template is auto-populated at `~/.local/share/flux/icons/template.svg`
//! the first time the module needs it, so a user can edit the file afterwards
//! and see their edits reflected on the next run.

use std::fs;
use std::path::PathBuf;

pub(super) const EMBEDDED_DEFAULT_TEMPLATE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" version="1.1">
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

/// Reads `~/.local/share/flux/icons/template.svg`, auto-creating it with the
/// embedded default when absent.
pub(super) fn get_or_create_template() -> String {
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
