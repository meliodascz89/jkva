//! Minimal CSS value parsing for the M1 pipeline.
//!
//! The cascade stores declared values as text; layout needs numbers. This module
//! is the honest minimum: physical lengths and the colours the fixtures use.
//! Percentages and unknown units return `None` instead of guessing.

use jkva_layout::{Display, Length};
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

/// Parses a declared width/height into a layout [`Length`].
///
/// `auto`, keywords and percentages resolve to what layout can use; unknown units
/// fall back to `Auto` rather than to a wrong number.
#[must_use]
pub fn parse_length_value(value: &str, base: f32) -> Length {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || matches!(
            value.as_str(),
            "auto" | "none" | "inherit" | "initial" | "unset" | "revert"
        )
    {
        return Length::Auto;
    }
    if let Some(percent) = value.strip_suffix('%') {
        return percent.trim().parse().map_or(Length::Auto, Length::Percent);
    }
    parse_length(&value, base).map_or(Length::Auto, Length::Px)
}

/// Parses a border width, including the `thin`/`medium`/`thick` keywords.
#[must_use]
pub fn parse_border_width(value: &str, base: f32) -> f32 {
    match value.trim().to_ascii_lowercase().as_str() {
        "thin" => 1.0,
        "medium" => 3.0,
        "thick" => 5.0,
        other => parse_length(other, base).unwrap_or(0.0),
    }
}

/// Parses the colours used by the fixtures: names, hex and `rgb()`.
///
/// Unknown values return `None`; callers decide whether that means black or
/// transparent.
#[must_use]
pub fn parse_color(value: &str) -> Option<[u8; 4]> {
    let value = strip_comments(&value.to_ascii_lowercase());
    if value.is_empty() || value == "transparent" {
        return None;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(rest) = value.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        return parse_rgb(rest, false);
    }
    if let Some(rest) = value
        .strip_prefix("rgba(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_rgb(rest, true);
    }
    if let Some(rest) = value.strip_prefix("hsl(").and_then(|v| v.strip_suffix(')')) {
        return parse_hsl(rest, false);
    }
    if let Some(rest) = value
        .strip_prefix("hsla(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_hsl(rest, true);
    }
    named_color(&value).map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
}

/// Removes CSS comments, which may legally separate tokens inside a colour.
fn strip_comments(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        // A comment is a token separator, not an empty string.
        out.push(' ');
        match rest[start..].find("*/") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
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
        4 => {
            let mut chars = hex.chars();
            Some([
                expand(chars.next()?)?,
                expand(chars.next()?)?,
                expand(chars.next()?)?,
                expand(chars.next()?)?,
            ])
        }
        6 => {
            let n = u32::from_str_radix(hex, 16).ok()?;
            Some([(n >> 16) as u8, (n >> 8) as u8, n as u8, 255])
        }
        8 => {
            let n = u32::from_str_radix(hex, 16).ok()?;
            Some([(n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8])
        }
        _ => None,
    }
}

