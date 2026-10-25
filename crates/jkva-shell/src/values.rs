//! Minimal CSS value parsing for the M1 pipeline.
//!
//! The cascade stores declared values as text; layout needs numbers. This module
//! is the honest minimum: physical lengths and the colours the fixtures use.
//! Percentages and unknown units return `None` instead of guessing.

use jkva_layout::Display;
use jkva_style::DisplayType;

/// Parses a CSS length into pixels.
///
/// `base` is the element's font size, used for `em`. `rem` uses 16 px.
/// `auto`, percentages and unknown units return `None`.
#[must_use]
pub fn parse_length(value: &str, base: f32) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || matches!(
            value.as_str(),
            "auto" | "none" | "inherit" | "initial" | "unset" | "revert"
        )
    {
        return None;
    }
    if value == "0" {
        return Some(0.0);
    }
    let (number, unit) = split_unit(&value)?;
    let n: f32 = number.parse().ok()?;
    match unit {
        "px" => Some(n),
        "em" => Some(n * base),
        "rem" => Some(n * 16.0),
        "pt" => Some(n * 96.0 / 72.0),
        _ => None,
    }
}

/// Parses a `font-size` value into pixels, relative to the parent size.
#[must_use]
pub fn parse_font_size(value: &str, parent: f32) -> f32 {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "medium" => 16.0,
        "small" => 13.0,
        "x-small" => 10.0,
        "large" => 18.0,
        "x-large" => 24.0,
        "xx-large" => 32.0,
        "smaller" => parent * 0.8,
        "larger" => parent * 1.2,
        _ => parse_length(&value, parent).unwrap_or(parent),
    }
}

fn split_unit(value: &str) -> Option<(&str, &str)> {
    let split = value
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(value.len());
    if split == 0 {
        return None;
    }
    Some(value.split_at(split))
}

/// Parses the colours used by the fixtures: names, hex and `rgb()`.
///
/// Unknown values return `None`; callers decide whether that means black or
/// transparent.
#[must_use]
pub fn parse_color(value: &str) -> Option<[u8; 4]> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() || value == "transparent" {
        return None;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(rest) = value.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let parts: Vec<u8> = rest
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        if parts.len() == 3 {
            return Some([parts[0], parts[1], parts[2], 255]);
        }
        return None;
    }
    let rgb = match value.as_str() {
        "black" => [0, 0, 0],
        "white" => [255, 255, 255],
        "red" => [255, 0, 0],
        "green" => [0, 128, 0],
        "blue" => [0, 0, 255],
        "orange" => [255, 165, 0],
        "gray" | "grey" => [128, 128, 128],
        "silver" => [192, 192, 192],
        "navy" => [0, 0, 128],
        "maroon" => [128, 0, 0],
        "yellow" => [255, 255, 0],
        "purple" => [128, 0, 128],
        "teal" => [0, 128, 128],
        _ => return None,
    };
    Some([rgb[0], rgb[1], rgb[2], 255])
}

fn parse_hex(hex: &str) -> Option<[u8; 4]> {
    let expand = |c: char| -> Option<u8> {
        let v = c.to_digit(16)?;
        Some((v * 16 + v) as u8)
    };
    match hex.len() {
        3 => {
            let mut chars = hex.chars();
            Some([
                expand(chars.next()?)?,
                expand(chars.next()?)?,
                expand(chars.next()?)?,
                255,
            ])
        }
        6 => {
            let n = u32::from_str_radix(hex, 16).ok()?;
            Some([(n >> 16) as u8, (n >> 8) as u8, n as u8, 255])
        }
        _ => None,
    }
}

/// Maps a computed `display` value onto the two box types layout supports.
#[must_use]
pub fn layout_display(display: DisplayType) -> Display {
    if display.is_none() {
        Display::None
    } else if display.is_inline() {
        Display::Inline
    } else {
        Display::Block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(parse_length("16px", 16.0), Some(16.0));
        assert_eq!(parse_length("2em", 16.0), Some(32.0));
        assert_eq!(parse_length("1.5rem", 16.0), Some(24.0));
        assert_eq!(parse_length("0", 16.0), Some(0.0));
        assert_eq!(parse_length("auto", 16.0), None);
        assert_eq!(parse_length("50%", 16.0), None);
        assert_eq!(parse_length("banana", 16.0), None);
    }

    #[test]
    fn font_sizes() {
        assert_eq!(parse_font_size("16px", 16.0), 16.0);
        assert_eq!(parse_font_size("2em", 16.0), 32.0);
        assert_eq!(parse_font_size("medium", 10.0), 16.0);
        assert_eq!(parse_font_size("smaller", 20.0), 16.0);
        assert_eq!(parse_font_size("nonsense", 14.0), 14.0);
    }

    #[test]
    fn colours() {
        assert_eq!(parse_color("red"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("#0f0"), Some([0, 255, 0, 255]));
        assert_eq!(parse_color("#112233"), Some([17, 34, 51, 255]));
        assert_eq!(parse_color("rgb(1,2,3)"), Some([1, 2, 3, 255]));
        assert_eq!(parse_color("transparent"), None);
        assert_eq!(parse_color("chartreuse"), None);
    }

    #[test]
    fn displays() {
        assert_eq!(layout_display(DisplayType::Block), Display::Block);
        assert_eq!(layout_display(DisplayType::Inline), Display::Inline);
        assert_eq!(layout_display(DisplayType::None), Display::None);
        assert_eq!(layout_display(DisplayType::Flex), Display::Block);
    }
}
