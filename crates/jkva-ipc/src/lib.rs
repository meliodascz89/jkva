//! The wire contract between the JKva processes (ADR-006).
//!
//! The browser, renderer, GPU and network processes talk over a versioned,
//! length-prefixed binary protocol. This crate owns the message types and the
//! framing; it deliberately knows nothing about sockets, shared memory or
//! scheduling. That keeps the contract testable in one place and lets the
//! transport (Unix socket, Mach port, shared memory) change without touching a
//! single message.
//!
//! # Frame format
//!
//! ```text
//! 0..2   protocol version, little endian u16
//! 2..6   payload length, little endian u32
//! 6..    bincode payload
//! ```
//!
//! A decoder rejects a different version, a length above [`MAX_FRAME_BYTES`],
//! a truncated frame and trailing bytes. Those checks are the reason this crate
//! exists: a renderer must never parse a frame it cannot trust.
//!
//! # Compatibility
//!
//! Bump [`PROTOCOL_VERSION`] for any change that is not purely additive to the
//! enums, and keep the old decoder around for one release so a browser update
//! can outlive a renderer restart.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Version of the IPC protocol.
pub const PROTOCOL_VERSION: u16 = 1;

/// Largest payload a frame may carry, in bytes.
pub const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;

/// Identifies one frame tree node across processes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameId(pub u64);

/// A rectangle in CSS pixels, in the frame's coordinate space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DamageRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

/// Input delivered by the browser process to a renderer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// A pointer button went down.
    PointerDown {
        /// X in CSS pixels.
        x: f32,
        /// Y in CSS pixels.
        y: f32,
        /// Button index, 0 is the primary button.
        button: u8,
    },
    /// A pointer button went up.
    PointerUp {
        /// X in CSS pixels.
        x: f32,
        /// Y in CSS pixels.
        y: f32,
        /// Button index.
        button: u8,
    },
    /// The wheel moved by `dy` CSS pixels.
    Wheel {
        /// Vertical delta, positive scrolls down.
        dy: f32,
    },
    /// A key was pressed; `text` is empty for non-printing keys.
    Key {
        /// DOM key value.
        key: String,
        /// Printable text, if any.
        text: String,
    },
}

/// Messages the browser process sends to a renderer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BrowserToRenderer {
    /// Create a frame and load `url` at the given viewport.
    CreateFrame {
        /// The new frame.
        frame: FrameId,
        /// Initial URL.
        url: String,
        /// Viewport width in CSS pixels.
        width: u32,
        /// Viewport height in CSS pixels.
        height: u32,
    },
    /// Navigate an existing frame.
    Navigate {
        /// Target frame.
        frame: FrameId,
        /// Destination URL.
        url: String,
    },
    /// Resize a frame's viewport.
    Resize {
        /// Target frame.
        frame: FrameId,
        /// New width in CSS pixels.
        width: u32,
        /// New height in CSS pixels.
        height: u32,
    },
    /// Deliver input to a frame.
    Input {
        /// Target frame.
        frame: FrameId,
        /// The event.
        event: InputEvent,
    },
    /// Drop the frame and free its resources.
    Close {
        /// Target frame.
        frame: FrameId,
    },
}

/// Messages a renderer sends back to the browser process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RendererToBrowser {
    /// The document is parsed and styled; first paint follows.
    Ready {
        /// The frame.
        frame: FrameId,
    },
    /// A frame was presented; `damage` is what changed.
    Presented {
        /// The frame.
        frame: FrameId,
        /// Damaged rectangles, empty for a full repaint.
        damage: Vec<DamageRect>,
    },
    /// The document title changed.
    Title {
        /// The frame.
        frame: FrameId,
        /// New title.
        title: String,
    },
    /// A console message was logged.
    Console {
        /// The frame.
        frame: FrameId,
        /// Message text.
        text: String,
    },
    /// The renderer process failed.
    Crashed {
        /// The frame.
        frame: FrameId,
        /// Human-readable reason, never a stack trace.
        reason: String,
    },
}

/// Every message on the wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    /// Browser → renderer.
    Browser(BrowserToRenderer),
    /// Renderer → browser.
    Renderer(RendererToBrowser),
}

/// Errors a decoder can return.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpcError {
    /// The frame is shorter than the header.
    TruncatedHeader,
    /// The frame version does not match [`PROTOCOL_VERSION`].
    VersionMismatch {
        /// Version in the frame.
        found: u16,
        /// Version this build speaks.
        expected: u16,
    },
    /// The declared payload is larger than [`MAX_FRAME_BYTES`].
    FrameTooLarge {
        /// Declared length.
        declared: u32,
        /// Configured maximum.
        maximum: u32,
    },
    /// The payload ends before the declared length.
    TruncatedPayload {
        /// Declared length.
        declared: u32,
        /// Bytes actually present.
        available: usize,
    },
    /// Bytes follow the declared payload.
    TrailingBytes {
        /// Extra bytes.
        extra: usize,
    },
    /// The payload is not a valid message.
    Malformed(String),
}

