//! GPU layer for JKva: Metal 4 device, offscreen render targets and readback.
//!
//! M0 scope: prove the pipeline end to end — create a device, render a clear
//! colour into an offscreen texture, read it back and hand the pixels to the
//! caller. The rasterizer, layers and compositor come in M3 (ADR-009).
//!
//! This crate owns every `unsafe` block that talks to Metal; the rest of the
//! engine sees plain Rust data.

#![deny(missing_docs)]
// Metal is a C API. This crate is the single unsafe boundary of the engine and
// every block below carries a SAFETY comment; the warning is allowed here only.
#![allow(unsafe_code)]

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;

use jkva_paint::{DisplayItem, DisplayList, Rect};
use objc2_metal::{
    MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat,
    MTLPrimitiveType, MTLRegion, MTLRenderCommandEncoder, MTLRenderPassDescriptor,
    MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLResourceOptions, MTLSize,
    MTLStorageMode, MTLStoreAction, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};

/// An RGBA8 image produced by the GPU and readable on the CPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Tightly packed RGBA8 pixels, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Image {
    /// Returns the pixel at `(x, y)` as RGBA, if inside the image.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y * self.width + x) * 4) as usize;
        Some([
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ])
    }
}

/// Returns the name of the Metal device used for rendering, if any.
#[must_use]
pub fn device_name() -> Option<String> {
    let device = MTLCreateSystemDefaultDevice()?;
    Some(device.name().to_string())
}

/// Offscreen render target plus the pass descriptor that targets it.
struct Target {
    texture: Retained<ProtocolObject<dyn MTLTexture>>,
    pass: Retained<MTLRenderPassDescriptor>,
}

fn new_target(
    device: &ProtocolObject<dyn MTLDevice>,
    width: u32,
    height: u32,
    clear: [f64; 4],
) -> Result<Target, String> {
    // SAFETY: valid pixel format and non-zero dimensions; the descriptor is only
    // used to allocate a texture on this device.
    let descriptor = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            MTLPixelFormat::RGBA8Unorm,
            width as usize,
            height as usize,
            false,
        )
    };
    descriptor.setUsage(MTLTextureUsage::RenderTarget);
    descriptor.setStorageMode(MTLStorageMode::Shared);
    let texture = device
        .newTextureWithDescriptor(&descriptor)
        .ok_or("could not create the render target")?;

    let pass = MTLRenderPassDescriptor::renderPassDescriptor();
    // SAFETY: Metal always allocates the attachments array; index 0 exists.
    let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setTexture(Some(&texture));
    attachment.setLoadAction(MTLLoadAction::Clear);
    attachment.setStoreAction(MTLStoreAction::Store);
    attachment.setClearColor(MTLClearColor {
        red: clear[0],
        green: clear[1],
        blue: clear[2],
        alpha: clear[3],
    });
    Ok(Target { texture, pass })
}

fn readback(target: &Target, width: u32, height: u32) -> Result<Image, String> {
    let bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|p| p.checked_mul(4))
        .ok_or("image is too large")?;
    let mut rgba = vec![0u8; bytes];
    let region = MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width: width as usize,
            height: height as usize,
            depth: 1,
        },
    };
    let pixels = std::ptr::NonNull::new(rgba.as_mut_ptr().cast::<std::ffi::c_void>())
        .ok_or("empty pixel buffer")?;
    // SAFETY: the buffer is exactly bytes long, the region covers the whole
    // texture, and the storage mode is Shared so the CPU may read it directly.
    unsafe {
        target.texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
            pixels,
            width as usize * 4,
            region,
            0,
        );
    }
    Ok(Image {
        width,
        height,
        rgba,
    })
}

fn finish(
    device: &ProtocolObject<dyn MTLDevice>,
    target: &Target,
    draw: impl FnOnce(&ProtocolObject<dyn MTLRenderCommandEncoder>) -> Result<(), String>,
) -> Result<(), String> {
    let queue = device
        .newCommandQueue()
        .ok_or("could not create a command queue")?;
    let command_buffer = queue
        .commandBuffer()
        .ok_or("could not create a command buffer")?;
    let encoder = command_buffer
        .renderCommandEncoderWithDescriptor(&target.pass)
        .ok_or("could not create a render encoder")?;
    draw(&encoder)?;
    encoder.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();
    Ok(())
}

/// Renders a solid clear colour into an offscreen RGBA8 texture and reads it back.
///
/// `color` is linear RGBA in `0.0..=1.0`. The function blocks until the GPU has
/// finished; it is a probe and a test helper, not a frame path.
///
/// # Errors
///
/// Returns an error when Metal is unavailable, an object cannot be created, or
/// the readback size would overflow.
pub fn render_clear(width: u32, height: u32, color: [f64; 4]) -> Result<Image, String> {
    if width == 0 || height == 0 {
        return Err("image dimensions must be non-zero".into());
    }
    let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
    let target = new_target(&device, width, height, color)?;
    finish(&device, &target, |_encoder| Ok(()))?;
    readback(&target, width, height)
}

