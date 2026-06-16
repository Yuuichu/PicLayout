use std::collections::HashSet;

use cosmic_text::{fontdb, FontSystem};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FontFaceInfo {
    pub family: String,
    pub post_script_name: String,
    pub weight: u16,
    pub style: String,
    pub monospaced: bool,
}

pub fn list_system_fonts() -> Vec<FontFaceInfo> {
    let font_system = FontSystem::new();
    let mut seen = HashSet::new();
    let mut fonts = Vec::new();

    for face in font_system.db().faces() {
        for (family, _) in &face.families {
            let style = style_name(face.style).to_string();
            let key = (
                family.to_lowercase(),
                face.post_script_name.clone(),
                face.weight.0,
                style.clone(),
            );
            if seen.insert(key) {
                fonts.push(FontFaceInfo {
                    family: family.clone(),
                    post_script_name: face.post_script_name.clone(),
                    weight: face.weight.0,
                    style,
                    monospaced: face.monospaced,
                });
            }
        }
    }

    fonts.sort_by(|a, b| {
        a.family
            .to_lowercase()
            .cmp(&b.family.to_lowercase())
            .then(a.weight.cmp(&b.weight))
            .then(a.style.cmp(&b.style))
            .then(a.post_script_name.cmp(&b.post_script_name))
    });
    fonts
}

pub fn system_has_family(family: &str) -> bool {
    if family.trim().is_empty() || is_generic_family(family) {
        return true;
    }

    let needle = family.trim();
    let font_system = FontSystem::new();
    let found = font_system.db().faces().any(|face| {
        face.families
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(needle))
    });
    found
}

pub fn is_generic_family(family: &str) -> bool {
    matches!(
        family.trim().to_ascii_lowercase().as_str(),
        "sans-serif" | "sans" | "serif" | "monospace" | "monospaced" | "cursive" | "fantasy"
    )
}

fn style_name(style: fontdb::Style) -> &'static str {
    match style {
        fontdb::Style::Normal => "normal",
        fontdb::Style::Italic => "italic",
        fontdb::Style::Oblique => "oblique",
    }
}
