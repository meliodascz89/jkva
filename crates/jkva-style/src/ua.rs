//! The built-in user-agent stylesheet.
//!
//! `jkva-style` ships a deliberately small UA sheet: enough structure for M1
//! layout to see block flow, hidden metadata elements and the classic heading
//! and list defaults. It is ordinary CSS, parsed by this crate's own parser, and
//! it lives in the *user-agent origin*, which is the weakest origin for normal
//! declarations and the strongest for `!important` ones (see
//! [`crate::cascade`]).
//!
//! # Deliberate simplifications
//!
//! * `table` and `li` are `block`. Real user agents use `table` /
//!   `list-item` (and the whole `table-*` box tree); that needs the table and
//!   list layout work, so it is marked with a `TODO` in the source below.
//! * The sheet sets no `color`, `font-family`, `font-size` or `background`
//!   defaults beyond the monospace group and the heading scale: the crate's
//!   initial values already provide `black` and a 16px font size, and there is
//!   no UA font stack or system-colour support yet.
//! * Properties this crate does not track (`white-space`, `border`, `font-style`,
//!   `text-decoration`, …) are simply absent.
//! * There is no `@media`, no `@page` and no form-control styling.
//!
//! The sheet is parsed once and shared; [`crate::cascade::StyleEngine::new`]
//! clones it into every engine.

/// The CSS source of the built-in user-agent stylesheet.
///
/// Exposed so that tests and developer tools can inspect exactly what the engine
/// applies before any author stylesheet.
///
/// ```
/// use jkva_style::{Stylesheet, ua::USER_AGENT_CSS};
///
/// let sheet = Stylesheet::parse(USER_AGENT_CSS).expect("the UA sheet must parse");
/// assert!(sheet.warnings().is_empty(), "the UA sheet must be warning-free");
/// assert!(sheet.at_rules().is_empty(), "the UA sheet has no at-rules");
/// ```
pub const USER_AGENT_CSS: &str = r#"
/* ---- Metadata and non-rendered elements ------------------------------- */
head, title, meta, style, script, link, base, template { display: none }

/* ---- Block-level elements -------------------------------------------- */
/*
 * TODO(jkva-layout/table): `table` becomes `display: table` once the table box
 * tree exists.
 */
html, body, div, p, h1, h2, h3, h4, h5, h6, ul, ol, li, blockquote, pre,
section, article, header, footer, nav, main, aside, form, table, hr, figure,
figcaption, address, fieldset, dl, dt, dd, details, summary { display: block }

/* ---- Inline-level elements ------------------------------------------- */
span, a, em, strong, b, i, code, small, sub, sup, label, abbr, cite, q, s, u,
kbd, samp, mark, time, data, bdi, bdo, var { display: inline }

/* ---- Page defaults --------------------------------------------------- */
body { margin: 8px }

/* ---- Paragraphs and quotes ------------------------------------------- */
p { margin: 1em 0 }
blockquote { margin: 1em 40px }

/* ---- Headings -------------------------------------------------------- */
h1 { font-size: 2em; margin: 0.67em 0 }
h2 { font-size: 1.5em; margin: 0.83em 0 }
h3 { font-size: 1.17em; margin: 1em 0 }
h4 { font-size: 1em; margin: 1.33em 0 }
h5 { font-size: 0.83em; margin: 1.67em 0 }
h6 { font-size: 0.67em; margin: 2.33em 0 }
h1, h2, h3, h4, h5, h6, b, strong, th { font-weight: bold }

/* ---- Lists ----------------------------------------------------------- */
/*
 * TODO(jkva-layout/list): `li` becomes `display: list-item` once marker boxes
 * exist.
 */
ul, ol { margin: 1em 0; padding-left: 40px }

/* ---- Monospace ------------------------------------------------------- */
pre { margin: 1em 0 }
code, kbd, samp, pre { font-family: monospace }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stylesheet::Stylesheet;

    #[test]
    fn the_user_agent_sheet_parses_cleanly() {
        let sheet = Stylesheet::parse(USER_AGENT_CSS).expect("the UA sheet must parse");
        assert!(
            sheet.warnings().is_empty(),
            "the UA sheet must not warn: {:?}",
            sheet.warnings()
        );
        assert!(sheet.at_rules().is_empty());
        assert!(!sheet.rules().is_empty());
    }

    #[test]
    fn the_user_agent_sheet_covers_the_structural_defaults() {
        let sheet = Stylesheet::parse(USER_AGENT_CSS).expect("parses");
        let mut display_rules = 0;
        for rule in sheet.rules() {
            for declaration in &rule.declarations {
                if declaration.name == "display" {
                    display_rules += 1;
                    assert!(
                        matches!(declaration.value.as_str(), "block" | "inline" | "none"),
                        "unexpected UA display value {:?}",
                        declaration.value
                    );
                }
            }
        }
        assert!(display_rules >= 3, "the sheet sets display in three groups");
    }
}