/// Splits `rgb()` arguments, accepting both the comma and the space syntax.
fn split_color_args(rest: &str) -> Vec<String> {
    if rest.contains(',') {
        rest.split(',').map(|p| p.trim().to_string()).collect()
    } else {
        rest.split(['/', ' '])
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn channel(value: &str) -> Option<f64> {
    let value = value.trim();
    let number: f64 = value
        .strip_suffix('%')
        .map_or_else(|| value.parse(), |p| p.trim().parse::<f64>())
        .ok()?;
    let scaled = if value.ends_with('%') {
        number / 100.0 * 255.0
    } else {
        number
    };
    Some(scaled.clamp(0.0, 255.0).round())
}

fn alpha(value: &str) -> Option<f64> {
    let value = value.trim();
    let number: f64 = value
        .strip_suffix('%')
        .map_or_else(|| value.parse(), |p| p.trim().parse::<f64>())
        .ok()?;
    let scaled = if value.ends_with('%') {
        number / 100.0
    } else {
        number
    };
    Some(scaled.clamp(0.0, 1.0))
}

fn parse_rgb(rest: &str, _has_alpha: bool) -> Option<[u8; 4]> {
    let parts = split_color_args(rest);
    if !matches!(parts.len(), 3 | 4) {
        return None;
    }
    let r = channel(&parts[0])? as u8;
    let g = channel(&parts[1])? as u8;
    let b = channel(&parts[2])? as u8;
    let a = if parts.len() == 4 {
        (alpha(&parts[3])? * 255.0).round() as u8
    } else {
        255
    };
    Some([r, g, b, a])
}

fn parse_hsl(rest: &str, _has_alpha: bool) -> Option<[u8; 4]> {
    let parts = split_color_args(rest);
    if !matches!(parts.len(), 3 | 4) {
        return None;
    }
    let hue_text = parts[0].trim();
    let hue = if let Some(v) = hue_text.strip_suffix("deg") {
        v.parse::<f64>().ok()?
    } else if let Some(v) = hue_text.strip_suffix("turn") {
        v.parse::<f64>().ok()? * 360.0
    } else if let Some(v) = hue_text.strip_suffix("rad") {
        v.parse::<f64>().ok()?.to_degrees()
    } else if let Some(v) = hue_text.strip_suffix("grad") {
        v.parse::<f64>().ok()? * 0.9
    } else {
        hue_text.parse::<f64>().ok()?
    };
    let saturation = parts[1].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
    let lightness = parts[2].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
    let a = if parts.len() == 4 {
        (alpha(&parts[3])? * 255.0).round() as u8
    } else {
        255
    };
    let (r, g, b) = hsl_to_rgb(hue, saturation.clamp(0.0, 1.0), lightness.clamp(0.0, 1.0));
    Some([r, g, b, a])
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> (u8, u8, u8) {
    let h = hue.rem_euclid(360.0) / 360.0;
    if saturation == 0.0 {
        let grey = (lightness * 255.0).round() as u8;
        return (grey, grey, grey);
    }
    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    let channel = |t: f64| {
        let t = t.rem_euclid(1.0);
        let value = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (value * 255.0).round().clamp(0.0, 255.0) as u8
    };
    (channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0))
}

/// The CSS named colours this engine knows, with their sRGB values.
fn named_color(name: &str) -> Option<[u8; 3]> {
    const NAMED: &[(&str, [u8; 3])] = &[
        ("aliceblue", [240, 248, 255]),
        ("aqua", [0, 255, 255]),
        ("aquamarine", [127, 255, 212]),
        ("azure", [240, 255, 255]),
        ("beige", [245, 245, 220]),
        ("black", [0, 0, 0]),
        ("blue", [0, 0, 255]),
        ("blueviolet", [138, 43, 226]),
        ("brown", [165, 42, 42]),
        ("burlywood", [222, 184, 135]),
        ("cadetblue", [95, 158, 160]),
        ("chartreuse", [127, 255, 0]),
        ("chocolate", [210, 105, 30]),
        ("coral", [255, 127, 80]),
        ("cornflowerblue", [100, 149, 237]),
        ("cornsilk", [255, 248, 220]),
        ("crimson", [220, 20, 60]),
        ("cyan", [0, 255, 255]),
        ("darkblue", [0, 0, 139]),
        ("darkcyan", [0, 139, 139]),
        ("darkgoldenrod", [184, 134, 11]),
        ("darkgray", [169, 169, 169]),
        ("darkgreen", [0, 100, 0]),
        ("darkgrey", [169, 169, 169]),
        ("darkkhaki", [189, 183, 107]),
        ("darkmagenta", [139, 0, 139]),
        ("darkolivegreen", [85, 107, 47]),
        ("darkorange", [255, 140, 0]),
        ("darkorchid", [153, 50, 204]),
        ("darkred", [139, 0, 0]),
        ("darksalmon", [233, 150, 122]),
        ("darkseagreen", [143, 188, 143]),
        ("darkslateblue", [72, 61, 139]),
        ("darkslategray", [47, 79, 79]),
        ("darkslategrey", [47, 79, 79]),
        ("darkturquoise", [0, 206, 209]),
        ("darkviolet", [148, 0, 211]),
        ("deeppink", [255, 20, 147]),
        ("deepskyblue", [0, 191, 255]),
        ("dimgray", [105, 105, 105]),
        ("dimgrey", [105, 105, 105]),
        ("dodgerblue", [30, 144, 255]),
        ("firebrick", [178, 34, 34]),
        ("floralwhite", [255, 250, 240]),
        ("forestgreen", [34, 139, 34]),
        ("fuchsia", [255, 0, 255]),
        ("gainsboro", [220, 220, 220]),
        ("ghostwhite", [248, 248, 255]),
        ("gold", [255, 215, 0]),
        ("goldenrod", [218, 165, 32]),
        ("gray", [128, 128, 128]),
        ("green", [0, 128, 0]),
        ("greenyellow", [173, 255, 47]),
        ("grey", [128, 128, 128]),
        ("honeydew", [240, 255, 240]),
        ("hotpink", [255, 105, 180]),
        ("indianred", [205, 92, 92]),
        ("indigo", [75, 0, 130]),
        ("ivory", [255, 255, 240]),
        ("khaki", [240, 230, 140]),
        ("lavender", [230, 230, 250]),
        ("lavenderblush", [255, 240, 245]),
        ("lawngreen", [124, 252, 0]),
        ("lemonchiffon", [255, 250, 205]),
        ("lightblue", [173, 216, 230]),
        ("lightcoral", [240, 128, 128]),
        ("lightcyan", [224, 255, 255]),
        ("lightgoldenrodyellow", [250, 250, 210]),
        ("lightgray", [211, 211, 211]),
        ("lightgreen", [144, 238, 144]),
        ("lightgrey", [211, 211, 211]),
        ("lightpink", [255, 182, 193]),
        ("lightsalmon", [255, 160, 122]),
        ("lightseagreen", [32, 178, 170]),
        ("lightskyblue", [135, 206, 250]),
        ("lightslategray", [119, 136, 153]),
        ("lightslategrey", [119, 136, 153]),
        ("lightsteelblue", [176, 196, 222]),
        ("lightyellow", [255, 255, 224]),
        ("lime", [0, 255, 0]),
        ("limegreen", [50, 205, 50]),
        ("linen", [250, 240, 230]),
        ("magenta", [255, 0, 255]),
        ("maroon", [128, 0, 0]),
        ("mediumaquamarine", [102, 205, 170]),
        ("mediumblue", [0, 0, 205]),
        ("mediumorchid", [186, 85, 211]),
        ("mediumpurple", [147, 112, 219]),
        ("mediumseagreen", [60, 179, 113]),
        ("mediumslateblue", [123, 104, 238]),
        ("mediumspringgreen", [0, 250, 154]),
        ("mediumturquoise", [72, 209, 204]),
        ("mediumvioletred", [199, 21, 133]),
        ("midnightblue", [25, 25, 112]),
        ("mintcream", [245, 255, 250]),
        ("mistyrose", [255, 228, 225]),
        ("moccasin", [255, 228, 181]),
        ("navajowhite", [255, 222, 173]),
        ("navy", [0, 0, 128]),
        ("oldlace", [253, 245, 230]),
        ("olive", [128, 128, 0]),
        ("olivedrab", [107, 142, 35]),
        ("orange", [255, 165, 0]),
        ("orangered", [255, 69, 0]),
        ("orchid", [218, 112, 214]),
        ("palegoldenrod", [238, 232, 170]),
        ("palegreen", [152, 251, 152]),
        ("paleturquoise", [175, 238, 238]),
        ("palevioletred", [219, 112, 147]),
        ("papayawhip", [255, 239, 213]),
        ("peachpuff", [255, 218, 185]),
        ("peru", [205, 133, 63]),
        ("pink", [255, 192, 203]),
        ("plum", [221, 160, 221]),
        ("powderblue", [176, 224, 230]),
        ("purple", [128, 0, 128]),
        ("rebeccapurple", [102, 51, 153]),
        ("red", [255, 0, 0]),
        ("rosybrown", [188, 143, 143]),
        ("royalblue", [65, 105, 225]),
        ("saddlebrown", [139, 69, 19]),
        ("salmon", [250, 128, 114]),
        ("sandybrown", [244, 164, 96]),
        ("seagreen", [46, 139, 87]),
        ("seashell", [255, 245, 238]),
        ("sienna", [160, 82, 45]),
        ("silver", [192, 192, 192]),
        ("skyblue", [135, 206, 235]),
        ("slateblue", [106, 90, 205]),
        ("slategray", [112, 128, 144]),
        ("slategrey", [112, 128, 144]),
        ("snow", [255, 250, 250]),
        ("springgreen", [0, 255, 127]),
        ("steelblue", [70, 130, 180]),
        ("tan", [210, 180, 140]),
        ("teal", [0, 128, 128]),
        ("thistle", [216, 191, 216]),
        ("tomato", [255, 99, 71]),
        ("turquoise", [64, 224, 208]),
        ("violet", [238, 130, 238]),
        ("wheat", [245, 222, 179]),
        ("white", [255, 255, 255]),
        ("whitesmoke", [245, 245, 245]),
        ("yellow", [255, 255, 0]),
        ("yellowgreen", [154, 205, 50]),
    ];
    NAMED
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, rgb)| *rgb)
}

