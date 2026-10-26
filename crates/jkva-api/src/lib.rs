//! Browser API surface of JKva.
//!
//! The engine exposes capabilities to applications through this crate. The first
//! shipping piece is the permission gate from ADR-026: an application can ask for
//! a capability (UDP today), and only an explicit user decision turns that
//! request into a grant. Nothing here talks to the network or the GPU; the gate
//! is the pure policy layer that the browser UI and the network process share.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod permissions;

pub use permissions::{
    Grant, Origin, OriginError, PermissionGate, PermissionKind, PermissionRequest,
};
