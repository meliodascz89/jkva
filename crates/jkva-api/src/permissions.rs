//! Explicit, per-origin and per-application capability grants (ADR-026).
//!
//! A game may request a capability; the user decides once or for the session.
//! The gate stores only decisions, never implicit defaults: an unknown request is
//! denied. Session grants are cleared when the application closes, and a
//! one-shot grant is consumed by the first successful check.

use std::collections::HashSet;

/// A capability an application can request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PermissionKind {
    /// Open UDP sockets (games, ADR-026).
    Udp,
}

impl PermissionKind {
    /// Human-readable capability name for prompts and logs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Udp => "network access (UDP)",
        }
    }
}

/// A normalised web origin.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Origin(String);

/// Why an origin string was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginError {
    /// No `scheme://` part.
    MissingScheme,
    /// Only `http` and `https` are supported.
    UnsupportedScheme,
    /// The host is empty.
    MissingHost,
    /// Plain HTTP is allowed only for loopback development origins.
    InsecureOrigin,
}

impl std::fmt::Display for OriginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingScheme => write!(f, "origin has no scheme"),
            Self::UnsupportedScheme => write!(f, "origin scheme must be http or https"),
            Self::MissingHost => write!(f, "origin has no host"),
            Self::InsecureOrigin => {
                write!(f, "plain http is allowed only for localhost development")
            }
        }
    }
}

impl std::error::Error for OriginError {}

impl Origin {
    /// Parses and normalises an origin.
    ///
    /// The path, query and fragment are dropped: a permission belongs to
    /// `https://game.test`, not to one page inside it. `http` is accepted only
    /// for `localhost`, `127.0.0.1` and `[::1]`, which keeps local development
    /// possible without weakening the rule for the web.
    ///
    /// # Errors
    ///
    /// Returns [`OriginError`] for a missing or unsupported scheme, an empty host,
    /// or insecure remote HTTP.
    pub fn parse(value: &str) -> Result<Self, OriginError> {
        let (scheme, rest) = value.split_once("://").ok_or(OriginError::MissingScheme)?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(OriginError::UnsupportedScheme);
        }
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if authority.is_empty() {
            return Err(OriginError::MissingHost);
        }
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let hostname = host
            .rsplit_once(':')
            .and_then(|(name, port)| port.parse::<u16>().ok().map(|_| name))
            .unwrap_or(host);
        let local = matches!(hostname, "localhost" | "127.0.0.1" | "[::1]" | "::1");
        if scheme == "http" && !local {
            return Err(OriginError::InsecureOrigin);
        }
        Ok(Self(format!("{scheme}://{host}")))
    }

    /// The normalised origin string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One application asking for one capability at one origin.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PermissionRequest {
    /// Requesting origin.
    pub origin: Origin,
    /// Application identity (a stable id, not a display name).
    pub application: String,
    /// Requested capability.
    pub kind: PermissionKind,
}

impl PermissionRequest {
    /// Creates a request; the origin must already be parsed.
    #[must_use]
    pub fn new(origin: Origin, application: impl Into<String>, kind: PermissionKind) -> Self {
        Self {
            origin,
            application: application.into(),
            kind,
        }
    }
}

/// How long a grant lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grant {
    /// One successful check consumes it.
    Once,
    /// Lasts until the application closes or the user revokes it.
    Session,
}

/// Policy state for capability requests.
///
/// The gate is intentionally not a singleton: the browser owns one instance and
/// the network process is told only the resulting decision.
#[derive(Debug, Default)]
pub struct PermissionGate {
    session: HashSet<PermissionRequest>,
    once: HashSet<PermissionRequest>,
    denied: HashSet<PermissionRequest>,
}

impl PermissionGate {
    /// Creates an empty gate: every request is denied until granted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the user's grant for a request.
    pub fn grant(&mut self, request: PermissionRequest, grant: Grant) {
        self.denied.remove(&request);
        match grant {
            Grant::Once => {
                self.session.remove(&request);
                self.once.insert(request);
            }
            Grant::Session => {
                self.once.remove(&request);
                self.session.insert(request);
            }
        }
    }

