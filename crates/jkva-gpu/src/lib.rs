//! Backend-neutral GPU command ABI for the WebGPU-class path (ADR-026).
//!
//! The renderer does not talk to Metal directly. It builds a [`CommandList`],
//! validates it against the [`DeviceCaps`] the GPU process reported, and sends
//! the encoded list over the GPU channel. A backend (Metal today, anything later)
//! executes the same list. Keeping the commands typed and validated here is what
//! makes "the GPU process cannot be asked to do something the device cannot do"
//! a property of the code rather than a convention.
//!
//! # Framing
//!
//! ```text
//! 0..2   protocol version, little endian u16
//! 2..6   payload length, little endian u32
//! 6..    bincode-encoded CommandList
//! ```
//!
//! This is the first, deliberately small surface: buffers, textures, one render
//! pass, draws. Pipelines, bind groups, samplers and compute arrive in later
//! increments; the version byte is what lets a GPU process reject a list it does
//! not understand instead of executing half of it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Version of the GPU command protocol.
pub const PROTOCOL_VERSION: u16 = 1;

/// Largest encoded command list accepted by [`CommandList::decode`].
pub const MAX_COMMAND_BYTES: u32 = 32 * 1024 * 1024;

/// Device limits negotiated with the GPU process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Largest single buffer, in bytes.
    pub max_buffer_bytes: u64,
    /// Largest texture edge, in pixels.
    pub max_texture_dimension: u32,
    /// Largest number of draws in one command list.
    pub max_draw_calls: u32,
}

impl Limits {
    /// Conservative defaults for a browser engine, not the hardware maximum.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            max_buffer_bytes: 256 * 1024 * 1024,
            max_texture_dimension: 8192,
            max_draw_calls: 65_536,
        }
    }
}

/// Optional device features as a bit set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Features(pub u32);

impl Features {
    /// Timestamp queries for per-pass GPU timing.
    pub const TIMESTAMP_QUERY: Self = Self(1 << 0);
    /// A unified memory architecture (zero-copy uploads may be possible).
    pub const UNIFIED_MEMORY: Self = Self(1 << 1);
    /// Texture compression formats.
    pub const TEXTURE_COMPRESSION: Self = Self(1 << 2);

    /// Returns whether every bit of `other` is present.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// What the GPU process reports after device creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCaps {
    /// Negotiated limits.
    pub limits: Limits,
    /// Available optional features.
    pub features: Features,
}

impl DeviceCaps {
    /// Conservative capabilities for tests and for a device without extensions.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            limits: Limits::conservative(),
            features: Features(0),
        }
    }
}

/// Stable identifier of a buffer or texture inside one command list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u32);

/// How a buffer will be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferUsage {
    /// Vertex data.
    Vertex,
    /// Index data.
    Index,
    /// Uniform data.
    Uniform,
    /// CPU-readable results.
    Readback,
}

/// Texture formats the ABI knows today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureFormat {
    /// 8 bits per channel, sRGB, the frame format.
    Rgba8Unorm,
    /// Depth attachment.
    Depth32Float,
}

/// One command in a list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Allocates a buffer.
    CreateBuffer {
        /// New resource.
        id: ResourceId,
        /// Size in bytes.
        size: u64,
        /// Intended usage.
        usage: BufferUsage,
    },
    /// Uploads bytes into an existing buffer.
    WriteBuffer {
        /// Target buffer.
        id: ResourceId,
        /// Byte offset.
        offset: u64,
        /// Payload; the offset plus this length must fit inside the buffer.
        data: Vec<u8>,
    },
    /// Allocates a texture.
    CreateTexture {
        /// New resource.
        id: ResourceId,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Pixel format.
        format: TextureFormat,
    },
    /// Starts a render pass into a texture.
    BeginPass {
        /// Render target texture.
        target: ResourceId,
        /// Clear colour in linear RGBA.
        clear: [f64; 4],
    },
    /// Draws `vertices` vertices from the bound vertex buffer.
    Draw {
        /// Pipeline identifier, resolved by the backend.
        pipeline: u32,
        /// First vertex.
        first: u32,
        /// Vertex count.
        vertices: u32,
        /// Instance count (1 for non-instanced).
        instances: u32,
    },
    /// Ends the open render pass.
    EndPass,
}

