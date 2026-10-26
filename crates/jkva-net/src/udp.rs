//! Permission-gated UDP for games (ADR-026).
//!
//! UDP is never a general browser capability. [`PermissionStore`] holds the
//! user's explicit grants, one per `(origin, application)` pair. Without a grant,
//! [`UdpSocket::bind`] fails; nothing listens and no socket exists.
//!
//! Quotas are enforced on every datagram: a maximum payload size and a total byte
//! budget per socket. They are deliberately conservative and belong in the network
//! process, not in the renderer.

use std::collections::HashSet;
use std::io;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::net::UdpSocket as TokioUdpSocket;

/// Largest datagram this transport sends or accepts by default, in bytes.
///
/// 1200 bytes keeps every datagram inside a typical Ethernet MTU, so a game does
/// not fragment its own packets by accident.
pub const DEFAULT_MAX_DATAGRAM: usize = 1200;

/// Default byte budget for one socket.
pub const DEFAULT_BYTE_BUDGET: u64 = 64 * 1024 * 1024;

/// Why a UDP operation was refused.
#[derive(Debug)]
pub enum UdpError {
    /// The user has not granted network access to this application and origin.
    PermissionDenied {
        /// Origin the application asked for.
        origin: String,
        /// Application identity.
        application: String,
    },
    /// The datagram is larger than the configured maximum.
    DatagramTooLarge {
        /// Payload size in bytes.
        size: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// The socket exhausted its byte budget.
    QuotaExceeded {
        /// Bytes already transferred.
        used: u64,
        /// Configured budget.
        budget: u64,
    },
    /// The operating system refused the operation.
    Io(io::Error),
}

impl std::fmt::Display for UdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied {
                origin,
                application,
            } => write!(
                f,
                "network access for {application} at {origin} was not granted"
            ),
            Self::DatagramTooLarge { size, maximum } => {
                write!(
                    f,
                    "datagram of {size} bytes exceeds the {maximum} byte limit"
                )
            }
            Self::QuotaExceeded { used, budget } => {
                write!(f, "socket used {used} of its {budget} byte budget")
            }
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for UdpError {}

impl From<io::Error> for UdpError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// The user's explicit network grants, one per application and origin.
#[derive(Debug, Default)]
pub struct PermissionStore {
    granted: Mutex<HashSet<(String, String)>>,
}

impl PermissionStore {
    /// Creates an empty store: everything is denied until granted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an explicit grant for `application` at `origin`.
    pub fn grant(&self, origin: &str, application: &str) {
        self.granted
            .lock()
            .expect("permission store lock")
            .insert((origin.to_string(), application.to_string()));
    }

    /// Removes a grant. Existing sockets keep their grant for the session; new
    /// bind attempts fail until the user grants access again.
    pub fn revoke(&self, origin: &str, application: &str) {
        self.granted
            .lock()
            .expect("permission store lock")
            .remove(&(origin.to_string(), application.to_string()));
    }

    /// Returns whether the pair is currently granted.
    #[must_use]
    pub fn is_granted(&self, origin: &str, application: &str) -> bool {
        self.granted
            .lock()
            .expect("permission store lock")
            .contains(&(origin.to_string(), application.to_string()))
    }

    /// Number of active grants.
    #[must_use]
    pub fn len(&self) -> usize {
        self.granted.lock().expect("permission store lock").len()
    }

    /// Returns `true` when no grant exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A UDP socket that exists only because the user granted it.
#[derive(Debug)]
pub struct UdpSocket {
    socket: TokioUdpSocket,
    max_datagram: usize,
    byte_budget: u64,
    used_bytes: AtomicU64,
}

impl UdpSocket {
    /// Binds `address` for `application` at `origin`, if the user granted it.
    ///
    /// # Errors
    ///
    /// Returns [`UdpError::PermissionDenied`] without a grant and [`UdpError::Io`]
    /// when the address cannot be bound.
    pub async fn bind(
        permissions: &PermissionStore,
        origin: &str,
        application: &str,
        address: &str,
    ) -> Result<Self, UdpError> {
        if !permissions.is_granted(origin, application) {
            return Err(UdpError::PermissionDenied {
                origin: origin.to_string(),
                application: application.to_string(),
            });
        }
        let socket = TokioUdpSocket::bind(address).await?;
        Ok(Self {
            socket,
            max_datagram: DEFAULT_MAX_DATAGRAM,
            byte_budget: DEFAULT_BYTE_BUDGET,
            used_bytes: AtomicU64::new(0),
        })
    }

    /// Uses a different maximum datagram size (for tests and future tuning).
    #[must_use]
    pub fn with_max_datagram(mut self, bytes: usize) -> Self {
        self.max_datagram = bytes;
        self
    }

    /// Uses a different byte budget.
    #[must_use]
    pub fn with_byte_budget(mut self, bytes: u64) -> Self {
        self.byte_budget = bytes;
        self
    }