    /// Records the user's refusal for the session.
    pub fn deny(&mut self, request: PermissionRequest) {
        self.session.remove(&request);
        self.once.remove(&request);
        self.denied.insert(request);
    }

    /// Checks a request and consumes a one-shot grant when present.
    ///
    /// Returns `true` for a session grant and for the first check of a one-shot
    /// grant; a denied or unknown request stays `false`.
    pub fn check(&mut self, request: &PermissionRequest) -> bool {
        if self.denied.contains(request) {
            return false;
        }
        if self.session.contains(request) {
            return true;
        }
        self.once.remove(request)
    }

    /// Returns whether a session grant exists without consuming anything.
    #[must_use]
    pub fn is_granted(&self, request: &PermissionRequest) -> bool {
        !self.denied.contains(request) && self.session.contains(request)
    }

    /// Removes every grant and denial for one application.
    ///
    /// Called when the application closes so nothing outlives the session.
    pub fn clear_application(&mut self, origin: &Origin, application: &str) {
        self.session
            .retain(|request| &request.origin != origin || request.application != application);
        self.once
            .retain(|request| &request.origin != origin || request.application != application);
        self.denied
            .retain(|request| &request.origin != origin || request.application != application);
    }

    /// Number of active session grants.
    #[must_use]
    pub fn session_grants(&self) -> usize {
        self.session.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(origin: &str, app: &str) -> PermissionRequest {
        PermissionRequest::new(Origin::parse(origin).unwrap(), app, PermissionKind::Udp)
    }

    #[test]
    fn origins_are_normalised_and_paths_dropped() {
        let origin = Origin::parse("https://Game.Test:443/path?q=1").unwrap();
        assert_eq!(origin.as_str(), "https://Game.Test:443");
    }

    #[test]
    fn insecure_remote_http_is_rejected() {
        assert_eq!(
            Origin::parse("http://game.test"),
            Err(OriginError::InsecureOrigin)
        );
        assert!(Origin::parse("http://localhost:8080").is_ok());
        assert!(Origin::parse("http://127.0.0.1").is_ok());
        assert!(Origin::parse("ftp://game.test").is_err());
        assert!(Origin::parse("game.test").is_err());
    }

    #[test]
    fn unknown_requests_are_denied() {
        let mut gate = PermissionGate::new();
        assert!(!gate.check(&request("https://game.test", "cs-like")));
    }

    #[test]
    fn a_one_shot_grant_is_consumed_once() {
        let mut gate = PermissionGate::new();
        let req = request("https://game.test", "cs-like");
        gate.grant(req.clone(), Grant::Once);
        assert!(gate.check(&req));
        assert!(!gate.check(&req));
    }

    #[test]
    fn a_session_grant_survives_until_cleared() {
        let mut gate = PermissionGate::new();
        let req = request("https://game.test", "cs-like");
        gate.grant(req.clone(), Grant::Session);
        assert!(gate.check(&req));
        assert!(gate.check(&req));
        assert_eq!(gate.session_grants(), 1);
        let origin = Origin::parse("https://game.test").unwrap();
        gate.clear_application(&origin, "cs-like");
        assert!(!gate.check(&req));
    }

    #[test]
    fn grants_are_per_origin_and_application() {
        let mut gate = PermissionGate::new();
        let req = request("https://game.test", "cs-like");
        gate.grant(req.clone(), Grant::Session);
        assert!(!gate.check(&request("https://other.test", "cs-like")));
        assert!(!gate.check(&request("https://game.test", "other-game")));
        assert!(gate.check(&req));
    }

    #[test]
    fn denial_overrides_a_previous_grant() {
        let mut gate = PermissionGate::new();
        let req = request("https://game.test", "cs-like");
        gate.grant(req.clone(), Grant::Session);
        gate.deny(req.clone());
        assert!(!gate.check(&req));
        assert_eq!(gate.session_grants(), 0);
    }
}
