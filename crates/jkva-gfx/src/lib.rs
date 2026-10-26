//! GPU layer for JKva: Metal 4 device, offscreen render targets and readback.
//!
//! M0 scope: prove the pipeline end to end — create a device, render a clear
//! colour into an offscreen texture, read it back and hand the pixels to the
//! caller. The rasterizer, layers and compositor come in M3 (ADR-009).
//!
//! The capability report ([`capabilities`]) answers what the device can do
//! before anything renders: memory limits, Apple GPU families and whether
//! timestamp counters exist. Gaming performance is a 0.1 goal (ADR-026), so a
//! number produced without the hardware identity is not a measurement.
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

pub use jkva_paint::Rect;

mod executor;
pub use executor::GpuExecutor;
use jkva_paint::{DisplayItem, DisplayList};
use objc2_metal::{
    MTLBlendFactor, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLCounterSamplingPoint, MTLCounterSet, MTLCreateSystemDefaultDevice, MTLDevice, MTLGPUFamily,
    MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion,
    MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState, MTLResourceOptions, MTLSamplerAddressMode, MTLSamplerDescriptor,
    MTLSamplerMinMagFilter, MTLSamplerState, MTLSize, MTLStorageMode, MTLStoreAction, MTLTexture,
    MTLTextureDescriptor, MTLTextureUsage,
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

/// Apple GPU family membership of the default Metal device.
///
/// A game engine uses these queries to decide which shader features and
/// resource limits it may rely on. On Apple Silicon the baseline is `apple7`
/// (M1-class); each newer family adds features, so a renderer that only needs
/// `apple7` runs on every supported Mac, while an optional path can require
/// `apple9` for newer hardware. The exact chip-to-family mapping is Apple's;
/// this struct only records what the device answered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuFamilies {
    /// `MTLGPUFamilyApple7` is supported.
    pub apple7: bool,
    /// `MTLGPUFamilyApple8` is supported.
    pub apple8: bool,
    /// `MTLGPUFamilyApple9` is supported.
    pub apple9: bool,
    /// `MTLGPUFamilyMetal4` is supported, i.e. the Metal 4 API generation.
    pub metal4: bool,
}

/// Snapshot of what the default Metal device can do.
///
/// Read it once at startup (for example while building the GPU process) and
/// treat it as immutable for the lifetime of the process: devices do not gain
/// memory or lose families, and a stale report is cheaper than a query per
/// frame. A game engine needs these numbers to size its budgets and to choose
/// feature paths instead of finding out through an out-of-memory error mid-game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuCapabilities {
    /// Marketing name reported by Metal, for example `Apple M4`.
    ///
    /// Benchmark reports carry it because a frame time without the hardware is
    /// not a measurement (`docs/src/en/gaming.md`).
    pub device_name: String,
    /// Largest single `MTLBuffer` in bytes (`MTLDevice.maxBufferLength`).
    ///
    /// A level mesh, a texture atlas or a vertex batch must fit into one
    /// buffer; a streaming game has to split an upload into chunks below this
    /// limit or fail at runtime.
    pub max_buffer_length: u64,
    /// Bytes of GPU memory macOS recommends keeping resident
    /// (`MTLDevice.recommendedMaxWorkingSetSize`).
    ///
    /// This is the practical GPU memory budget for a game: staying under it
    /// avoids paging and the associated frame-time spikes. It is a
    /// recommendation, not a hard limit, and it can be smaller than the
    /// physical memory.
    pub recommended_max_working_set_size: u64,
    /// Which Apple GPU families the device answers `true` to.
    pub families: GpuFamilies,
    /// `true` when the GPU shares physical memory with the CPU (UMA).
    ///
    /// UMA is what makes JKva's zero-copy upload path possible (ADR-009); a
    /// discrete GPU would require explicit staging copies.
    pub supports_unified_memory: bool,
    /// `true` when the device exposes a timestamp counter set that can be
    /// sampled at stage boundaries.
    ///
    /// Games need real per-pass GPU timings, not guesses. When this is `false`
    /// the engine can still measure command-buffer completion wall time, but
    /// not the duration of an individual pass; see the frame-time benchmark
    /// limitations. The flag is true only when Metal reports both the
    /// `timestamp` counter set and `MTLCounterSamplingPointAtStageBoundary`
    /// support.
    pub supports_timestamp_counters: bool,
}