    /// Local address the socket is bound to.
    ///
    /// # Errors
    ///
    /// Returns [`UdpError::Io`] when the operating system cannot report it.
    pub fn local_addr(&self) -> Result<std::net::SocketAddr, UdpError> {
        Ok(self.socket.local_addr()?)
    }

    /// Sends one datagram, enforcing the payload limit and the byte budget.
    ///
    /// # Errors
    ///
    /// Returns a size, quota or I/O error.
    pub async fn send_to(
        &self,
        payload: &[u8],
        target: std::net::SocketAddr,
    ) -> Result<usize, UdpError> {
        self.check_size(payload.len())?;
        self.charge(payload.len() as u64)?;
        Ok(self.socket.send_to(payload, target).await?)
    }

    /// Receives one datagram, enforcing the byte budget on what arrives.
    ///
    /// # Errors
    ///
    /// Returns a quota or I/O error; the buffer is the caller's.
    pub async fn recv_from(
        &self,
        buffer: &mut [u8],
    ) -> Result<(usize, std::net::SocketAddr), UdpError> {
        if buffer.len() > self.max_datagram {
            return Err(UdpError::DatagramTooLarge {
                size: buffer.len(),
                maximum: self.max_datagram,
            });
        }
        let (size, source) = self.socket.recv_from(buffer).await?;
        if size > self.max_datagram {
            return Err(UdpError::DatagramTooLarge {
                size,
                maximum: self.max_datagram,
            });
        }
        self.charge(size as u64)?;
        Ok((size, source))
    }

    fn check_size(&self, size: usize) -> Result<(), UdpError> {
        if size > self.max_datagram {
            return Err(UdpError::DatagramTooLarge {
                size,
                maximum: self.max_datagram,
            });
        }
        Ok(())
    }

    fn charge(&self, bytes: u64) -> Result<(), UdpError> {
        let previous = self.used_bytes.fetch_add(bytes, Ordering::Relaxed);
        let used = previous + bytes;
        if used > self.byte_budget {
            return Err(UdpError::QuotaExceeded {
                used,
                budget: self.byte_budget,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_is_denied_by_default() {
        let store = PermissionStore::new();
        assert!(store.is_empty());
        assert!(!store.is_granted("https://game.test", "cs-like"));
    }

    #[test]
    fn grant_and_revoke_are_per_origin_and_application() {
        let store = PermissionStore::new();
        store.grant("https://game.test", "cs-like");
        assert!(store.is_granted("https://game.test", "cs-like"));
        assert!(!store.is_granted("https://other.test", "cs-like"));
        assert!(!store.is_granted("https://game.test", "other-game"));
        store.revoke("https://game.test", "cs-like");
        assert!(store.is_empty());
    }

    #[tokio::test]
    async fn bind_without_grant_is_refused() {
        let store = PermissionStore::new();
        let error = UdpSocket::bind(&store, "https://game.test", "cs-like", "127.0.0.1:0")
            .await
            .unwrap_err();
        assert!(matches!(error, UdpError::PermissionDenied { .. }));
    }

    #[tokio::test]
    async fn granted_sockets_exchange_a_datagram() {
        let store = PermissionStore::new();
        store.grant("https://game.test", "cs-like");
        let server = UdpSocket::bind(&store, "https://game.test", "cs-like", "127.0.0.1:0")
            .await
            .unwrap();
        let client = UdpSocket::bind(&store, "https://game.test", "cs-like", "127.0.0.1:0")
            .await
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        client.send_to(b"ping", server_addr).await.unwrap();
        let mut buffer = [0u8; 16];
        let (size, _) = server.recv_from(&mut buffer).await.unwrap();
        assert_eq!(&buffer[..size], b"ping");
    }

    #[tokio::test]
    async fn oversized_datagrams_are_refused() {
        let store = PermissionStore::new();
        store.grant("https://game.test", "cs-like");
        let socket = UdpSocket::bind(&store, "https://game.test", "cs-like", "127.0.0.1:0")
            .await
            .unwrap()
            .with_max_datagram(8);
        let payload = [0u8; 9];
        let error = socket
            .send_to(&payload, socket.local_addr().unwrap())
            .await
            .unwrap_err();
        assert!(matches!(error, UdpError::DatagramTooLarge { .. }));
    }

    #[tokio::test]
    async fn the_byte_budget_is_enforced() {
        let store = PermissionStore::new();
        store.grant("https://game.test", "cs-like");
        let socket = UdpSocket::bind(&store, "https://game.test", "cs-like", "127.0.0.1:0")
            .await
            .unwrap()
            .with_byte_budget(6);
        let target = socket.local_addr().unwrap();
        socket.send_to(b"1234", target).await.unwrap();
        let error = socket.send_to(b"5678", target).await.unwrap_err();
        assert!(matches!(error, UdpError::QuotaExceeded { .. }));
    }
}
