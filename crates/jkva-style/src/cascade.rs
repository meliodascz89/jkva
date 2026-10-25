//! The cascade: from matched declarations to a [`ComputedStyle`] per element.
//!
//! # How a style is computed
//!
//! 1. The rule index narrows the selectors worth testing for an element (see
//!    [the rule index](#the-rule-index)), and every declaration of every
//!    selector that does match becomes a *candidate*, tagged with the sort key
//!    the CSS cascade prescribes: `(origin, specificity, source order)`.
//! 2. Candidates are sorted by that key and applied in ascending order, so the
//!    last writer wins. Origin outranks specificity, specificity outranks source
//!    order, and only four of the eight CSS origin/importance ranks exist (no
//!    user origin, no animations, no transitions).
//! 3. `margin`, `padding`, `border-width`, `border-style` and `border` shorthands are
//!    expanded into their longhands
//!    before the winner is chosen, so `margin: 1px` and `margin-top: 2px` compete
//!    per longhand.
//! 4. Elements are visited in document order, so a parent's computed style is
//!    always ready when its children are computed. Inherited properties
//!    (`color`, `font-size`, `font-weight`, `font-family`) take their value from
//!    the parent; everything else falls back to its initial value.
//! 5. `inherit`, `initial` and `unset` are resolved per property. `revert` and
//!    `revert-layer` are treated as `unset`, which is the closest behaviour
//!    available while there is no user-agent stylesheet to revert to.
//!
//! # Where values come from, and where this guesses
//!
//! * Values stay as specified-value text (see [`ComputedStyle`]); only `display`
//!   is validated and parsed into [`DisplayType`]. An unrecognised `display`
//!   keyword drops that declaration, as a browser would.
//! * The reported initial values are the CSS ones with three approximations:
//!   `color` starts as `black` (system colours are not implemented),
//!   `font-family` starts empty, meaning "user-agent default", and `font-size`
//!   starts at `16px`, which is what the keyword `medium` computes to.
//! * `var()` is never substituted. A declaration such as `color: var(--brand)`
//!   computes to the literal text `var(--brand)`, because custom properties are
//!   parsed but not resolved.
//! * Shorthands are expanded for `margin`, `padding` and the border shorthands.
//!   `background`, `font` and friends are ignored entirely, so `background: red`
//!   does **not** set `background-color`.
//! * Declarations for properties outside the tracked set are dropped, including
//!   `list-style`, `position`, `text-align` and every other property the engine
//!   does not model yet.
//! * There is no style sharing and no incremental restyle: every
//!   [`StyleEngine::compute_styles`] pass recomputes every element.
//!
//! # What one element costs
//!
//! Per element the pass does one lookup per bucket the element belongs to, one
//! full match test per bucketed selector, one sort of that element's candidates,
//! and exactly one write per tracked property. The candidate buffer is owned by
//! [`StyleEngine::compute_styles`] and reused across elements, so the only
//! per-element heap traffic left is the style itself: the 23 owned `String`
//! fields of [`ComputedStyle`], allocated once each.
//!
//! That is the floor while the fields own their text as `String`. Going lower
//! needs a shared or interned representation (`Arc<str>`), which would change
//! `ComputedStyle`'s public field types.
//!
//! # The rule index
//!
//! Every selector is filed under the most selective simple selector of its
//! **rightmost** compound selector: an `#id` bucket, a `.class` bucket, a type
//! bucket, or the *universal* bucket. For one element the cascade therefore tests
//! its id bucket, its class buckets, its type bucket and the universal bucket,
//! instead of every rule in the engine.
//!
//! Bucketing is only a filter; [`matches`] stays authoritative, so a bucket may
//! over-approximate but must never under-approximate. That is why a compound such
//! as `p:hover` or `div[data-x]` is still filed under bucket `p`/`div` — the
//! pseudo-class and attribute are re-tested by the matcher — while a compound with
//! nothing indexable (`.a:hover` is indexed by class, but `:first-child` alone,
//! `[data-x]` alone, `*` or a namespace-qualified type) falls into the universal
//! bucket and is tested for every element.
//!
//! Bucket order is irrelevant to the result: the per-element candidate set is
//! sorted by [`OrderKey`], so specificity and source order decide exactly as
//! before. The index is append-only, because stylesheets can only be added.
//!
//! # At-rules
//!
//! At-rules are parsed and then dropped by [`Stylesheet::parse`], so a rule
//! inside `@media` never reaches this cascade. The intended shape, decided but
//! not implemented, is a flat rule list where each rule carries a
//! `conditions: Vec<Condition>` (empty meaning unconditional); nested `@media`
//! blocks would flatten to a conjunction, keeping one ordered walk.
//!
//! [`Stylesheet::parse`]: crate::Stylesheet::parse

use std::collections::HashMap;
use std::sync::OnceLock;

use jkva_dom::{Document, ElementData, NodeId};

use crate::computed::{ComputedStyle, DisplayType};
use crate::declaration::Declaration;
use crate::matching::matches;
use crate::selector::{Selector, SimpleSelector};
use crate::specificity::Specificity;
use crate::stylesheet::Stylesheet;
use crate::ua::USER_AGENT_CSS;

/// The computed style of every element of one document.
///
/// Styles are stored in a flat vector indexed by the node's arena index, with the
/// full [`NodeId`] kept alongside each entry, so a lookup is generation-checked:
/// a handle whose arena slot has been reused by a *different* node reports `None`
/// instead of that node's style. Iteration is in document order, which makes the
/// map suitable for deterministic snapshot tests (ADR-016).
///
/// A map is a snapshot of the tree at the moment
/// [`StyleEngine::compute_styles`] ran. A node removed afterwards still resolves
/// to the style it had (the handle and the recorded entry still match); call
/// `compute_styles` again to observe the removal.
///
/// ```
/// use jkva_dom::parse_document;
/// use jkva_style::{StyleEngine, Stylesheet};
///
/// let document = parse_document("<p id=x>hi</p>");
/// let engine = StyleEngine::new();
/// let styles = engine.compute_styles(&document);
///
/// // Iteration follows document order, so the root <html> comes first.
/// let order: Vec<_> = styles
///     .iter()
///     .filter_map(|(node, _)| document.element(node).map(|e| e.name().to_owned()))
///     .collect();
/// assert_eq!(order, ["html", "head", "body", "p"]);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ComputedStyleMap {
    /// One slot per arena index; `None` for indices that were never styled.
    slots: Vec<Option<StyledNode>>,
    /// Arena indices of the styled nodes, in document order.
    document_order: Vec<u32>,
}