/// Reports the capabilities of the default Metal device.
///
/// # Errors
///
/// Returns an error when Metal is unavailable, for example in a headless
/// container without a GPU. Callers must fall back to a non-GPU path instead of
/// assuming a device exists.
pub fn capabilities() -> Result<GpuCapabilities, String> {
    let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
    Ok(GpuCapabilities {
        device_name: device.name().to_string(),
        max_buffer_length: device.maxBufferLength() as u64,
        recommended_max_working_set_size: device.recommendedMaxWorkingSetSize(),
        families: GpuFamilies {
            apple7: device.supportsFamily(MTLGPUFamily::Apple7),
            apple8: device.supportsFamily(MTLGPUFamily::Apple8),
            apple9: device.supportsFamily(MTLGPUFamily::Apple9),
            metal4: device.supportsFamily(MTLGPUFamily::Metal4),
        },
        supports_unified_memory: device.hasUnifiedMemory(),
        supports_timestamp_counters: has_counter_set(&device, "timestamp")
            && device.supportsCounterSampling(MTLCounterSamplingPoint::AtStageBoundary),
    })
}

/// Returns `true` when the device advertises a counter set whose name contains
/// `needle` (Metal names the timestamp set `timestamp`).
fn has_counter_set(device: &ProtocolObject<dyn MTLDevice>, needle: &str) -> bool {
    device.counterSets().is_some_and(|sets| {
        sets.iter()
            .any(|set| set.name().to_string().to_ascii_lowercase().contains(needle))
    })
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
    encode_and_wait(&queue, target, draw)
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

/// CPU-side RGBA8 image uploaded to the GPU as a texture.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureData {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Tightly packed RGBA8 pixels, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// One drawable item of a [`Scene`], in paint order.
#[derive(Clone, Debug, PartialEq)]
pub enum SceneItem {
    /// An image whose pixels the renderer already owns under `key`.
    ///
    /// The caller uploads it once through [`SceneRenderer::ensure_texture`]; every
    /// later frame refers to it by key instead of copying the pixels again.
    CachedImage {
        /// Destination rectangle in CSS pixels.
        rect: Rect,
        /// Key passed to [`SceneRenderer::ensure_texture`].
        key: u64,
        /// Straight sRGB RGBA colour multiplied with the texture.
        tint: [u8; 4],
    },
    /// A solid rectangle.
    Rect {
        /// Position and size in scene pixels.
        rect: Rect,
        /// Straight-alpha sRGB colour.
        color: [u8; 4],
    },
    /// A textured rectangle, used for glyph bitmaps and images.
    Image {
        /// Position and size in scene pixels.
        rect: Rect,
        /// Source pixels; must be `width * height * 4` bytes.
        texture: TextureData,
        /// Multiplied with the sampled colour; white leaves the texture unchanged.
        tint: [u8; 4],
    },
}

/// An ordered list of items, later items painting over earlier ones.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// Items in paint order.
    pub items: Vec<SceneItem>,
}

impl Scene {
    /// Creates an empty scene.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Appends an item on top of the current scene.
    pub fn push(&mut self, item: SceneItem) {
        self.items.push(item);
    }