/// A validated-by-construction list of GPU commands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandList {
    /// Commands in execution order.
    pub commands: Vec<Command>,
}

/// Why a command list was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuError {
    /// A command references a resource that was never created.
    UnknownResource(ResourceId),
    /// A buffer is larger than the device limit.
    BufferTooLarge {
        /// Requested size.
        size: u64,
        /// Limit.
        maximum: u64,
    },
    /// A write leaves the buffer.
    WriteOutOfRange {
        /// Offset of the write.
        offset: u64,
        /// Length of the write.
        length: u64,
        /// Buffer size.
        size: u64,
    },
    /// A texture edge exceeds the device limit.
    TextureTooLarge {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
        /// Limit.
        maximum: u32,
    },
    /// A second render pass started while one was open.
    PassAlreadyOpen,
    /// A pass was ended while none was open.
    NoPass,
    /// The list ended with an open pass.
    PassNotEnded,
    /// More draws than the device allows.
    TooManyDraws {
        /// Requested count.
        count: u32,
        /// Limit.
        maximum: u32,
    },
    /// A resource id was created twice.
    ResourceAlreadyExists(ResourceId),
    /// The frame version does not match [`PROTOCOL_VERSION`].
    VersionMismatch {
        /// Version found in the frame.
        found: u16,
        /// Version this build speaks.
        expected: u16,
    },
    /// The frame is structurally invalid.
    Malformed(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownResource(id) => write!(f, "unknown resource {}", id.0),
            Self::BufferTooLarge { size, maximum } => {
                write!(f, "buffer of {size} bytes exceeds {maximum}")
            }
            Self::WriteOutOfRange {
                offset,
                length,
                size,
            } => write!(f, "write {offset}+{length} leaves a {size} byte buffer"),
            Self::TextureTooLarge {
                width,
                height,
                maximum,
            } => write!(f, "texture {width}x{height} exceeds {maximum}"),
            Self::PassAlreadyOpen => write!(f, "a render pass is already open"),
            Self::NoPass => write!(f, "no render pass is open"),
            Self::PassNotEnded => write!(f, "the list ends with an open render pass"),
            Self::TooManyDraws { count, maximum } => {
                write!(f, "{count} draws exceed the limit of {maximum}")
            }
            Self::ResourceAlreadyExists(id) => write!(f, "resource {} already exists", id.0),
            Self::VersionMismatch { found, expected } => {
                write!(f, "protocol version {found}, expected {expected}")
            }
            Self::Malformed(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl CommandList {
    /// Creates an empty list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Appends a command.
    pub fn push(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// Validates the list against the negotiated device capabilities.
    ///
    /// This is pure bookkeeping: it never allocates GPU memory and never talks to
    /// a device, so the renderer can reject a bad list before the GPU process sees
    /// it.
    ///
    /// # Errors
    ///
    /// Returns the first violation found, in command order.
    pub fn validate(&self, caps: &DeviceCaps) -> Result<(), GpuError> {
        let mut buffers: Vec<(ResourceId, u64)> = Vec::new();
        let mut textures: Vec<ResourceId> = Vec::new();
        let mut pass_open = false;
        let mut draws = 0u32;
        for command in &self.commands {
            match command {
                Command::CreateBuffer { id, size, .. } => {
                    if buffers.iter().any(|(existing, _)| existing == id) || textures.contains(id) {
                        return Err(GpuError::ResourceAlreadyExists(*id));
                    }
                    if *size > caps.limits.max_buffer_bytes {
                        return Err(GpuError::BufferTooLarge {
                            size: *size,
                            maximum: caps.limits.max_buffer_bytes,
                        });
                    }
                    buffers.push((*id, *size));
                }
                Command::WriteBuffer { id, offset, data } => {
                    let Some((_, size)) = buffers.iter().find(|(existing, _)| existing == id)
                    else {
                        return Err(GpuError::UnknownResource(*id));
                    };
                    let end = offset.saturating_add(data.len() as u64);
                    if end > *size {
                        return Err(GpuError::WriteOutOfRange {
                            offset: *offset,
                            length: data.len() as u64,
                            size: *size,
                        });
                    }
                }
                Command::CreateTexture {
                    id, width, height, ..
                } => {
                    if textures.contains(id) || buffers.iter().any(|(existing, _)| existing == id) {
                        return Err(GpuError::ResourceAlreadyExists(*id));
                    }
                    if *width > caps.limits.max_texture_dimension
                        || *height > caps.limits.max_texture_dimension
                    {
                        return Err(GpuError::TextureTooLarge {
                            width: *width,
                            height: *height,
                            maximum: caps.limits.max_texture_dimension,
                        });
                    }
                    textures.push(*id);
                }
                Command::BeginPass { target, .. } => {
                    if pass_open {
                        return Err(GpuError::PassAlreadyOpen);
                    }
                    if !textures.contains(target) {
                        return Err(GpuError::UnknownResource(*target));
                    }
                    pass_open = true;
                }
                Command::Draw { instances, .. } => {
                    if !pass_open {
                        return Err(GpuError::NoPass);
                    }
                    draws = draws.saturating_add(*instances);
                    if draws > caps.limits.max_draw_calls {
                        return Err(GpuError::TooManyDraws {
                            count: draws,
                            maximum: caps.limits.max_draw_calls,
                        });
                    }
                }
                Command::EndPass => {
                    if !pass_open {
                        return Err(GpuError::NoPass);
                    }
                    pass_open = false;
                }
            }
        }
        if pass_open {
            return Err(GpuError::PassNotEnded);
        }
        Ok(())
    }

    /// Encodes the list into a versioned frame.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload exceeds [`MAX_COMMAND_BYTES`].
    pub fn encode(&self) -> Result<Vec<u8>, GpuError> {
        let payload = bincode::serialize(self)
            .map_err(|error| GpuError::Malformed(format!("serialize failed: {error}")))?;
        let length = u32::try_from(payload.len())
            .map_err(|_| GpuError::Malformed("payload too large".into()))?;
        if length > MAX_COMMAND_BYTES {
            return Err(GpuError::Malformed(format!(
                "payload of {length} bytes exceeds {MAX_COMMAND_BYTES}"
            )));
        }
        let mut frame = Vec::with_capacity(6 + payload.len());
        frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        frame.extend_from_slice(&length.to_le_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    /// Decodes one frame produced by [`CommandList::encode`].
    ///
    /// # Errors
    ///
    /// Returns an error for a short header, a foreign version, an oversized or
    /// truncated payload, trailing bytes, or a payload that does not deserialize.
    pub fn decode(frame: &[u8]) -> Result<Self, GpuError> {
        if frame.len() < 6 {
            return Err(GpuError::Malformed("truncated header".into()));
        }
        let version = u16::from_le_bytes([frame[0], frame[1]]);
        if version != PROTOCOL_VERSION {
            return Err(GpuError::VersionMismatch {
                found: version,
                expected: PROTOCOL_VERSION,
            });
        }
        let length = u32::from_le_bytes([frame[2], frame[3], frame[4], frame[5]]);
        if length > MAX_COMMAND_BYTES {
            return Err(GpuError::Malformed(format!(
                "payload of {length} bytes exceeds {MAX_COMMAND_BYTES}"
            )));
        }
        let body = &frame[6..];
        if body.len() < length as usize {
            return Err(GpuError::Malformed("truncated payload".into()));
        }
        if body.len() > length as usize {
            return Err(GpuError::Malformed("trailing bytes".into()));
        }
        bincode::deserialize(body).map_err(|error| GpuError::Malformed(error.to_string()))
    }
}

impl Default for CommandList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> DeviceCaps {
        DeviceCaps::conservative()
    }

    fn valid_list() -> CommandList {
        let mut list = CommandList::new();
        list.push(Command::CreateBuffer {
            id: ResourceId(1),
            size: 64,
            usage: BufferUsage::Vertex,
        });
        list.push(Command::WriteBuffer {
            id: ResourceId(1),
            offset: 0,
            data: vec![0; 16],
        });
        list.push(Command::CreateTexture {
            id: ResourceId(2),
            width: 320,
            height: 200,
            format: TextureFormat::Rgba8Unorm,
        });
        list.push(Command::BeginPass {
            target: ResourceId(2),
            clear: [0.0, 0.0, 0.0, 1.0],
        });
        list.push(Command::Draw {
            pipeline: 0,
            first: 0,
            vertices: 3,
            instances: 1,
        });
        list.push(Command::EndPass);
        list
    }

    #[test]
    fn a_well_formed_list_validates_and_round_trips() {
        let list = valid_list();
        list.validate(&caps()).unwrap();
        let frame = list.encode().unwrap();
        assert_eq!(CommandList::decode(&frame).unwrap(), list);
    }

    #[test]
    fn unknown_resources_are_rejected() {
        let mut draw_without_pass = CommandList::new();
        draw_without_pass.push(Command::Draw {
            pipeline: 0,
            first: 0,
            vertices: 3,
            instances: 1,
        });
        assert_eq!(draw_without_pass.validate(&caps()), Err(GpuError::NoPass));

        let mut unknown_target = CommandList::new();
        unknown_target.push(Command::BeginPass {
            target: ResourceId(9),
            clear: [0.0; 4],
        });
        assert_eq!(
            unknown_target.validate(&caps()),
            Err(GpuError::UnknownResource(ResourceId(9)))
        );
    }

    #[test]
    fn limits_are_enforced() {
        let mut list = CommandList::new();
        list.push(Command::CreateBuffer {
            id: ResourceId(1),
            size: u64::MAX,
            usage: BufferUsage::Vertex,
        });
        assert!(matches!(
            list.validate(&caps()),
            Err(GpuError::BufferTooLarge { .. })
        ));
    }

    #[test]
    fn writes_must_fit_inside_the_buffer() {
        let mut list = CommandList::new();
        list.push(Command::CreateBuffer {
            id: ResourceId(1),
            size: 4,
            usage: BufferUsage::Vertex,
        });
        list.push(Command::WriteBuffer {
            id: ResourceId(1),
            offset: 2,
            data: vec![0; 4],
        });
        assert!(matches!(
            list.validate(&caps()),
            Err(GpuError::WriteOutOfRange { .. })
        ));
    }

    #[test]
    fn passes_must_be_balanced() {
        let mut list = valid_list();
        list.push(Command::EndPass);
        assert_eq!(list.validate(&caps()), Err(GpuError::NoPass));

        let mut open = CommandList::new();
        open.push(Command::CreateTexture {
            id: ResourceId(1),
            width: 1,
            height: 1,
            format: TextureFormat::Rgba8Unorm,
        });
        open.push(Command::BeginPass {
            target: ResourceId(1),
            clear: [0.0; 4],
        });
        assert_eq!(open.validate(&caps()), Err(GpuError::PassNotEnded));
    }

    #[test]
    fn a_foreign_version_is_rejected() {
        let mut frame = valid_list().encode().unwrap();
        frame[0] = 9;
        assert!(matches!(
            CommandList::decode(&frame),
            Err(GpuError::VersionMismatch { found: 9, .. })
        ));
    }

    #[test]
    fn truncated_and_trailing_frames_are_rejected() {
        let frame = valid_list().encode().unwrap();
        assert!(CommandList::decode(&frame[..frame.len() - 1]).is_err());
        let mut longer = frame.clone();
        longer.push(0);
        assert!(CommandList::decode(&longer).is_err());
    }
}
