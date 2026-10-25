//! Font discovery, face loading and line metrics.
//!
//! This module is the glue between the three font libraries: `fontdb` knows
//! which faces exist on the machine, `harfrust` parses them for shaping and
//! metrics, and `swash` needs the raw bytes plus the face index to rasterise.
//! A [`LoadedFace`] keeps one copy of the bytes for all three.
//!
//! Nothing here is public; [`crate::FontCache`] is the public surface.

use std::sync::Arc;

use fontdb::{Database, Family, Query, Stretch, Style as DbStyle, Weight};

/// CSS generic families, mapped onto the values `fontdb` resolves itself.
///
/// `system-ui` and `-apple-system` are not CSS generic families in the strict
/// sense, but both appear in real stylesheets and both mean "the platform UI
/// font", which is the sans-serif slot on macOS.
const GENERIC_FAMILIES: [(&str, Family<'static>); 7] = [
    ("serif", Family::Serif),
    ("sans-serif", Family::SansSerif),
    ("monospace", Family::Monospace),
    ("cursive", Family::Cursive),
    ("fantasy", Family::Fantasy),
    ("system-ui", Family::SansSerif),
    ("-apple-system", Family::SansSerif),
];

/// A font face whose bytes have been read once and stay in memory.
///
/// The bytes are shared: `harfrust` parses them lazily through the same
/// [`Arc`] allocation that `swash` reads, so a 20 MB CJK font costs one copy
/// rather than one copy per consumer.
pub(crate) struct LoadedFace {
    /// Raw font file bytes, shared with [`Self::font`].
    pub(crate) data: Arc<Vec<u8>>,
    /// Index of this face inside `data`, for `.ttc`/`.otc` collections.
    pub(crate) face_index: u32,
    /// Parsed font used for shaping, metrics and glyph names.
    pub(crate) font: harfrust::Font,
    /// Design units per em, never zero.
    pub(crate) units_per_em: u16,
}

impl LoadedFace {
    /// Loads one face from `database`, returning `None` when its bytes cannot
    /// be read or do not contain a supported font at `id`.
    pub(crate) fn load(database: &Database, id: fontdb::ID) -> Option<Self> {
        let (bytes, face_index) =
            database.with_face_data(id, |data, index| (data.to_vec(), index))?;
        let data = Arc::new(bytes);
        // `read-fonts` keeps the blob behind an `Arc<dyn AsRef<[u8]>>`, so this
        // clone shares the allocation instead of copying the font.
        let shared: Arc<dyn AsRef<[u8]> + Send + Sync> = data.clone();
        let font = harfrust::Font::new(shared, face_index)?;
        let units_per_em = font.units_per_em().max(1);
        Some(Self {
            data,
            face_index,
            font,
            units_per_em,
        })
    }

    /// Ascent and descent in CSS pixels for a `size` px em.
    ///
    /// Both are positive distances from the baseline: `ascent` upwards,
    /// `descent` downwards, so the line box is `ascent + descent` tall.
    ///
    /// Fallback behaviour: a font that declares no usable line metrics (or a
    /// degenerate one with a non-positive ascender or descender) gets the CSS
    /// defaults of 0.8 em above and 0.2 em below the baseline. Callers
    /// therefore never see a zero-height line.
    pub(crate) fn line_metrics(&self, size: f32) -> (f32, f32) {
        let scale = size / f32::from(self.units_per_em);
        if let Some(line) = self.font.metrics().h_line() {
            let ascent = line.ascender.to_f64() as f32 * scale;
            let descent = -line.descender.to_f64() as f32 * scale;
            if ascent > 0.0 && descent > 0.0 {
                return (ascent, descent);
            }
        }
        (size * 0.8, size * 0.2)
    }

    /// PostScript name of one glyph, if the face has one.
    ///
    /// `read-fonts` synthesises a `gidNNN` name for fonts without a usable
    /// `post` table, so this only returns `None` for a glyph id the face does
    /// not know.
    pub(crate) fn glyph_name(&self, glyph_id: u16) -> Option<String> {
        let glyph = harfrust::GlyphId::new(u32::from(glyph_id));
        self.font
            .glyph_name(glyph)
            .map(|name| name.as_str().to_owned())
    }
}

/// Result of matching one CSS family against the system font database.
pub(crate) struct Resolution {
    /// The selected face, when the database knows any usable font.
    pub(crate) face: Option<fontdb::ID>,
    /// Why the request could not be satisfied exactly, when it could not.
    pub(crate) warning: Option<String>,
}

/// Finds the face for `family`, `weight` and `italic`.
///
/// The search order is: the family exactly as written, the generics
/// `sans-serif` and then `serif`, and finally any installed face. Every step
/// after the first records a warning that names the requested family, so a
/// page silently rendered in the wrong typeface is explainable from
/// [`crate::FontCache::warnings`].
pub(crate) fn resolve(database: &Database, family: &str, weight: u16, italic: bool) -> Resolution {
    let weight = Weight(weight.clamp(1, 1000));
    let style = if italic {
        DbStyle::Italic
    } else {
        DbStyle::Normal
    };
    let requested = family.trim().trim_matches(|c| c == '"' || c == '\'');

    if let Some(generic) = generic_family(requested) {
        let query = Query {
            families: &[generic],
            weight,
            stretch: Stretch::Normal,
            style,
        };
        if let Some(id) = database.query(&query) {
            return Resolution {
                face: Some(id),
                warning: None,
            };
        }
    } else if let Some(canonical) = canonical_family(database, requested) {
        // `fontdb` compares family names case-sensitively, so retry with the
        // spelling the database itself stores.
        let query = Query {
            families: &[Family::Name(&canonical)],
            weight,
            stretch: Stretch::Normal,
            style,
        };
        if let Some(id) = database.query(&query) {
            return Resolution {
                face: Some(id),
                warning: None,
            };
        }
    }

    for generic in [Family::SansSerif, Family::Serif] {
        let query = Query {
            families: &[generic],
            weight,
            stretch: Stretch::Normal,
            style,
        };
        if let Some(id) = database.query(&query) {
            return Resolution {
                face: Some(id),
                warning: Some(format!(
                    "font family `{requested}` is not installed; falling back to the system {} family `{}`",
                    generic_name(generic),
                    label(database, id),
                )),
            };
        }
    }

    // Last resort: some face is better than no text at all.
    if let Some(face) = database.faces().next() {
        let name = face
            .families
            .first()
            .map(|(family, _)| family.clone())
            .unwrap_or_else(|| face.post_script_name.clone());
        return Resolution {
            face: Some(face.id),
            warning: Some(format!(
                "font family `{requested}` is not installed and no generic family resolved; using `{name}`"
            )),
        };
    }

    Resolution {
        face: None,
        warning: Some(format!(
            "no fonts are installed; text in the family `{requested}` cannot be shaped"
        )),
    }
}

/// Returns the `fontdb` generic family for a CSS generic name.
fn generic_family(name: &str) -> Option<Family<'static>> {
    GENERIC_FAMILIES
        .iter()
        .find(|(keyword, _)| keyword.eq_ignore_ascii_case(name))
        .map(|(_, family)| *family)
}