    /// Number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` when the scene has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

const SCENE_MSL: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct VIn {
    float2 position;
    float2 uv;
    float4 color;
};

struct VOut {
    float4 position [[position]];
    float2 uv;
    float4 color;
};

vertex VOut v_main(uint vid [[vertex_id]], constant VIn *vertices [[buffer(0)]]) {
    VOut out;
    out.position = float4(vertices[vid].position, 0.0, 1.0);
    out.uv = vertices[vid].uv;
    out.color = vertices[vid].color;
    return out;
}

fragment float4 f_main(VOut in [[stage_in]],
                       texture2d<float> tex [[texture(0)]],
                       sampler samp [[sampler(0)]]) {
    return in.color * tex.sample(samp, in.uv);
}
"#;

/// One draw: the texture to bind, the first vertex and the vertex count.
type SceneDraw = (Retained<ProtocolObject<dyn MTLTexture>>, usize, usize);

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct SceneVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

fn make_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    data: &TextureData,
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>, String> {
    if data.width == 0 || data.height == 0 {
        return Err("texture dimensions must be non-zero".into());
    }
    let expected = data.width as usize * data.height as usize * 4;
    if data.rgba.len() != expected {
        return Err(format!(
            "texture has {} bytes, expected {expected}",
            data.rgba.len()
        ));
    }
    // SAFETY: valid pixel format and non-zero dimensions.
    let descriptor = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            MTLPixelFormat::RGBA8Unorm,
            data.width as usize,
            data.height as usize,
            false,
        )
    };
    descriptor.setUsage(MTLTextureUsage::ShaderRead);
    descriptor.setStorageMode(MTLStorageMode::Shared);
    let texture = device
        .newTextureWithDescriptor(&descriptor)
        .ok_or("could not create a texture")?;
    let pointer = std::ptr::NonNull::new(data.rgba.as_ptr().cast::<std::ffi::c_void>().cast_mut())
        .ok_or("texture data is empty")?;
    let region = MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width: data.width as usize,
            height: data.height as usize,
            depth: 1,
        },
    };
    // SAFETY: the buffer holds exactly width*height*4 bytes and the region covers
    // the whole texture; storage mode is Shared so a CPU upload is allowed.
    unsafe {
        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            region,
            0,
            pointer,
            data.width as usize * 4,
        );
    }
    Ok(texture)
}

/// Renders a scene of rectangles and textured quads onto a white background.
///
/// Text is drawn by uploading an alpha coverage bitmap as a [`TextureData`] with
/// white RGB and using the tint as the text colour; the pipeline blends with
/// source alpha, so coverage becomes anti-aliasing.
///
/// # Errors
///
/// Returns an error when Metal is unavailable, a texture is malformed, or a
/// pipeline object cannot be created.
/// Offscreen renderer with a persistent Metal device, queue, pipeline, sampler and
/// white texture.
///
/// [`render_scene`] creates all of that per call, which is fine for a screenshot but
/// useless for a game frame. A renderer keeps the objects alive, so a caller can
/// submit frames back to back; [`SceneRenderer::submit`] deliberately skips the CPU
/// readback, which a presented frame does not need.
pub struct SceneRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    white: Retained<ProtocolObject<dyn MTLTexture>>,
    textures: std::collections::HashMap<u64, Retained<ProtocolObject<dyn MTLTexture>>>,
    frame_target: Option<(u32, u32, Target)>,
    drawn_items: usize,
}

impl SceneRenderer {
    /// Creates the renderer, or returns why Metal is unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error when the device, queue, pipeline, sampler or the 1×1 white
    /// texture cannot be created.
    pub fn new() -> Result<Self, String> {
        let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
        let queue = device
            .newCommandQueue()
            .ok_or("could not create a command queue")?;
        let state = build_scene_pipeline(&device)?;
        let sampler = {
            let descriptor = MTLSamplerDescriptor::new();
            descriptor.setMinFilter(MTLSamplerMinMagFilter::Linear);
            descriptor.setMagFilter(MTLSamplerMinMagFilter::Linear);
            descriptor.setSAddressMode(MTLSamplerAddressMode::ClampToEdge);
            descriptor.setTAddressMode(MTLSamplerAddressMode::ClampToEdge);
            device
                .newSamplerStateWithDescriptor(&descriptor)
                .ok_or("could not create a sampler")?
        };
        let white = make_texture(
            &device,
            &TextureData {
                width: 1,
                height: 1,
                rgba: vec![255, 255, 255, 255],
            },
        )?;
        Ok(Self {
            device,
            queue,
            state,
            sampler,
            white,
            textures: std::collections::HashMap::new(),
            frame_target: None,
            drawn_items: 0,
        })
    }

    /// Uploads a texture once and keeps it for later [`SceneItem::CachedImage`]
    /// references.
    ///
    /// Uploading the same key again replaces the texture; callers must change the
    /// key when the pixels change.
    ///
    /// # Errors
    ///
    /// Returns an error when the texture cannot be created.
    pub fn ensure_texture(&mut self, key: u64, data: &TextureData) -> Result<(), String> {
        if self.textures.contains_key(&key) {
            return Ok(());
        }
        let texture = make_texture(&self.device, data)?;
        self.textures.insert(key, texture);
        Ok(())
    }

    /// Renders a scene and reads the result back to the CPU.
    ///
    /// # Errors
    ///
    /// Returns an error for zero dimensions, allocation failures or a readback
    /// failure.
    pub fn render(&mut self, width: u32, height: u32, scene: &Scene) -> Result<Image, String> {
        let target = self.draw(width, height, scene)?;
        readback(&target, width, height)
    }