/// Maps a computed `display` value onto the two box types layout supports.
#[must_use]
pub fn layout_display(display: DisplayType) -> Display {
    if display.is_none() {
        Display::None
    } else if display == DisplayType::Flex {
        Display::Flex
    } else if display == DisplayType::Grid {
        Display::Grid
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
    fn length_values() {
        assert_eq!(parse_length_value("50%", 16.0), Length::Percent(50.0));
        assert_eq!(parse_length_value("200px", 16.0), Length::Px(200.0));
        assert_eq!(parse_length_value("auto", 16.0), Length::Auto);
        assert_eq!(parse_length_value("banana", 16.0), Length::Auto);
    }

    #[test]
    fn css_colour_forms() {
        assert_eq!(parse_color("#f00"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("#ff000080"), Some([255, 0, 0, 128]));
        assert_eq!(parse_color("rgb(100%, 0%, 0%)"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("rgba(0, 0, 255, 0.5)"), Some([0, 0, 255, 128]));
        assert_eq!(parse_color("hsl(120, 100%, 25%)"), Some([0, 128, 0, 255]));
        assert_eq!(parse_color("rebeccapurple"), Some([102, 51, 153, 255]));
        assert_eq!(parse_color("transparent"), None);
    }

    #[test]
    fn colours() {
        assert_eq!(parse_color("red"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("#0f0"), Some([0, 255, 0, 255]));
        assert_eq!(parse_color("#112233"), Some([17, 34, 51, 255]));
        assert_eq!(parse_color("rgb(1,2,3)"), Some([1, 2, 3, 255]));
        assert_eq!(parse_color("transparent"), None);
        assert_eq!(parse_color("chartreuse"), Some([127, 255, 0, 255]));
    }

    #[test]
    fn displays() {
        assert_eq!(layout_display(DisplayType::Block), Display::Block);
        assert_eq!(layout_display(DisplayType::Inline), Display::Inline);
        assert_eq!(layout_display(DisplayType::None), Display::None);
        assert_eq!(layout_display(DisplayType::Flex), Display::Flex);
        assert_eq!(layout_display(DisplayType::Grid), Display::Grid);
    }
}
