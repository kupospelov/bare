use crate::debug;
use fontconfig::{Fontconfig, Pattern};
use fontdue::Font;
use std::ffi::CString;

const DEFAULT_SIZE: u32 = 10;

const FC_WEIGHT_THIN: i32 = 0;
const FC_WEIGHT_LIGHT: i32 = 50;
const FC_WEIGHT_MEDIUM: i32 = 100;
const FC_WEIGHT_BOLD: i32 = 200;
const FC_WEIGHT_BLACK: i32 = 210;

const FC_SLANT_ITALIC: i32 = 100;
const FC_SLANT_OBLIQUE: i32 = 110;

/// Convert a font point size into the corresponding pixel size.
macro_rules! pt_to_px {
    ($pt:expr) => {
        $pt * 96 / 72
    };
}

/// Resolve a Pango-style font query (e.g. "Sans Bold 8") to a font and its pixel size.
pub fn load(query: &str) -> (Font, u32) {
    let (family, weight, slant, size_px) = parse_query(query);

    let fc = Fontconfig::new().expect("Failed to init fontconfig");
    let mut pattern = Pattern::new(&fc);
    let family_c = CString::new(family).expect("Font family contains nul byte");
    pattern.add_string(c"family", &family_c);
    if let Some(w) = weight {
        pattern.add_integer(c"weight", w);
    }
    if let Some(s) = slant {
        pattern.add_integer(c"slant", s);
    }
    pattern.config_substitute();
    pattern.default_substitute();

    let matched = pattern.font_match();
    let path = matched.filename().expect("No matching font found");
    let bytes = std::fs::read(path).expect("Failed to read font");
    let font =
        Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("Failed to load font");

    debug!(
        "Font query {} resolved to {}, size {}px",
        query, path, size_px
    );
    (font, size_px)
}

fn parse_query(query: &str) -> (String, Option<i32>, Option<i32>, u32) {
    let mut tokens: Vec<&str> = query.split_whitespace().collect();
    let size = match tokens.last().and_then(|t| parse_size(t)) {
        Some(n) => {
            tokens.pop();
            n
        }
        None => pt_to_px!(DEFAULT_SIZE),
    };

    let mut weight = None;
    let mut slant = None;
    while let Some(&last) = tokens.last() {
        let matched = match last {
            "Thin" => weight.replace(FC_WEIGHT_THIN).is_none(),
            "Light" => weight.replace(FC_WEIGHT_LIGHT).is_none(),
            "Medium" => weight.replace(FC_WEIGHT_MEDIUM).is_none(),
            "Bold" => weight.replace(FC_WEIGHT_BOLD).is_none(),
            "Black" => weight.replace(FC_WEIGHT_BLACK).is_none(),
            "Italic" => slant.replace(FC_SLANT_ITALIC).is_none(),
            "Oblique" => slant.replace(FC_SLANT_OBLIQUE).is_none(),
            _ => false,
        };
        if !matched {
            break;
        }
        tokens.pop();
    }

    (tokens.join(" "), weight, slant, size)
}

/// Parse a font size token like "12", "12pt", or "12px" into pixels.
fn parse_size(token: &str) -> Option<u32> {
    if let Some(s) = token.strip_suffix("px") {
        s.parse::<u32>().ok()
    } else if let Some(s) = token.strip_suffix("pt") {
        s.parse::<u32>().ok().map(|n| pt_to_px!(n))
    } else {
        token.parse::<u32>().ok().map(|n| pt_to_px!(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_family_only() {
        assert_eq!(
            parse_query("Sans"),
            ("Sans".into(), None, None, pt_to_px!(DEFAULT_SIZE)),
        );
    }

    #[test]
    fn parses_family_with_size() {
        assert_eq!(
            parse_query("Sans 12"),
            ("Sans".into(), None, None, pt_to_px!(12)),
        );
    }

    #[test]
    fn parses_family_bold_size() {
        assert_eq!(
            parse_query("Sans Bold 8"),
            ("Sans".into(), Some(FC_WEIGHT_BOLD), None, pt_to_px!(8)),
        );
    }

    #[test]
    fn parses_multiword_family() {
        assert_eq!(
            parse_query("DejaVu Sans Bold 8"),
            (
                "DejaVu Sans".into(),
                Some(FC_WEIGHT_BOLD),
                None,
                pt_to_px!(8),
            )
        );
    }

    #[test]
    fn parses_bold_italic() {
        assert_eq!(
            parse_query("Sans Bold Italic 10"),
            (
                "Sans".into(),
                Some(FC_WEIGHT_BOLD),
                Some(FC_SLANT_ITALIC),
                pt_to_px!(10),
            )
        );
    }

    #[test]
    fn parses_multiword_bold_italic() {
        assert_eq!(
            parse_query("DejaVu Sans Bold Italic 10"),
            (
                "DejaVu Sans".into(),
                Some(FC_WEIGHT_BOLD),
                Some(FC_SLANT_ITALIC),
                pt_to_px!(10),
            )
        );
    }

    #[test]
    fn parses_font_size_pt() {
        assert_eq!(
            parse_query("Sans Bold 14pt"),
            ("Sans".into(), Some(FC_WEIGHT_BOLD), None, pt_to_px!(14)),
        );
    }

    #[test]
    fn parses_font_size_px() {
        assert_eq!(
            parse_query("Sans Bold 12px"),
            ("Sans".into(), Some(FC_WEIGHT_BOLD), None, 12),
        );
    }
}