/// Returns the spelling of `name` that the database stores, if any face has it.
fn canonical_family(database: &Database, name: &str) -> Option<String> {
    database
        .faces()
        .flat_map(|face| face.families.iter().map(|(family, _)| family))
        .find(|family| family.eq_ignore_ascii_case(name))
        .cloned()
}

/// A human-readable family name for a resolved face.
fn label(database: &Database, id: fontdb::ID) -> String {
    database
        .face(id)
        .and_then(|face| face.families.first())
        .map(|(family, _)| family.clone())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The CSS keyword for a `fontdb` generic family, for warning messages.
fn generic_name(family: Family<'static>) -> &'static str {
    match family {
        Family::Serif => "serif",
        Family::SansSerif => "sans-serif",
        Family::Monospace => "monospace",
        Family::Cursive => "cursive",
        Family::Fantasy => "fantasy",
        Family::Name(_) => "named",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_keywords_are_recognised_case_insensitively() {
        assert!(matches!(
            generic_family("SANS-SERIF"),
            Some(Family::SansSerif)
        ));
        assert!(matches!(
            generic_family("System-UI"),
            Some(Family::SansSerif)
        ));
        assert!(matches!(generic_family("serif"), Some(Family::Serif)));
        assert!(generic_family("Helvetica").is_none());
    }

    #[test]
    fn every_generic_has_a_label() {
        for (keyword, family) in GENERIC_FAMILIES {
            assert!(!generic_name(family).is_empty(), "{keyword} has no label");
        }
    }
}