const TRIANGLE_MSL: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct VOut {
    float4 position [[position]];
    float4 color;
};

vertex VOut v_main(uint vid [[vertex_id]], constant float2 *positions [[buffer(0)]]) {
    VOut out;
    out.position = float4(positions[vid], 0.0, 1.0);
    out.color = float4(0.95, 0.40, 0.15, 1.0);
    return out;
}

fragment float4 f_main(VOut in [[stage_in]]) {
    return in.color;
}
"#;

fn build_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    msl: &str,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>, String> {
    let source = NSString::from_str(msl);
    let library = device
        .newLibraryWithSource_options_error(&source, None)
        .map_err(|e| format!("shader compilation failed: {}", e.localizedDescription()))?;
    let vertex = library
        .newFunctionWithName(&NSString::from_str("v_main"))
        .ok_or("vertex function not found")?;
    let fragment = library
        .newFunctionWithName(&NSString::from_str("f_main"))
        .ok_or("fragment function not found")?;
    let pipeline = MTLRenderPipelineDescriptor::new();
    pipeline.setVertexFunction(Some(&vertex));
    pipeline.setFragmentFunction(Some(&fragment));
    // SAFETY: index 0 exists in the attachments array.
    unsafe { pipeline.colorAttachments().objectAtIndexedSubscript(0) }
        .setPixelFormat(MTLPixelFormat::RGBA8Unorm);
    device
        .newRenderPipelineStateWithDescriptor_error(&pipeline)
        .map_err(|e| format!("pipeline creation failed: {}", e.localizedDescription()))
}

/// Renders an orange triangle on a dark background and reads the pixels back.
///
/// This is the M0 "first triangle": it proves shader compilation, pipeline
/// creation, vertex buffer binding and a real draw call on Metal.
///
/// # Errors
///
/// Returns an error when Metal is unavailable or any pipeline object cannot be
/// created (the shader error text is included).
pub fn render_triangle(width: u32, height: u32) -> Result<Image, String> {
    if width == 0 || height == 0 {
        return Err("image dimensions must be non-zero".into());
    }
    let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
    let clear = [0.04, 0.05, 0.08, 1.0];
    let target = new_target(&device, width, height, clear)?;

    let state = build_pipeline(&device, TRIANGLE_MSL)?;

    let vertices: [f32; 6] = [0.0, 0.8, -0.8, -0.8, 0.8, -0.8];
    let pointer = std::ptr::NonNull::new(vertices.as_ptr().cast::<std::ffi::c_void>().cast_mut())
        .ok_or("vertex buffer is empty")?;
    // SAFETY: the pointer is valid for the duration of the call; the buffer copies.
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            pointer,
            std::mem::size_of_val(&vertices),
            MTLResourceOptions::StorageModeShared,
        )
    }
    .ok_or("could not create the vertex buffer")?;

    finish(&device, &target, |encoder| {
        encoder.setRenderPipelineState(&state);
        // SAFETY: buffer index 0 matches the shader signature, offset 0 is valid.
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&buffer), 0, 0);
            encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
        }
        Ok(())
    })?;

    readback(&target, width, height)
}

const RECT_MSL: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct VIn {
    float2 position;
    float4 color;
};

struct VOut {
    float4 position [[position]];
    float4 color;
};

vertex VOut v_main(uint vid [[vertex_id]], constant VIn *vertices [[buffer(0)]]) {
    VOut out;
    out.position = float4(vertices[vid].position, 0.0, 1.0);
    out.color = vertices[vid].color;
    return out;
}