/// One styled element: the handle that identifies it plus its computed style.
#[derive(Clone, Debug)]
struct StyledNode {
    node: NodeId,
    style: ComputedStyle,
}

impl ComputedStyleMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the computed style of `node`, or `None` when it was not styled.
    ///
    /// Only elements are styled, so text and comment nodes always report `None`.
    /// A stale handle also reports `None`: the entry is matched on the full
    /// handle, generation included.
    #[must_use]
    pub fn get(&self, node: NodeId) -> Option<&ComputedStyle> {
        let styled = self.slots.get(node.index() as usize)?.as_ref()?;
        (styled.node == node).then_some(&styled.style)
    }

    /// Iterates over `(node, style)` pairs in document order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &ComputedStyle)> {
        self.document_order.iter().filter_map(|index| {
            let styled = self.slots.get(*index as usize)?.as_ref()?;
            Some((styled.node, &styled.style))
        })
    }

    /// Number of styled elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.document_order.len()
    }

    /// Returns `true` when no element was styled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.document_order.is_empty()
    }

    /// Records the style of one node.
    ///
    /// Called in document order by [`StyleEngine::compute_styles`], which is what
    /// makes [`ComputedStyleMap::iter`] document-ordered.
    fn insert(&mut self, node: NodeId, style: ComputedStyle) {
        let index = node.index() as usize;
        if self.slots.len() <= index {
            // One slot per arena index; `None` for the gap left by a removed node.
            self.slots.resize_with(index.saturating_add(1), || None);
        }
        if self.slots[index].is_none() {
            self.document_order.push(node.index());
        }
        self.slots[index] = Some(StyledNode { node, style });
    }
}

/// A reference to one selector of one rule.
///
/// The index stores references rather than copies so that a stylesheet is never
/// duplicated; `rule` indexes [`StyleEngine::rules`] and `selector` indexes that
/// rule's selector list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectorRef {
    /// Index into [`StyleEngine::rules`].
    rule: u32,
    /// Index of the selector inside the rule.
    selector: u32,
}