/// Encodes a message into a length-prefixed frame.
///
/// # Errors
///
/// Returns an error when the payload exceeds [`MAX_FRAME_BYTES`].
pub fn encode(message: &Message) -> Result<Vec<u8>, IpcError> {
    let payload = bincode::serialize(message)
        .map_err(|e| IpcError::Malformed(format!("serialize failed: {e}")))?;
    let length = u32::try_from(payload.len()).map_err(|_| IpcError::FrameTooLarge {
        declared: u32::MAX,
        maximum: MAX_FRAME_BYTES,
    })?;
    if length > MAX_FRAME_BYTES {
        return Err(IpcError::FrameTooLarge {
            declared: length,
            maximum: MAX_FRAME_BYTES,
        });
    }
    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Decodes exactly one frame.
///
/// # Errors
///
/// Returns an error for a short header, a foreign version, an oversized or
/// truncated payload, trailing bytes, or a payload that does not deserialize.
pub fn decode(frame: &[u8]) -> Result<Message, IpcError> {
    if frame.len() < 6 {
        return Err(IpcError::TruncatedHeader);
    }
    let version = u16::from_le_bytes([frame[0], frame[1]]);
    if version != PROTOCOL_VERSION {
        return Err(IpcError::VersionMismatch {
            found: version,
            expected: PROTOCOL_VERSION,
        });
    }
    let declared = u32::from_le_bytes([frame[2], frame[3], frame[4], frame[5]]);
    if declared > MAX_FRAME_BYTES {
        return Err(IpcError::FrameTooLarge {
            declared,
            maximum: MAX_FRAME_BYTES,
        });
    }
    let body = &frame[6..];
    let declared_usize = declared as usize;
    if body.len() < declared_usize {
        return Err(IpcError::TruncatedPayload {
            declared,
            available: body.len(),
        });
    }
    if body.len() > declared_usize {
        return Err(IpcError::TrailingBytes {
            extra: body.len() - declared_usize,
        });
    }
    bincode::deserialize(&body[..declared_usize]).map_err(|e| IpcError::Malformed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Message {
        Message::Browser(BrowserToRenderer::CreateFrame {
            frame: FrameId(7),
            url: "https://example.test/".into(),
            width: 800,
            height: 600,
        })
    }

    #[test]
    fn every_message_round_trips() {
        let messages = [
            sample(),
            Message::Browser(BrowserToRenderer::Navigate {
                frame: FrameId(1),
                url: "about:blank".into(),
            }),
            Message::Browser(BrowserToRenderer::Input {
                frame: FrameId(1),
                event: InputEvent::PointerDown {
                    x: 10.0,
                    y: 20.0,
                    button: 0,
                },
            }),
            Message::Renderer(RendererToBrowser::Presented {
                frame: FrameId(1),
                damage: vec![DamageRect {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                }],
            }),
            Message::Renderer(RendererToBrowser::Crashed {
                frame: FrameId(2),
                reason: "out of memory".into(),
            }),
        ];
        for message in messages {
            let frame = encode(&message).unwrap();
            assert_eq!(decode(&frame).unwrap(), message);
        }
    }

    #[test]
    fn short_header_is_rejected() {
        assert_eq!(decode(&[1, 0, 0]), Err(IpcError::TruncatedHeader));
    }

    #[test]
    fn foreign_version_is_rejected() {
        let mut frame = encode(&sample()).unwrap();
        frame[0] = 9;
        assert!(matches!(
            decode(&frame),
            Err(IpcError::VersionMismatch { found: 9, .. })
        ));
    }

    #[test]
    fn truncated_and_trailing_frames_are_rejected() {
        let frame = encode(&sample()).unwrap();
        assert!(matches!(
            decode(&frame[..frame.len() - 1]),
            Err(IpcError::TruncatedPayload { .. })
        ));
        let mut longer = frame.clone();
        longer.push(0);
        assert!(matches!(
            decode(&longer),
            Err(IpcError::TrailingBytes { extra: 1 })
        ));
    }

    #[test]
    fn oversized_frame_is_rejected_before_allocating() {
        let mut frame = vec![0u8; 6];
        frame[..2].copy_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        frame[2..6].copy_from_slice(&(MAX_FRAME_BYTES + 1).to_le_bytes());
        assert!(matches!(
            decode(&frame),
            Err(IpcError::FrameTooLarge { .. })
        ));
    }
}
