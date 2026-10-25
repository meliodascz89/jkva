//! macOS platform layer for JKva: Metal device discovery, thread QoS and signposts.
//!
//! This is the only crate allowed to depend on macOS frameworks directly (see ADR-005).
//! Everything above it talks to `jkva-core` types and the traits defined here.

#![deny(missing_docs)]

use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

/// Returns the name of the system default Metal device.
///
/// Returns `None` when Metal is unavailable (for example in a headless CI container
/// without a GPU). Callers must treat that as "software path only".
pub fn metal_device_name() -> Option<String> {
    let device = MTLCreateSystemDefaultDevice()?;
    Some(device.name().to_string())
}

/// Returns the Metal feature families supported by the default device, if any.
///
/// This is a probe used by M0 to prove that the binding layer compiles and that the
/// machine exposes the API we target. It is not used by the renderer yet.
pub fn metal_supports_apple_family() -> bool {
    let Some(device) = MTLCreateSystemDefaultDevice() else {
        return false;
    };
    device.supportsFamily(objc2_metal::MTLGPUFamily::Apple9)
}