/// Which bucket a selector is filed under.
///
/// Derived from the rightmost compound selector only; see the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BucketKey<'a> {
    /// The compound has an id selector.
    Id(&'a str),
    /// The compound has a class selector.
    Class(&'a str),
    /// The compound has a type selector without a namespace prefix.
    Type(&'a str),
    /// Nothing indexable: tested for every element.
    Universal,
}

impl<'a> BucketKey<'a> {
    /// Picks the bucket for a selector from its rightmost compound selector.
    ///
    /// Preference order is id, then class, then type, because that is the order
    /// of decreasing selectivity. A namespace-qualified type selector is not
    /// indexable (namespaces are not resolved by the matcher), and neither is a
    /// relative selector, so both fall through to [`BucketKey::Universal`].
    fn of(selector: &'a Selector) -> Self {
        if selector.leading_combinator().is_some() {
            return Self::Universal;
        }
        let Some(component) = selector.components().last() else {
            return Self::Universal;
        };

        let mut id = None;
        let mut class = None;
        let mut type_name = None;
        for simple in &component.compound.simples {
            match simple {
                SimpleSelector::Id(value) => id = Some(value.as_str()),
                SimpleSelector::Class(value) => class = Some(value.as_str()),
                SimpleSelector::Type(value) if !value.contains('|') => {
                    type_name = Some(value.as_str());
                }
                _ => {}
            }
        }

        if let Some(id) = id {
            Self::Id(id)
        } else if let Some(class) = class {
            Self::Class(class)
        } else if let Some(type_name) = type_name {
            Self::Type(type_name)
        } else {
            Self::Universal
        }
    }
}

/// Selectors bucketed by the most selective part of their rightmost compound
/// selector.
///
/// Type buckets are keyed by the ASCII-lowercased type name so that the HTML
/// case-insensitivity of type selectors is handled once, at index time. The
/// lookup lowercases the element's name only when it actually contains an ASCII
/// uppercase byte, which foreign-content names can and HTML names cannot.
#[derive(Clone, Debug, Default)]
struct RuleIndex {
    /// Selectors whose rightmost compound has an id selector.
    by_id: HashMap<String, Vec<SelectorRef>>,
    /// Selectors whose rightmost compound has a class selector.
    by_class: HashMap<String, Vec<SelectorRef>>,
    /// Selectors whose rightmost compound has a type selector, keyed by its
    /// ASCII-lowercased name.
    by_type: HashMap<String, Vec<SelectorRef>>,
    /// Selectors that have to be tested against every element.
    universal: Vec<SelectorRef>,
}

impl RuleIndex {
    /// Files one selector under the bucket its rightmost compound selector picks.
    fn insert(&mut self, selector: &Selector, selector_ref: SelectorRef) {
        match BucketKey::of(selector) {
            BucketKey::Id(id) => self
                .by_id
                .entry(id.to_owned())
                .or_default()
                .push(selector_ref),
            BucketKey::Class(class) => self
                .by_class
                .entry(class.to_owned())
                .or_default()
                .push(selector_ref),
            BucketKey::Type(name) => self
                .by_type
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(selector_ref),
            BucketKey::Universal => self.universal.push(selector_ref),
        }
    }

    /// Returns the selectors filed under `id`.
    fn by_id(&self, id: &str) -> &[SelectorRef] {
        self.by_id.get(id).map_or(&[], Vec::as_slice)
    }

    /// Returns the selectors filed under `class`.
    fn by_class(&self, class: &str) -> &[SelectorRef] {
        self.by_class.get(class).map_or(&[], Vec::as_slice)
    }

    /// Returns the selectors filed under the element's type name.
    fn by_type(&self, element_name: &str) -> &[SelectorRef] {
        if element_name.bytes().any(|byte| byte.is_ascii_uppercase()) {
            self.by_type
                .get(element_name.to_ascii_lowercase().as_str())
                .map_or(&[], Vec::as_slice)
        } else {
            self.by_type.get(element_name).map_or(&[], Vec::as_slice)
        }
    }

    /// Returns the selectors that must be tested for every element.
    fn universal(&self) -> &[SelectorRef] {
        &self.universal
    }
}

/// Applies a set of stylesheets to a document.
///
/// The engine owns its stylesheets and pre-computes, per rule, the specificity of
/// every selector and the source-order position of every declaration, so the
/// per-element pass does no parsing work.
///
/// # Origins
///
/// Two cascade origins exist: the **user agent** (the built-in sheet from
/// [`crate::ua`], loaded by [`StyleEngine::new`]) and the **author** (everything
/// added with [`StyleEngine::add_stylesheet`]). They are ordered the way CSS
/// Cascade requires:
///
/// ```text
/// weakest ─ UA normal < author normal < author !important < UA !important ─ strongest
/// ```
///
/// So an author declaration at *equal* specificity beats the UA sheet, and only a
/// UA `!important` declaration can beat an author `!important` one.
///
/// ```
/// use jkva_dom::parse_document;
/// use jkva_style::{DisplayType, StyleEngine, Stylesheet};
///
/// let document = parse_document("<p class=lead>hi</p>");
/// let mut engine = StyleEngine::new();
/// engine.add_stylesheet(Stylesheet::parse(".lead { color: red }").unwrap());
///
/// let styles = engine.compute_styles(&document);
/// let paragraph = document
///     .descendants(document.root())
///     .find(|node| {
///         document
///             .element(*node)
///             .is_some_and(|element| element.name() == "p")
///     })
///     .expect("the document has a <p>");
/// assert_eq!(styles.get(paragraph).unwrap().color, "red");
/// // ...and the user-agent sheet still supplies the box type.
/// assert_eq!(styles.get(paragraph).unwrap().display, DisplayType::Block);
/// ```
#[derive(Clone, Debug)]
pub struct StyleEngine {
    sheets: Vec<Stylesheet>,
    rules: Vec<IndexedRule>,
    index: RuleIndex,
    next_order: u32,
}

impl Default for StyleEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// One rule of one stylesheet, with the values the cascade needs precomputed.
#[derive(Clone, Debug)]
struct IndexedRule {
    /// Index into [`StyleEngine::sheets`].
    sheet: usize,
    /// Index into the stylesheet's rule list.
    rule: usize,
    /// Which cascade origin the sheet belongs to.
    origin: SheetOrigin,
    /// Specificity of each selector of the rule, in selector order.
    specificities: Vec<Specificity>,
    /// Source-order position of the rule's first declaration.
    first_declaration_order: u32,
}

impl StyleEngine {
    /// Creates an engine with the built-in user-agent stylesheet loaded.
    ///
    /// Author stylesheets added afterwards are in the author origin and therefore
    /// outrank the UA sheet for normal declarations.
    #[must_use]
    pub fn new() -> Self {
        let mut engine = Self::without_user_agent();
        engine.add_user_agent_stylesheet(user_agent_stylesheet());
        engine
    }

    /// Creates an engine with **no** stylesheets, not even the user-agent one.
    ///
    /// Useful when a caller wants to supply its own user-agent sheet, and in
    /// tests that must isolate one origin.
    #[must_use]
    pub fn without_user_agent() -> Self {
        Self {
            sheets: Vec::new(),
            rules: Vec::new(),
            index: RuleIndex::default(),
            next_order: 0,
        }
    }

    /// Creates an engine from already parsed author stylesheets, in cascade
    /// order.
    ///
    /// The built-in user-agent sheet is loaded first, as in [`StyleEngine::new`].
    #[must_use]
    pub fn from_stylesheets<I>(sheets: I) -> Self
    where
        I: IntoIterator<Item = Stylesheet>,
    {
        let mut engine = Self::new();
        for sheet in sheets {
            engine.add_stylesheet(sheet);
        }
        engine
    }

    /// Appends an author stylesheet.
    ///
    /// Stylesheets added later win ties against earlier ones, exactly as if their
    /// text had been concatenated.
    pub fn add_stylesheet(&mut self, sheet: Stylesheet) {
        self.push_stylesheet(sheet, SheetOrigin::Author);
    }

    /// Appends a stylesheet in the user-agent origin.
    ///
    /// The built-in sheet is already present in an engine created by
    /// [`StyleEngine::new`]; this is how a caller adds or replaces UA-level
    /// rules, in particular UA `!important` declarations, which outrank every
    /// author declaration.
    pub fn add_user_agent_stylesheet(&mut self, sheet: Stylesheet) {
        self.push_stylesheet(sheet, SheetOrigin::UserAgent);
    }

    /// The stylesheets the engine knows about, in insertion order.
    ///
    /// The built-in user-agent sheet is first when the engine was created by
    /// [`StyleEngine::new`].
    #[must_use]
    pub fn sheets(&self) -> &[Stylesheet] {
        &self.sheets
    }

    /// Indexes one stylesheet's rules into the cascade.
    fn push_stylesheet(&mut self, sheet: Stylesheet, origin: SheetOrigin) {
        let sheet_index = self.sheets.len();
        let mut order = self.next_order;
        for (rule_index, rule) in sheet.rules().iter().enumerate() {
            let engine_rule = u32::try_from(self.rules.len()).unwrap_or(u32::MAX);

            // File every selector into its bucket. Doing this while the sheet is
            // still a local keeps the borrows local; the index stores indices,
            // not references into the sheet.
            for (position, selector) in rule.selectors.iter().enumerate() {
                self.index.insert(
                    selector,
                    SelectorRef {
                        rule: engine_rule,
                        selector: u32::try_from(position).unwrap_or(u32::MAX),
                    },
                );
            }

            self.rules.push(IndexedRule {
                sheet: sheet_index,
                rule: rule_index,
                origin,
                specificities: rule.selectors.iter().map(Selector::specificity).collect(),
                first_declaration_order: order,
            });
            order =
                order.saturating_add(u32::try_from(rule.declarations.len()).unwrap_or(u32::MAX));
        }
        self.next_order = order;
        self.sheets.push(sheet);
    }

    /// Computes the style of every element of `document`.
    ///
    /// The walk is a single document-order pass. Per element the cost is one
    /// lookup per bucket the element can belong to (its id, each of its classes,
    /// its type name, plus the universal bucket) followed by a full match test
    /// per *bucketed* selector, so a selector that the index can filter out is
    /// never matched. Elements whose rules are all in the universal bucket fall
    /// back to the pre-index cost of testing every such selector.
    #[must_use]
    pub fn compute_styles(&self, document: &Document) -> ComputedStyleMap {
        let mut styles = ComputedStyleMap::new();
        // One candidate buffer for the whole pass: the candidates borrow the
        // stylesheets, not the element, so the buffer survives from element to
        // element and its capacity is reused.
        let mut candidates: Vec<Candidate<'_>> = Vec::new();
        for node in document.descendants(document.root()) {
            let Some(element) = document.element(node) else {
                continue;
            };
            let parent_style = document.parent(node).and_then(|parent| styles.get(parent));
            let computed = self.compute_one(document, node, element, parent_style, &mut candidates);
            styles.insert(node, computed);
        }
        styles
    }

    /// Computes the style of one element.
    ///
    /// This still walks the whole document, because the element's inherited
    /// values depend on its ancestors. Use [`StyleEngine::compute_styles`] when
    /// more than one element is needed.
    #[must_use]
    pub fn computed_style(&self, document: &Document, node: NodeId) -> Option<ComputedStyle> {
        self.compute_styles(document).get(node).cloned()
    }

    /// Computes the style of one element, given the style of its parent.
    ///
    /// `candidates` is a caller-owned scratch buffer; reusing it across elements
    /// keeps the per-element pass allocation-free apart from the style itself.
    fn compute_one<'a>(
        &'a self,
        document: &Document,
        node: NodeId,
        element: &ElementData,
        parent: Option<&ComputedStyle>,
        candidates: &mut Vec<Candidate<'a>>,
    ) -> ComputedStyle {
        candidates.clear();
        self.collect_candidates(document, node, element, candidates);

        // Ascending sort, then apply in order: the last writer wins. Bucket order
        // never leaks into the result, because the key carries specificity and
        // source order.
        candidates.sort_by_key(|candidate| candidate.key);
        let mut declared: [Option<&str>; PropertyId::COUNT] = [None; PropertyId::COUNT];
        for candidate in candidates.iter() {
            declared[candidate.slot] = Some(candidate.value);
        }

        // Every slot is written exactly once, so starting from an empty style is
        // equivalent to starting from `ComputedStyle::initial()` — the unit test
        // `filling_every_property_from_initial_reproduces_the_initial_style`
        // proves the property table really does cover every field.
        let mut style = ComputedStyle::empty();
        for property in PropertyId::ALL {
            assign_resolved(property, declared[property.index()], parent, &mut style);
        }
        style
    }

    /// Appends every declaration whose selector could match `element`.
    ///
    /// Only the buckets the element can possibly belong to are consulted: its id,
    /// each of its classes, its type name, and the universal bucket.
    fn collect_candidates<'a>(
        &'a self,
        document: &Document,
        node: NodeId,
        element: &ElementData,
        candidates: &mut Vec<Candidate<'a>>,
    ) {
        if let Some(id) = element.attribute("id") {
            self.collect_selectors(document, node, self.index.by_id(id), candidates);
        }
        if let Some(classes) = element.attribute("class") {
            for class in classes.split_ascii_whitespace() {
                self.collect_selectors(document, node, self.index.by_class(class), candidates);
            }
        }
        self.collect_selectors(
            document,
            node,
            self.index.by_type(element.name()),
            candidates,
        );
        self.collect_selectors(document, node, self.index.universal(), candidates);
    }

    /// Tests every selector in one bucket and appends the declarations of those
    /// that match.
    fn collect_selectors<'a>(
        &'a self,
        document: &Document,
        node: NodeId,
        selectors: &[SelectorRef],
        candidates: &mut Vec<Candidate<'a>>,
    ) {
        for selector_ref in selectors {
            let Some(indexed) = self.rules.get(selector_ref.rule as usize) else {
                continue;
            };
            let Some(rule) = self
                .sheets
                .get(indexed.sheet)
                .and_then(|sheet| sheet.rules().get(indexed.rule))
            else {
                continue;
            };
            let Some(selector) = rule.selectors.get(selector_ref.selector as usize) else {
                continue;
            };
            if !matches(document, node, selector) {
                continue;
            }
            let specificity = indexed
                .specificities
                .get(selector_ref.selector as usize)
                .copied()
                .unwrap_or_default();
            for (offset, declaration) in rule.declarations.iter().enumerate() {
                let key = OrderKey {
                    origin: CascadeOrigin::new(indexed.origin, declaration.important),
                    specificity,
                    order: indexed
                        .first_declaration_order
                        .saturating_add(u32::try_from(offset).unwrap_or(u32::MAX)),
                };
                collect_declaration(declaration, key, candidates);
            }
        }
    }
}