    /// Renders a scene and waits for the GPU without reading the pixels back.
    ///
    /// This is the frame path: a presented frame never needs the RGBA bytes on the
    /// CPU.
    ///
    /// # Errors
    ///
    /// Returns an error for zero dimensions or allocation failures.
    pub fn submit(&mut self, width: u32, height: u32, scene: &Scene) -> Result<(), String> {
        self.draw(width, height, scene).map(|_| ())
    }

    /// Number of items drawn by the most recent frame (diagnostics).
    #[must_use]
    pub fn last_drawn_items(&self) -> usize {
        self.drawn_items
    }

    fn draw(&mut self, width: u32, height: u32, scene: &Scene) -> Result<Target, String> {
        if width == 0 || height == 0 {
            return Err("image dimensions must be non-zero".into());
        }
        let target = new_target(&self.device, width, height, [1.0, 1.0, 1.0, 1.0])?;
        self.frame_target = None;
        let mut vertices: Vec<SceneVertex> = Vec::new();
        let mut draws: Vec<SceneDraw> = Vec::new();
        for item in &scene.items {
            draws.push(self.append_item(item, width, height, &mut vertices)?);
        }
        self.drawn_items = draws.len();
        self.encode_draws(&target, &vertices, &draws, true)?;
        Ok(target)
    }

    /// Draws only the items intersecting `damage` on top of the previous frame.
    ///
    /// The render target is kept between calls, so the pass loads its previous
    /// contents instead of clearing them. The first call for a size (or after a
    /// full [`SceneRenderer::submit`]) renders everything, because there is no
    /// previous frame to preserve. Returns the number of items drawn.
    ///
    /// # Errors
    ///
    /// Returns an error for zero dimensions, a missing cached texture key or an
    /// allocation failure.
    pub fn submit_incremental(
        &mut self,
        width: u32,
        height: u32,
        scene: &Scene,
        damage: &[Rect],
    ) -> Result<usize, String> {
        if width == 0 || height == 0 {
            return Err("image dimensions must be non-zero".into());
        }
        let continuing = matches!(
            &self.frame_target,
            Some((w, h, _)) if *w == width && *h == height
        );
        let target = match self.frame_target.take() {
            Some((w, h, target)) if w == width && h == height => {
                // SAFETY: the pass always owns attachment 0.
                let attachment =
                    unsafe { target.pass.colorAttachments().objectAtIndexedSubscript(0) };
                attachment.setLoadAction(MTLLoadAction::Load);
                target
            }
            _ => new_target(&self.device, width, height, [1.0, 1.0, 1.0, 1.0])?,
        };
        let mut vertices: Vec<SceneVertex> = Vec::new();
        let mut draws: Vec<SceneDraw> = Vec::new();
        for item in &scene.items {
            if continuing {
                if damage.is_empty() {
                    continue;
                }
                if !item_intersects(item, damage) {
                    continue;
                }
            }
            draws.push(self.append_item(item, width, height, &mut vertices)?);
        }
        self.drawn_items = draws.len();
        self.encode_draws(&target, &vertices, &draws, !continuing)?;
        self.frame_target = Some((width, height, target));
        Ok(draws.len())
    }

    fn append_item(
        &self,
        item: &SceneItem,
        width: u32,
        height: u32,
        vertices: &mut Vec<SceneVertex>,
    ) -> Result<SceneDraw, String> {
        let start = vertices.len();
        let (rect, texture, tint) = match item {
            SceneItem::Rect { rect, color } => (*rect, self.white.clone(), *color),
            SceneItem::Image {
                rect,
                texture,
                tint,
            } => (*rect, make_texture(&self.device, texture)?, *tint),
            SceneItem::CachedImage { rect, key, tint } => (
                *rect,
                self.textures
                    .get(key)
                    .cloned()
                    .ok_or_else(|| format!("no cached texture under key {key}"))?,
                *tint,
            ),
        };
        let to_ndc = |x: f32, y: f32| -> [f32; 2] {
            [x / width as f32 * 2.0 - 1.0, 1.0 - y / height as f32 * 2.0]
        };
        let (x0, y0, x1, y1) = (rect.min_x(), rect.min_y(), rect.max_x(), rect.max_y());
        let c = [
            f32::from(tint[0]) / 255.0,
            f32::from(tint[1]) / 255.0,
            f32::from(tint[2]) / 255.0,
            f32::from(tint[3]) / 255.0,
        ];
        let p0 = SceneVertex {
            position: to_ndc(x0, y0),
            uv: [0.0, 0.0],
            color: c,
        };
        let p1 = SceneVertex {
            position: to_ndc(x1, y0),
            uv: [1.0, 0.0],
            color: c,
        };
        let p2 = SceneVertex {
            position: to_ndc(x1, y1),
            uv: [1.0, 1.0],
            color: c,
        };
        let p3 = SceneVertex {
            position: to_ndc(x0, y1),
            uv: [0.0, 1.0],
            color: c,
        };
        vertices.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
        Ok((texture, start, 6))
    }