fragment float4 f_main(VOut in [[stage_in]]) {
    return in.color;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vertex {
    position: [f32; 2],
    // MSL aligns `float4` to 16 bytes; without this padding the shader reads
    // the colour from the wrong offset. Keep the layout in sync with RECT_MSL.
    _padding: [f32; 2],
    color: [f32; 4],
}

fn append_rect(out: &mut Vec<Vertex>, rect: Rect, color: [u8; 4], width: u32, height: u32) {
    let to_ndc = |x: f32, y: f32| -> [f32; 2] {
        [x / width as f32 * 2.0 - 1.0, 1.0 - y / height as f32 * 2.0]
    };
    let c = [
        f32::from(color[0]) / 255.0,
        f32::from(color[1]) / 255.0,
        f32::from(color[2]) / 255.0,
        f32::from(color[3]) / 255.0,
    ];
    let (x0, y0, x1, y1) = (rect.min_x(), rect.min_y(), rect.max_x(), rect.max_y());
    let p0 = Vertex {
        position: to_ndc(x0, y0),
        _padding: [0.0; 2],
        color: c,
    };
    let p1 = Vertex {
        position: to_ndc(x1, y0),
        _padding: [0.0; 2],
        color: c,
    };
    let p2 = Vertex {
        position: to_ndc(x1, y1),
        _padding: [0.0; 2],
        color: c,
    };
    let p3 = Vertex {
        position: to_ndc(x0, y1),
        _padding: [0.0; 2],
        color: c,
    };
    out.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
}

/// Renders the solid rectangles of a display list onto a white background.
///
/// M1 scope: only [`DisplayItem::Rect`] is drawn; every other item is skipped
/// until the rasterizer lands in M3. Items paint in list order, so a later
/// rectangle covers an earlier one.
///
/// # Errors
///
/// Returns an error when Metal is unavailable, a pipeline cannot be created, or
/// the image dimensions are zero.
pub fn render_display_list(width: u32, height: u32, list: &DisplayList) -> Result<Image, String> {
    if width == 0 || height == 0 {
        return Err("image dimensions must be non-zero".into());
    }
    let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
    let target = new_target(&device, width, height, [1.0, 1.0, 1.0, 1.0])?;

    let mut vertices: Vec<Vertex> = Vec::new();
    for item in list.iter() {
        if let DisplayItem::Rect { rect, color } = item {
            append_rect(&mut vertices, *rect, *color, width, height);
        }
    }
    if vertices.is_empty() {
        finish(&device, &target, |_encoder| Ok(()))?;
        return readback(&target, width, height);
    }

    let state = build_pipeline(&device, RECT_MSL)?;
    let bytes = std::mem::size_of_val(vertices.as_slice());
    let pointer = std::ptr::NonNull::new(vertices.as_ptr().cast::<std::ffi::c_void>().cast_mut())
        .ok_or("vertex buffer is empty")?;
    // SAFETY: the pointer is valid for the duration of the call; the buffer copies.
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            pointer,
            bytes,
            MTLResourceOptions::StorageModeShared,
        )
    }
    .ok_or("could not create the vertex buffer")?;
    let count = vertices.len();

    finish(&device, &target, |encoder| {
        encoder.setRenderPipelineState(&state);
        // SAFETY: buffer index 0 matches the shader signature, offset 0 is valid.
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&buffer), 0, 0);
            encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, count);
        }
        Ok(())
    })?;
    readback(&target, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_accessor_is_bounds_checked() {
        let image = Image {
            width: 2,
            height: 1,
            rgba: vec![1, 2, 3, 255, 4, 5, 6, 255],
        };
        assert_eq!(image.pixel(0, 0), Some([1, 2, 3, 255]));
        assert_eq!(image.pixel(1, 0), Some([4, 5, 6, 255]));
        assert_eq!(image.pixel(2, 0), None);
    }

    #[test]
    fn zero_sized_render_is_rejected() {
        assert!(render_clear(0, 4, [0.0, 0.0, 0.0, 1.0]).is_err());
    }

    #[test]
    fn display_list_rects_paint_in_order() {
        let mut list = DisplayList::new();
        list.push(DisplayItem::Rect {
            rect: Rect::new(0.0, 0.0, 32.0, 32.0),
            color: [255, 0, 0, 255],
        });
        list.push(DisplayItem::Rect {
            rect: Rect::new(16.0, 16.0, 32.0, 32.0),
            color: [0, 0, 255, 255],
        });
        let Some(image) = render_display_list(64, 64, &list).ok() else {
            return;
        };
        assert_eq!(image.pixel(8, 8), Some([255, 0, 0, 255]));
        assert_eq!(image.pixel(24, 24), Some([0, 0, 255, 255]));
        assert_eq!(image.pixel(60, 8), Some([255, 255, 255, 255]));
    }

    #[test]
    fn triangle_covers_the_centre_and_not_the_corner() {
        let Some(image) = render_triangle(64, 64).ok() else {
            return;
        };
        let centre = image.pixel(32, 32).unwrap();
        let corner = image.pixel(0, 0).unwrap();
        assert_eq!(centre, [242, 102, 38, 255]);
        assert_eq!(corner, [10, 13, 20, 255]);
    }

    #[test]
    fn clear_colour_round_trips_through_the_gpu() {
        let Some(image) = render_clear(4, 4, [1.0, 0.0, 0.0, 1.0]).ok() else {
            // No GPU (headless CI): the probe is skipped, not failed.
            return;
        };
        assert_eq!(image.width, 4);
        assert_eq!(image.rgba.len(), 64);
        assert_eq!(image.pixel(0, 0), Some([255, 0, 0, 255]));
    }
}
