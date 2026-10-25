//! Selector specificity.
//!
//! Specificity is the three-component `(A, B, C)` tuple from
//! [Selectors Level 4](https://drafts.csswg.org/selectors-4/#specificity-rules):
//!
//! * `A` counts ID selectors,
//! * `B` counts class selectors, attribute selectors and pseudo-classes,
//! * `C` counts type selectors and pseudo-elements.
//!
//! Two specificities are compared lexicographically: `A` first, then `B`, then
//! `C`. The derived [`Ord`] implementation on this struct compares the fields in
//! declaration order, which is exactly that comparison, so `Ord` can be used
//! directly by the cascade.
//!
//! The universal selector `*` and combinators contribute nothing, `:where()`
//! contributes nothing at all, and `:is()`, `:not()` and `:has()` contribute the
//! specificity of their most specific argument.

use std::fmt;
use std::ops::{Add, AddAssign};

/// The specificity of a selector, as the `(ids, classes, types)` tuple.
///
/// The ordering of this type is the ordering used by the cascade: `ids` is
/// compared first, then `classes`, then `types`. That follows from the field
/// order, which is deliberate and must not be changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Specificity {
    /// Number of ID selectors (`#header`).
    pub ids: u16,
    /// Number of class selectors (`.note`), attribute selectors (`[type]`) and
    /// pseudo-classes (`:hover`).
    pub classes: u16,
    /// Number of type selectors (`div`) and pseudo-elements (`::before`).
    pub types: u16,
}

impl Specificity {
    /// The specificity of a selector that contributes nothing, such as `*` or
    /// the argument of `:where()`.
    pub const ZERO: Self = Self {
        ids: 0,
        classes: 0,
        types: 0,
    };

    /// Creates a specificity from its three components.
    #[must_use]
    pub const fn new(ids: u16, classes: u16, types: u16) -> Self {
        Self {
            ids,
            classes,
            types,
        }
    }

    /// Returns `true` when no component counts anything.
    ///
    /// Only `*`, combinators and the arguments of `:where()` are this weak.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.ids == 0 && self.classes == 0 && self.types == 0
    }

    /// Adds another specificity component-wise, saturating at [`u16::MAX`].
    ///
    /// Saturating is intentional: a malicious or generated stylesheet must not
    /// be able to overflow the counter. Real selectors never come close.
    #[must_use]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self {
            ids: self.ids.saturating_add(other.ids),
            classes: self.classes.saturating_add(other.classes),
            types: self.types.saturating_add(other.types),
        }
    }
}

impl Add for Specificity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }
}

impl AddAssign for Specificity {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.saturating_add(rhs);
    }
}

impl fmt::Display for Specificity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.ids, self.classes, self.types)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_is_lexicographic() {
        let id = Specificity::new(1, 0, 0);
        let many_classes = Specificity::new(0, 200, 0);
        let many_types = Specificity::new(0, 0, 65_000);
        assert!(id > many_classes);
        assert!(many_classes > many_types);
        assert!(many_types > Specificity::ZERO);
    }

    #[test]
    fn zero_is_the_identity_of_addition() {
        let specificity = Specificity::new(2, 3, 4);
        assert_eq!(specificity + Specificity::ZERO, specificity);
        assert!(Specificity::ZERO.is_zero());
    }

    #[test]
    fn addition_saturates_instead_of_overflowing() {
        let huge = Specificity::new(u16::MAX, u16::MAX, u16::MAX);
        assert_eq!(huge + Specificity::new(1, 1, 1), huge);
    }

    #[test]
    fn add_assign_accumulates() {
        let mut specificity = Specificity::ZERO;
        specificity += Specificity::new(1, 0, 0);
        specificity += Specificity::new(0, 2, 0);
        assert_eq!(specificity, Specificity::new(1, 2, 0));
    }
}