/// A declaration that applies to an element, with its cascade sort key.
struct Candidate<'a> {
    key: OrderKey,
    /// Index of the longhand property in [`PropertyId::ALL`].
    slot: usize,
    /// The declared value text.
    value: &'a str,
}

/// The built-in user-agent stylesheet, parsed once per process.
///
/// The sheet is compiled into the binary as [`USER_AGENT_CSS`] and is covered by
/// a unit test that it parses without warnings, so the fallback to an empty sheet
/// is unreachable in practice.
fn user_agent_stylesheet() -> Stylesheet {
    static CACHED: OnceLock<Stylesheet> = OnceLock::new();
    CACHED
        .get_or_init(|| Stylesheet::parse(USER_AGENT_CSS).unwrap_or_default())
        .clone()
}

/// Which origin a stylesheet belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SheetOrigin {
    /// The built-in user-agent sheet, or one a caller installed in that origin.
    UserAgent,
    /// Document, embedder or user stylesheets.
    Author,
}

/// A declaration's position in the cascade's origin-and-importance ordering,
/// from weakest to strongest.
///
/// The variant order *is* the tie-break order: `Ord` is derived, so a UA normal
/// declaration sorts below an author normal one, which sorts below an author
/// `!important` one, which sorts below a UA `!important` one. This is the
/// "Origin and Importance" step of CSS Cascade; only these four of its eight
/// ranks exist because there is no user origin, no animations and no
/// transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CascadeOrigin {
    /// Normal declaration from the user-agent origin: the weakest of all.
    UserAgentNormal,
    /// Normal declaration from an author stylesheet.
    AuthorNormal,
    /// `!important` declaration from an author stylesheet.
    AuthorImportant,
    /// `!important` declaration from the user-agent origin: the strongest of all.
    ///
    /// This is the only rank that can beat an author `!important` declaration,
    /// which is what lets a UA sheet enforce something the page cannot override.
    UserAgentImportant,
}

