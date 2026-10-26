//! `jkva-net` — part of the JKva engine.
//!
//! Holds the network transports. The HTTP stack arrives with the browsing
//! milestones; the first shipping piece is the **permission-gated UDP transport**
//! required by ADR-026: a game may open a UDP socket only after the user grants
//! network access to that exact application and origin, the grant is revocable and
//! scoped to the session, and quota enforcement lives here rather than in the
//! renderer.

#![deny(missing_docs)]

pub mod udp;

pub use udp::{PermissionStore, UdpError, UdpSocket};