    fn encode_draws(
        &self,
        target: &Target,
        vertices: &[SceneVertex],
        draws: &[SceneDraw],
        clear: bool,
    ) -> Result<(), String> {
        if vertices.is_empty() {
            if clear {
                encode_and_wait(&self.queue, target, |_encoder| Ok(()))?;
            }
            return Ok(());
        }
        let bytes = std::mem::size_of_val(vertices);
        let pointer =
            std::ptr::NonNull::new(vertices.as_ptr().cast::<std::ffi::c_void>().cast_mut())
                .ok_or("vertex buffer is empty")?;
        // SAFETY: the pointer is valid for the duration of the call; the buffer copies.
        let buffer = unsafe {
            self.device.newBufferWithBytes_length_options(
                pointer,
                bytes,
                MTLResourceOptions::StorageModeShared,
            )
        }
        .ok_or("could not create the vertex buffer")?;

        encode_and_wait(&self.queue, target, |encoder| {
            encoder.setRenderPipelineState(&self.state);
            // SAFETY: buffer index 0 matches the shader signature and the sampler is
            // bound at index 0; every texture and vertex range belongs to a draw.
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(&buffer), 0, 0);
                encoder.setFragmentSamplerState_atIndex(Some(&self.sampler), 0);
                for (texture, start, count) in draws {
                    encoder.setFragmentTexture_atIndex(Some(texture), 0);
                    encoder.drawPrimitives_vertexStart_vertexCount(
                        MTLPrimitiveType::Triangle,
                        *start,
                        *count,
                    );
                }
            }
            Ok(())
        })
    }
}

/// Renders a scene once with a fresh Metal device and reads it back.
///
/// # Errors
///
/// Returns an error from [`SceneRenderer::new`] or [`SceneRenderer::render`].
pub fn render_scene(width: u32, height: u32, scene: &Scene) -> Result<Image, String> {
    SceneRenderer::new()?.render(width, height, scene)
}

fn item_rect(item: &SceneItem) -> Rect {
    match item {
        SceneItem::Rect { rect, .. }
        | SceneItem::Image { rect, .. }
        | SceneItem::CachedImage { rect, .. } => *rect,
    }
}

fn item_intersects(item: &SceneItem, damage: &[Rect]) -> bool {
    let rect = item_rect(item);
    damage.iter().any(|d| {
        rect.min_x() < d.max_x()
            && d.min_x() < rect.max_x()
            && rect.min_y() < d.max_y()
            && d.min_y() < rect.max_y()
    })
}

