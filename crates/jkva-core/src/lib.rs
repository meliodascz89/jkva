//! Core primitives shared by every JKva crate: typed identifiers and arenas.
//!
//! # Invariants
//!
//! - This crate must not depend on any platform crate or on macOS frameworks
//!   (ADR-005). It is checked on Linux CI.
//! - Identifiers are opaque and carry a generation. Holding an [`Id`] never
//!   guarantees that the value is still alive; callers must go through the arena.
//! - No `unsafe` code. If an optimisation needs it, it belongs in a crate that
//!   documents the invariant and is covered by Miri.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Version of the JKva engine core.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Opaque, typed handle to a value stored in an [`Arena`].
///
/// The generation is bumped when the slot is freed, so a stale handle can never
/// observe a value that was inserted later into the same slot.
pub struct Id<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Raw slot index. Useful for logging and for compact side tables.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Generation of the slot at the time this handle was created.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl<T> Copy for Id<T> {}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({}v{})", self.index, self.generation)
    }
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

/// A dense arena with stable indices and generation-checked handles.
///
/// Removing a value does not move the others, and the freed slot is reused by the
/// next insert with a new generation. Iteration visits live values in insertion
/// order, skipping freed slots.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Arena<T> {
    /// Creates an empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }

    /// Creates an empty arena with room for `capacity` values.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free: Vec::new(),
            len: 0,
        }
    }

    /// Inserts a value and returns its handle.
    pub fn insert(&mut self, value: T) -> Id<T> {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(slot.value.is_none(), "free slot must be empty");
            slot.value = Some(value);
            self.len += 1;
            Id {
                index,
                generation: slot.generation,
                _marker: PhantomData,
            }
        } else {
            let index = u32::try_from(self.slots.len()).expect("arena exceeds u32 slots");
            self.slots.push(Slot {
                generation: 0,
                value: Some(value),
            });
            self.len += 1;
            Id {
                index,
                generation: 0,
                _marker: PhantomData,
            }
        }
    }

    /// Returns a reference to the value if the handle is still valid.
    #[must_use]
    pub fn get(&self, id: Id<T>) -> Option<&T> {
        self.slots
            .get(id.index as usize)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.value.as_ref())
    }

    /// Returns a mutable reference to the value if the handle is still valid.
    #[must_use]
    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        self.slots
            .get_mut(id.index as usize)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.value.as_mut())
    }

    /// Removes the value and invalidates the handle.
    ///
    /// Returns `None` when the handle is stale.
    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        self.len -= 1;
        Some(value)
    }

    /// Returns `true` when the handle points at a live value.
    #[must_use]
    pub fn contains(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    /// Number of live values.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when no value is live.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterates over live values in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|value| {
                (
                    Id {
                        index: u32::try_from(index).expect("arena exceeds u32 slots"),
                        generation: slot.generation,
                        _marker: PhantomData,
                    },
                    value,
                )
            })
        })
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut arena = Arena::new();
        let a = arena.insert("a");
        let b = arena.insert("b");
        assert_eq!(arena.get(a), Some(&"a"));
        assert_eq!(arena.get(b), Some(&"b"));
        assert_eq!(arena.len(), 2);
        assert!(!arena.is_empty());
    }

    #[test]
    fn get_mut_updates_the_value() {
        let mut arena = Arena::new();
        let id = arena.insert(1);
        *arena.get_mut(id).unwrap() += 41;
        assert_eq!(arena.get(id), Some(&42));
    }

    #[test]
    fn remove_invalidates_the_handle() {
        let mut arena = Arena::new();
        let id = arena.insert(7);
        assert_eq!(arena.remove(id), Some(7));
        assert_eq!(arena.get(id), None);
        assert!(!arena.contains(id));
        assert_eq!(arena.remove(id), None);
        assert!(arena.is_empty());
    }

    #[test]
    fn reused_slot_rejects_the_stale_handle() {
        let mut arena = Arena::new();
        let stale = arena.insert("old");
        arena.remove(stale);
        let fresh = arena.insert("new");
        assert_eq!(fresh.index(), stale.index());
        assert_ne!(fresh.generation(), stale.generation());
        assert_eq!(arena.get(fresh), Some(&"new"));
        assert_eq!(arena.get(stale), None);
    }

    #[test]
    fn iteration_skips_freed_slots() {
        let mut arena = Arena::new();
        let a = arena.insert(1);
        let b = arena.insert(2);
        let c = arena.insert(3);
        arena.remove(b);
        let seen: Vec<_> = arena.iter().map(|(_, v)| *v).collect();
        assert_eq!(seen, vec![1, 3]);
        assert!(arena.contains(a));
        assert!(arena.contains(c));
    }

    #[test]
    fn ids_are_comparable_and_hashable() {
        use std::collections::HashSet;
        let mut arena = Arena::new();
        let a = arena.insert(());
        let b = arena.insert(());
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b);
        assert_eq!(set.len(), 2);
        assert_eq!(a, a);
        assert_ne!(a, b);
    }
}