impl CascadeOrigin {
    /// Ranks a declaration by its sheet's origin and its `!important` flag.
    const fn new(origin: SheetOrigin, important: bool) -> Self {
        match (origin, important) {
            (SheetOrigin::UserAgent, false) => Self::UserAgentNormal,
            (SheetOrigin::Author, false) => Self::AuthorNormal,
            (SheetOrigin::Author, true) => Self::AuthorImportant,
            (SheetOrigin::UserAgent, true) => Self::UserAgentImportant,
        }
    }
}

/// The CSS cascade sort key, ordered from weakest to strongest.
///
/// Origin outranks specificity, which outranks source order. The field order is
/// the comparison order and must not be changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OrderKey {
    origin: CascadeOrigin,
    specificity: Specificity,
    order: u32,
}

/// Turns one parsed declaration into candidates for one or more longhands.
fn collect_declaration<'a>(
    declaration: &'a Declaration,
    key: OrderKey,
    candidates: &mut Vec<Candidate<'a>>,
) {
    if let Some(property) = PropertyId::from_name(&declaration.name) {
        if property.accepts(&declaration.value) {
            candidates.push(Candidate {
                key,
                slot: property.index(),
                value: &declaration.value,
            });
        }
        return;
    }

    // Shorthands. Their longhands all share the shorthand's key, so a longhand
    // declared later in the same block still wins on source order.
    match declaration.name.as_str() {
        "margin" => expand_box(
            &declaration.value,
            [
                PropertyId::MarginTop,
                PropertyId::MarginRight,
                PropertyId::MarginBottom,
                PropertyId::MarginLeft,
            ],
            key,
            candidates,
        ),
        "padding" => expand_box(
            &declaration.value,
            [
                PropertyId::PaddingTop,
                PropertyId::PaddingRight,
                PropertyId::PaddingBottom,
                PropertyId::PaddingLeft,
            ],
            key,
            candidates,
        ),
        "border-width" => expand_box(
            &declaration.value,
            [
                PropertyId::BorderTopWidth,
                PropertyId::BorderRightWidth,
                PropertyId::BorderBottomWidth,
                PropertyId::BorderLeftWidth,
            ],
            key,
            candidates,
        ),
        "border-style" => expand_box(
            &declaration.value,
            [
                PropertyId::BorderTopStyle,
                PropertyId::BorderRightStyle,
                PropertyId::BorderBottomStyle,
                PropertyId::BorderLeftStyle,
            ],
            key,
            candidates,
        ),
        "border" => expand_border(&declaration.value, key, candidates),
        _ => {}
    }
}

/// Expands a `margin` or `padding` shorthand into its four longhands.
fn expand_box<'a>(
    value: &'a str,
    properties: [PropertyId; 4],
    key: OrderKey,
    candidates: &mut Vec<Candidate<'a>>,
) {
    let Some(values) = expand_four_sides(value) else {
        return;
    };
    for (property, value) in properties.into_iter().zip(values) {
        candidates.push(Candidate {
            key,
            slot: property.index(),
            value,
        });
    }
}

/// Expands the `border` shorthand into width and style longhands.
///
/// The colour component is accepted but ignored: this crate tracks no
/// `border-color`, so paint uses the element's text colour (`currentColor`).
/// Widths default to `medium`, styles to `none`, as the shorthand requires.
fn expand_border<'a>(value: &'a str, key: OrderKey, candidates: &mut Vec<Candidate<'a>>) {
    let mut width = "medium";
    let mut style = "none";
    for token in split_top_level(value) {
        if BorderStyleKeyword::from_str(token).is_some() {
            style = token;
        } else if looks_like_length(token) {
            width = token;
        }
    }
    for property in [
        PropertyId::BorderTopWidth,
        PropertyId::BorderRightWidth,
        PropertyId::BorderBottomWidth,
        PropertyId::BorderLeftWidth,
    ] {
        candidates.push(Candidate {
            key,
            slot: property.index(),
            value: width,
        });
    }
    for property in [
        PropertyId::BorderTopStyle,
        PropertyId::BorderRightStyle,
        PropertyId::BorderBottomStyle,
        PropertyId::BorderLeftStyle,
    ] {
        candidates.push(Candidate {
            key,
            slot: property.index(),
            value: style,
        });
    }
}

/// Returns `true` for tokens that can be a border width.
fn looks_like_length(token: &str) -> bool {
    let token = token.trim();
    token == "0"
        || matches!(token, "thin" | "medium" | "thick")
        || ["px", "em", "rem", "pt"]
            .iter()
            .any(|unit| token.ends_with(unit))
}

/// Border style keywords accepted by the `border` shorthand.
struct BorderStyleKeyword;

impl BorderStyleKeyword {
    fn from_str(value: &str) -> Option<()> {
        match value {
            "none" | "hidden" | "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge"
            | "inset" | "outset" => Some(()),
            _ => None,
        }
    }
}

/// Applies the CSS one-to-four-value syntax of the box shorthands.
///
/// Returns `(top, right, bottom, left)`, or `None` when the value has more than
/// four components and is therefore invalid.
fn expand_four_sides(value: &str) -> Option<[&str; 4]> {
    let parts = split_top_level(value);
    match parts.as_slice() {
        [all] => Some([all, all, all, all]),
        [vertical, horizontal] => Some([vertical, horizontal, vertical, horizontal]),
        [top, horizontal, bottom] => Some([top, horizontal, bottom, horizontal]),
        [top, right, bottom, left] => Some([top, right, bottom, left]),
        _ => None,
    }
}

