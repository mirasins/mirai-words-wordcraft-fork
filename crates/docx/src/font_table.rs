//! Fill Mirai/WordCraft settings that craft-docx does not yet populate from `fontTable.xml`:
//! `modern_fonts` (`w:family="modern"`) and `font_alt_names` (`w:altName`).
//!
//! craft-docx already loads `roman_fonts`; layout's `lookup_family` also needs these two lists
//! when opening a real DOCX (#Mirai Bugbot: font table alternates never loaded).

use craft_xml::El;
use wordcraft_doc::Document;

/// Longest font name kept, matching craft-docx's reader.
const MAX_FONT_NAME: usize = 256;
/// Most font-table entries kept, matching craft-docx's reader.
const MAX_TABLE_FONTS: usize = 2000;

/// East Asian charsets excluded from the modern (fixed-pitch) substitute path; layout draws those
/// in an East Asian face instead of Cambria (see `Settings::modern_fonts`).
const EAST_ASIAN_CHARSETS: &[&str] = &["80", "81", "86", "88"];

/// After a package read, load modern faces and altNames from the kept font table into `doc.settings`.
pub(crate) fn fill_from_passthrough(doc: &mut Document) {
    let Some(key) = font_table_key(&doc.passthrough) else {
        return;
    };
    let Some(bytes) = doc.passthrough.get(key) else {
        return;
    };
    let Ok(table) = craft_xml::parse(bytes) else {
        return;
    };
    apply_font_table(doc, &table);
}

fn font_table_key(passthrough: &std::collections::BTreeMap<String, std::sync::Arc<Vec<u8>>>) -> Option<&str> {
    passthrough.keys().find(|k| {
        let leaf = k.rsplit('/').next().unwrap_or(k.as_str());
        leaf.eq_ignore_ascii_case("fontTable.xml")
    }).map(String::as_str)
}

fn apply_font_table(doc: &mut Document, table: &El) {
    let mut modern = Vec::new();
    let mut alts = Vec::new();
    for font in table.children("w:font").take(MAX_TABLE_FONTS) {
        let Some(name) = font.attr("w:name").map(str::trim).filter(|n| !n.is_empty() && n.len() <= MAX_FONT_NAME) else {
            continue;
        };
        if let Some(alt) = font.child("w:altName").and_then(|a| a.attr("w:val")).map(str::trim).filter(|a| !a.is_empty() && a.len() <= MAX_FONT_NAME)
        {
            for primary in name.split(';').map(str::trim).filter(|n| !n.is_empty() && n.len() <= MAX_FONT_NAME) {
                alts.push((primary.to_string(), alt.to_string()));
            }
        }
        let family = font.child("w:family").and_then(|v| v.attr("w:val")).map(str::trim);
        if family != Some("modern") {
            continue;
        }
        let charset = font.child("w:charset").and_then(|v| v.attr("w:val")).map(str::trim);
        if charset.is_some_and(|c| EAST_ASIAN_CHARSETS.iter().any(|e| e.eq_ignore_ascii_case(c))) {
            continue;
        }
        for primary in name.split(';').map(str::trim).filter(|n| !n.is_empty() && n.len() <= MAX_FONT_NAME) {
            modern.push(primary.to_string());
        }
    }
    doc.settings.modern_fonts = modern;
    doc.settings.font_alt_names = alts;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn loads_modern_faces_and_alt_names_from_font_table() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:font w:name="Brand Mono"><w:family w:val="modern"/><w:altName w:val="Courier New"/></w:font>
  <w:font w:name="Brand Serif"><w:family w:val="roman"/></w:font>
  <w:font w:name="MS Gothic"><w:family w:val="modern"/><w:charset w:val="80"/></w:font>
</w:fonts>"#;
        let mut doc = Document::new();
        doc.passthrough.insert("word/fontTable.xml".into(), Arc::new(xml.to_vec()));
        fill_from_passthrough(&mut doc);
        assert_eq!(doc.settings.modern_fonts, ["Brand Mono"]);
        assert_eq!(doc.settings.font_alt_names, [("Brand Mono".into(), "Courier New".into())]);
    }
}