fn encode_and_wait(
    queue: &ProtocolObject<dyn MTLCommandQueue>,
    target: &Target,
    draw: impl FnOnce(&ProtocolObject<dyn MTLRenderCommandEncoder>) -> Result<(), String>,
) -> Result<(), String> {
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

fn build_scene_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>, String> {
    let source = NSString::from_str(SCENE_MSL);
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
    let attachment = unsafe { pipeline.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
    attachment.setBlendingEnabled(true);
    attachment.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);
    attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
    attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .newRenderPipelineStateWithDescriptor_error(&pipeline)
        .map_err(|e| format!("pipeline creation failed: {}", e.localizedDescription()))
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
    fn scene_draws_tinted_textures_over_rects() {
        let mut scene = Scene::new();
        scene.push(SceneItem::Image {
            rect: Rect::new(0.0, 0.0, 32.0, 32.0),
            texture: TextureData {
                width: 1,
                height: 1,
                rgba: vec![255, 255, 255, 255],
            },
            tint: [255, 0, 0, 255],
        });
        scene.push(SceneItem::Rect {
            rect: Rect::new(16.0, 16.0, 32.0, 32.0),
            color: [0, 0, 255, 255],
        });
        let Some(image) = render_scene(64, 64, &scene).ok() else {
            return;
        };
        assert_eq!(image.pixel(8, 8), Some([255, 0, 0, 255]));
        assert_eq!(image.pixel(24, 24), Some([0, 0, 255, 255]));
        assert_eq!(image.pixel(60, 8), Some([255, 255, 255, 255]));
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

    #[test]
    fn incremental_submission_filters_by_damage() {
        let Ok(mut renderer) = SceneRenderer::new() else {
            return;
        };
        let mut scene = Scene::new();
        scene.push(SceneItem::Rect {
            rect: Rect::new(0.0, 0.0, 2.0, 2.0),
            color: [255, 0, 0, 255],
        });
        scene.push(SceneItem::Rect {
            rect: Rect::new(10.0, 10.0, 2.0, 2.0),
            color: [0, 0, 255, 255],
        });
        // No previous frame: everything is drawn.
        let drawn = renderer.submit_incremental(20, 20, &scene, &[]).unwrap();
        assert_eq!(drawn, 2);
        // Damage on the first rect only.
        let drawn = renderer
            .submit_incremental(20, 20, &scene, &[Rect::new(0.0, 0.0, 2.0, 2.0)])
            .unwrap();
        assert_eq!(drawn, 1);
        // Empty damage means nothing changed.
        let drawn = renderer.submit_incremental(20, 20, &scene, &[]).unwrap();
        assert_eq!(drawn, 0);
    }

    #[test]
    fn a_cached_texture_is_uploaded_once_and_drawn() {
        let Ok(mut renderer) = SceneRenderer::new() else {
            return;
        };
        let data = TextureData {
            width: 1,
            height: 1,
            rgba: vec![0, 255, 0, 255],
        };
        renderer.ensure_texture(7, &data).unwrap();
        // A second call must be a no-op, not a re-upload or an error.
        renderer.ensure_texture(7, &data).unwrap();
        let mut scene = Scene::new();
        scene.push(SceneItem::CachedImage {
            rect: Rect::new(0.0, 0.0, 2.0, 2.0),
            key: 7,
            tint: [255, 255, 255, 255],
        });
        let image = renderer.render(4, 4, &scene).unwrap();
        assert_eq!(image.pixel(0, 0), Some([0, 255, 0, 255]));
    }

    #[test]
    fn a_scene_renderer_reuses_its_device_across_frames() {
        let Ok(mut renderer) = SceneRenderer::new() else {
            return;
        };
        let mut scene = Scene::new();
        scene.push(SceneItem::Rect {
            rect: Rect::new(0.0, 0.0, 2.0, 2.0),
            color: [255, 0, 0, 255],
        });
        let first = renderer.render(4, 4, &scene).unwrap();
        let second = renderer.render(4, 4, &scene).unwrap();
        assert_eq!(first, second, "the same scene must render identically");
        assert_eq!(first.pixel(0, 0), Some([255, 0, 0, 255]));
        renderer.submit(4, 4, &scene).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn capabilities_describe_the_metal_device() {
        let capabilities = match capabilities() {
            Ok(capabilities) => capabilities,
            Err(error) => {
                // GitHub's macOS runners can boot without a usable GPU, so this
                // test is skipped there; on a real Apple Silicon Mac it asserts.
                eprintln!("skipped: Metal unavailable: {error}");
                return;
            }
        };
        assert!(
            !capabilities.device_name.is_empty(),
            "device name must be populated"
        );
        assert!(
            capabilities.max_buffer_length >= 1 << 20,
            "a Metal device must allow at least a 1 MiB buffer, got {}",
            capabilities.max_buffer_length
        );
        assert!(
            capabilities.recommended_max_working_set_size > 0,
            "recommended working set size must be populated"
        );
        // Families are monotonically nested: a device that knows Apple9 also
        // knows Apple7. This is a real invariant, not a hardware assumption.
        if capabilities.families.apple9 {
            assert!(capabilities.families.apple7);
        }
        if capabilities.families.apple8 {
            assert!(capabilities.families.apple7);
        }
    }
}