/// Splits a value on top-level ASCII whitespace.
///
/// Whitespace inside parentheses, square brackets, curly brackets or quotes does
/// not split, so `calc(1px + 2px) auto` is two components and `rgb(1, 2, 3)` is
/// one. Unbalanced brackets are not diagnosed; they simply yield fewer
/// components, which the callers treat as an invalid value.
fn split_top_level(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut quote: Option<char> = None;
    let mut start: Option<usize> = None;

    for (index, character) in value.char_indices() {
        if let Some(open) = quote {
            if character == open {
                quote = None;
            }
            start.get_or_insert(index);
            continue;
        }
        match character {
            '"' | '\'' => {
                quote = Some(character);
                start.get_or_insert(index);
            }
            '(' | '[' | '{' => {
                depth = depth.saturating_add(1);
                start.get_or_insert(index);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                start.get_or_insert(index);
            }
            character if character.is_ascii_whitespace() && depth == 0 => {
                if let Some(begin) = start.take() {
                    parts.push(&value[begin..index]);
                }
            }
            _ => {
                start.get_or_insert(index);
            }
        }
    }
    if let Some(begin) = start {
        parts.push(&value[begin..]);
    }
    parts
}

/// A CSS-wide keyword.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WideKeyword {
    /// Use the parent's value.
    Inherit,
    /// Use the property's initial value.
    Initial,
    /// `inherit` for inherited properties, `initial` otherwise.
    Unset,
    /// Roll back to the previous origin. Treated as [`WideKeyword::Unset`].
    Revert,
    /// Roll back to the previous cascade layer. Treated as [`WideKeyword::Unset`].
    RevertLayer,
}

impl WideKeyword {
    /// Recognises a CSS-wide keyword, ignoring ASCII case and surrounding space.
    fn from_value(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("inherit") {
            Some(Self::Inherit)
        } else if value.eq_ignore_ascii_case("initial") {
            Some(Self::Initial)
        } else if value.eq_ignore_ascii_case("unset") {
            Some(Self::Unset)
        } else if value.eq_ignore_ascii_case("revert") {
            Some(Self::Revert)
        } else if value.eq_ignore_ascii_case("revert-layer") {
            Some(Self::RevertLayer)
        } else {
            None
        }
    }
}

/// Reads the value a property inherits, falling back to its initial value.
///
/// Borrows rather than allocating: the caller copies it into the style exactly
/// once.
fn inherited_value(property: PropertyId, parent: Option<&ComputedStyle>) -> &str {
    match parent {
        Some(parent) => property.value_of(parent),
        None => property.initial(),
    }
}

/// Resolves one property and writes it into `style`, allocating only the field's
/// own `String`.
///
/// This replaces an earlier `resolve_value() -> String` plus `assign(&str)` pair
/// that allocated twice per property: the candidate value is now borrowed — from
/// the declaration, from the parent's style or from the initial-value table — and
/// copied once.
fn assign_resolved(
    property: PropertyId,
    declared: Option<&str>,
    parent: Option<&ComputedStyle>,
    style: &mut ComputedStyle,
) {
    let source = match declared {
        Some(value) => match WideKeyword::from_value(value) {
            Some(WideKeyword::Inherit) => inherited_value(property, parent),
            Some(WideKeyword::Initial) => property.initial(),
            Some(WideKeyword::Unset | WideKeyword::Revert | WideKeyword::RevertLayer) => {
                if property.is_inherited() {
                    inherited_value(property, parent)
                } else {
                    property.initial()
                }
            }
            // The common case: the declared text *is* the computed text.
            None => value,
        },
        None => {
            if property.is_inherited() {
                inherited_value(property, parent)
            } else {
                property.initial()
            }
        }
    };
    property.assign(style, source);
}

/// The longhand properties this crate computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PropertyId {
    Display,
    Color,
    BackgroundColor,
    Width,
    Height,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BorderTopStyle,
    BorderRightStyle,
    BorderBottomStyle,
    BorderLeftStyle,
    FontSize,
    FontWeight,
    FontFamily,
}

impl PropertyId {
    /// Number of tracked properties.
    const COUNT: usize = 24;

    /// Every tracked property, ordered by [`PropertyId::index`].
    const ALL: [Self; Self::COUNT] = [
        Self::Display,
        Self::Color,
        Self::BackgroundColor,
        Self::Width,
        Self::Height,
        Self::MarginTop,
        Self::MarginRight,
        Self::MarginBottom,
        Self::MarginLeft,
        Self::PaddingTop,
        Self::PaddingRight,
        Self::PaddingBottom,
        Self::PaddingLeft,
        Self::BorderTopWidth,
        Self::BorderRightWidth,
        Self::BorderBottomWidth,
        Self::BorderLeftWidth,
        Self::BorderTopStyle,
        Self::BorderRightStyle,
        Self::BorderBottomStyle,
        Self::BorderLeftStyle,
        Self::FontSize,
        Self::FontWeight,
        Self::FontFamily,
    ];

    /// Slot of this property in the per-element declared-value array.
    const fn index(self) -> usize {
        match self {
            Self::Display => 0,
            Self::Color => 1,
            Self::BackgroundColor => 2,
            Self::Width => 3,
            Self::Height => 4,
            Self::MarginTop => 5,
            Self::MarginRight => 6,
            Self::MarginBottom => 7,
            Self::MarginLeft => 8,
            Self::PaddingTop => 9,
            Self::PaddingRight => 10,
            Self::PaddingBottom => 11,
            Self::PaddingLeft => 12,
            Self::BorderTopWidth => 13,
            Self::BorderRightWidth => 14,
            Self::BorderBottomWidth => 15,
            Self::BorderLeftWidth => 16,
            Self::BorderTopStyle => 17,
            Self::BorderRightStyle => 18,
            Self::BorderBottomStyle => 19,
            Self::BorderLeftStyle => 20,
            Self::FontSize => 21,
            Self::FontWeight => 22,
            Self::FontFamily => 23,
        }
    }

    /// Maps a longhand property name to its id.
    ///
    /// Shorthands and unknown properties return `None`.
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "display" => Self::Display,
            "color" => Self::Color,
            "background-color" => Self::BackgroundColor,
            "width" => Self::Width,
            "height" => Self::Height,
            "margin-top" => Self::MarginTop,
            "margin-right" => Self::MarginRight,
            "margin-bottom" => Self::MarginBottom,
            "margin-left" => Self::MarginLeft,
            "padding-top" => Self::PaddingTop,
            "padding-right" => Self::PaddingRight,
            "padding-bottom" => Self::PaddingBottom,
            "padding-left" => Self::PaddingLeft,
            "border-top-width" => Self::BorderTopWidth,
            "border-right-width" => Self::BorderRightWidth,
            "border-bottom-width" => Self::BorderBottomWidth,
            "border-left-width" => Self::BorderLeftWidth,
            "border-top-style" => Self::BorderTopStyle,
            "border-right-style" => Self::BorderRightStyle,
            "border-bottom-style" => Self::BorderBottomStyle,
            "border-left-style" => Self::BorderLeftStyle,
            "font-size" => Self::FontSize,
            "font-weight" => Self::FontWeight,
            "font-family" => Self::FontFamily,
            _ => return None,
        })
    }

    /// Returns `true` when the property inherits by default.
    const fn is_inherited(self) -> bool {
        matches!(
            self,
            Self::Color | Self::FontSize | Self::FontWeight | Self::FontFamily
        )
    }

    /// The property's CSS initial value, as text.
    ///
    /// `display` reports its keyword so that it can travel through the same
    /// string path as every other property; [`PropertyId::assign`] parses it
    /// back.
    const fn initial(self) -> &'static str {
        match self {
            Self::Display => "inline",
            Self::Color => "black",
            Self::BackgroundColor => "transparent",
            Self::Width | Self::Height => "auto",
            Self::MarginTop
            | Self::MarginRight
            | Self::MarginBottom
            | Self::MarginLeft
            | Self::PaddingTop
            | Self::PaddingRight
            | Self::PaddingBottom
            | Self::PaddingLeft => "0",
            Self::BorderTopWidth
            | Self::BorderRightWidth
            | Self::BorderBottomWidth
            | Self::BorderLeftWidth => "0",
            Self::BorderTopStyle
            | Self::BorderRightStyle
            | Self::BorderBottomStyle
            | Self::BorderLeftStyle => "none",
            // `medium` computes to 16px under default font settings; this crate
            // does not absolutise lengths, so the two spellings differ once a
            // document declares `font-size: medium` explicitly.
            Self::FontSize => "16px",
            Self::FontWeight => "normal",
            Self::FontFamily => "",
        }
    }

    /// Returns `true` when the declared value is acceptable for this property.
    ///
    /// Only `display` has a value grammar here; every other property accepts any
    /// text because there is no value parser yet.
    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Display => {
                DisplayType::from_keyword(value).is_some()
                    || WideKeyword::from_value(value).is_some()
            }
            _ => true,
        }
    }

    /// Reads this property out of a computed style.
    fn value_of(self, style: &ComputedStyle) -> &str {
        match self {
            Self::Display => style.display.as_str(),
            Self::Color => &style.color,
            Self::BackgroundColor => &style.background_color,
            Self::Width => &style.width,
            Self::Height => &style.height,
            Self::MarginTop => &style.margin_top,
            Self::MarginRight => &style.margin_right,
            Self::MarginBottom => &style.margin_bottom,
            Self::MarginLeft => &style.margin_left,
            Self::PaddingTop => &style.padding_top,
            Self::PaddingRight => &style.padding_right,
            Self::PaddingBottom => &style.padding_bottom,
            Self::PaddingLeft => &style.padding_left,
            Self::BorderTopWidth => &style.border_top_width,
            Self::BorderRightWidth => &style.border_right_width,
            Self::BorderBottomWidth => &style.border_bottom_width,
            Self::BorderLeftWidth => &style.border_left_width,
            Self::BorderTopStyle => &style.border_top_style,
            Self::BorderRightStyle => &style.border_right_style,
            Self::BorderBottomStyle => &style.border_bottom_style,
            Self::BorderLeftStyle => &style.border_left_style,
            Self::FontSize => &style.font_size,
            Self::FontWeight => &style.font_weight,
            Self::FontFamily => &style.font_family,
        }
    }

    /// Writes this property into a computed style.
    ///
    /// A `display` value that does not parse is ignored; callers are expected to
    /// have filtered it with [`PropertyId::accepts`] already.
    fn assign(self, style: &mut ComputedStyle, value: &str) {
        match self {
            Self::Display => {
                if let Some(display) = DisplayType::from_keyword(value) {
                    style.display = display;
                }
            }
            Self::Color => style.color = value.to_owned(),
            Self::BackgroundColor => style.background_color = value.to_owned(),
            Self::Width => style.width = value.to_owned(),
            Self::Height => style.height = value.to_owned(),
            Self::MarginTop => style.margin_top = value.to_owned(),
            Self::MarginRight => style.margin_right = value.to_owned(),
            Self::MarginBottom => style.margin_bottom = value.to_owned(),
            Self::MarginLeft => style.margin_left = value.to_owned(),
            Self::PaddingTop => style.padding_top = value.to_owned(),
            Self::PaddingRight => style.padding_right = value.to_owned(),
            Self::PaddingBottom => style.padding_bottom = value.to_owned(),
            Self::PaddingLeft => style.padding_left = value.to_owned(),
            Self::BorderTopWidth => style.border_top_width = value.to_owned(),
            Self::BorderRightWidth => style.border_right_width = value.to_owned(),
            Self::BorderBottomWidth => style.border_bottom_width = value.to_owned(),
            Self::BorderLeftWidth => style.border_left_width = value.to_owned(),
            Self::BorderTopStyle => style.border_top_style = value.to_owned(),
            Self::BorderRightStyle => style.border_right_style = value.to_owned(),
            Self::BorderBottomStyle => style.border_bottom_style = value.to_owned(),
            Self::BorderLeftStyle => style.border_left_style = value.to_owned(),
            Self::FontSize => style.font_size = value.to_owned(),
            Self::FontWeight => style.font_weight = value.to_owned(),
            Self::FontFamily => style.font_family = value.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_property_has_a_unique_slot_in_ascending_order() {
        for (expected, property) in PropertyId::ALL.into_iter().enumerate() {
            assert_eq!(property.index(), expected, "{property:?}");
        }
        let mut slots: Vec<_> = PropertyId::ALL.iter().map(|p| p.index()).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), PropertyId::COUNT);
    }

    #[test]
    fn property_names_round_trip_through_from_name() {
        for property in PropertyId::ALL {
            if property == PropertyId::Display {
                continue;
            }
            let style = ComputedStyle::initial();
            // `value_of` and `from_name` must agree about every property, which
            // is what ties the read and write halves of the cascade together.
            let name = PropertyId::from_name(dashed_name(property));
            assert_eq!(name, Some(property), "{property:?}");
            let _ = property.value_of(&style);
        }
    }

    /// The CSS name of a property, used only by the test above.
    fn dashed_name(property: PropertyId) -> &'static str {
        match property {
            PropertyId::Display => "display",
            PropertyId::Color => "color",
            PropertyId::BackgroundColor => "background-color",
            PropertyId::Width => "width",
            PropertyId::Height => "height",
            PropertyId::MarginTop => "margin-top",
            PropertyId::MarginRight => "margin-right",
            PropertyId::MarginBottom => "margin-bottom",
            PropertyId::MarginLeft => "margin-left",
            PropertyId::PaddingTop => "padding-top",
            PropertyId::PaddingRight => "padding-right",
            PropertyId::PaddingBottom => "padding-bottom",
            PropertyId::PaddingLeft => "padding-left",
            PropertyId::BorderTopWidth => "border-top-width",
            PropertyId::BorderRightWidth => "border-right-width",
            PropertyId::BorderBottomWidth => "border-bottom-width",
            PropertyId::BorderLeftWidth => "border-left-width",
            PropertyId::BorderTopStyle => "border-top-style",
            PropertyId::BorderRightStyle => "border-right-style",
            PropertyId::BorderBottomStyle => "border-bottom-style",
            PropertyId::BorderLeftStyle => "border-left-style",
            PropertyId::FontSize => "font-size",
            PropertyId::FontWeight => "font-weight",
            PropertyId::FontFamily => "font-family",
        }
    }

    #[test]
    fn four_side_shorthand_uses_the_css_expansion_rules() {
        assert_eq!(expand_four_sides("1px"), Some(["1px"; 4]));
        assert_eq!(
            expand_four_sides("1px 2px"),
            Some(["1px", "2px", "1px", "2px"])
        );
        assert_eq!(
            expand_four_sides("1px 2px 3px"),
            Some(["1px", "2px", "3px", "2px"])
        );
        assert_eq!(
            expand_four_sides("1px 2px 3px 4px"),
            Some(["1px", "2px", "3px", "4px"])
        );
        assert_eq!(expand_four_sides("1px 2px 3px 4px 5px"), None);
        assert_eq!(expand_four_sides("   "), None);
    }

    #[test]
    fn shorthand_splitting_respects_functions_and_strings() {
        assert_eq!(
            expand_four_sides("calc(1px + 2px) auto"),
            Some(["calc(1px + 2px)", "auto", "calc(1px + 2px)", "auto"])
        );
        assert_eq!(split_top_level("rgb(1, 2, 3) x"), vec!["rgb(1, 2, 3)", "x"]);
        assert_eq!(split_top_level(r#""a b" c"#), vec![r#""a b""#, "c"]);
        assert_eq!(split_top_level("  a \t b \n"), vec!["a", "b"]);
        assert!(split_top_level("   ").is_empty());
    }

    #[test]
    fn wide_keywords_are_recognised_case_insensitively() {
        assert_eq!(
            WideKeyword::from_value("inherit"),
            Some(WideKeyword::Inherit)
        );
        assert_eq!(
            WideKeyword::from_value("  INHERIT "),
            Some(WideKeyword::Inherit)
        );
        assert_eq!(
            WideKeyword::from_value("Revert-Layer"),
            Some(WideKeyword::RevertLayer)
        );
        assert_eq!(WideKeyword::from_value("red"), None);
        assert_eq!(WideKeyword::from_value("var(--x)"), None);
    }

    #[test]
    fn filling_every_property_from_initial_reproduces_the_initial_style() {
        // The per-element hot path starts from `ComputedStyle::empty()` and then
        // writes every slot exactly once, so it is only correct while the
        // property table covers *every* field of `ComputedStyle`. A field added
        // without a matching `PropertyId` leaves an empty string behind and makes
        // this comparison fail.
        let mut style = ComputedStyle::empty();
        for property in PropertyId::ALL {
            property.assign(&mut style, property.initial());
        }
        assert_eq!(style, ComputedStyle::initial());
    }

    #[test]
    fn every_property_initial_value_agrees_with_computed_style_initial() {
        // `PropertyId::initial` and `ComputedStyle::initial` are two views of the
        // same table; a mismatch would make `initial` resolve differently from an
        // unstyled element.
        let style = ComputedStyle::initial();
        for property in PropertyId::ALL {
            assert_eq!(
                property.initial(),
                property.value_of(&style),
                "{property:?} disagrees with ComputedStyle::initial()"
            );
        }
    }

    #[test]
    fn inherited_and_initial_values_are_per_property() {
        assert!(PropertyId::Color.is_inherited());
        assert!(PropertyId::FontFamily.is_inherited());
        assert!(!PropertyId::MarginTop.is_inherited());
        assert!(!PropertyId::Display.is_inherited());
        assert_eq!(PropertyId::Color.initial(), "black");
        assert_eq!(PropertyId::Display.initial(), "inline");
    }

    #[test]
    fn display_is_the_only_property_with_a_value_grammar() {
        assert!(PropertyId::Display.accepts("block"));
        assert!(PropertyId::Display.accepts("inherit"));
        assert!(!PropertyId::Display.accepts("blocc"));
        // No value parser exists for anything else, so any text is accepted.
        assert!(PropertyId::Width.accepts("not a length"));
    }
}
