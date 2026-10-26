//! Metal implementation of the frame-time benchmark.
//!
//! One device, queue, pipeline, sampler, texture, vertex buffer and render
//! target live for the whole run; only the command buffer and render encoder
//! are created per frame, which is exactly the steady-state cost this tool
//! measures.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::time::Instant;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlendFactor, MTLBuffer, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat,
    MTLPrimitiveType, MTLRegion, MTLRenderCommandEncoder, MTLRenderPassDescriptor,
    MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLResourceOptions, MTLSamplerAddressMode,
    MTLSamplerDescriptor, MTLSamplerMinMagFilter, MTLSamplerState, MTLSize, MTLStorageMode,
    MTLStoreAction, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};

use crate::{
    BenchConfig, BenchReport, BuildInfo, DeviceInfo, FrameSample, FrameStats, SceneInfo,
    TEXTURE_SIZE,
};

/// MSL shared by the benchmark: one textured, tinted quad per draw call.
const BENCH_MSL: &str = r#"
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

/// Vertex layout shared with `BENCH_MSL`; `float4` alignment puts the colour
/// at byte 16, so the struct is 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

/// Render target plus the pass descriptor that targets it.
struct Target {
    _texture: Retained<ProtocolObject<dyn MTLTexture>>,
    pass: Retained<MTLRenderPassDescriptor>,
}

/// All GPU objects that are reused for every frame of the run.
struct Renderer {
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    texture: Retained<ProtocolObject<dyn MTLTexture>>,
    vertices: Retained<ProtocolObject<dyn MTLBuffer>>,
    target: Target,
    quads: u32,
}

impl Renderer {
    fn new(config: &BenchConfig) -> Result<Self, String> {
        let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
        let queue = device
            .newCommandQueue()
            .ok_or("could not create a command queue")?;
        let pipeline = build_pipeline(&device)?;
        let sampler = build_sampler(&device)?;
        let texture = make_texture(&device, TEXTURE_SIZE, TEXTURE_SIZE, &pattern())?;
        let vertices = build_quad_vertices(config.quads, config.width, config.height);
        let vertex_count = vertices.len();
        let vertex_bytes = std::mem::size_of_val(vertices.as_slice());
        let pointer = NonNull::new(vertices.as_ptr().cast::<c_void>().cast_mut())
            .ok_or("vertex buffer is empty")?;
        // SAFETY: the pointer is valid for `vertex_bytes` while the Vec is
        // alive and Metal copies the bytes into its own buffer.
        let vertices = unsafe {
            device.newBufferWithBytes_length_options(
                pointer,
                vertex_bytes,
                MTLResourceOptions::StorageModeShared,
            )
        }
        .ok_or("could not create the vertex buffer")?;
        let target = new_target(&device, config.width, config.height)?;
        Ok(Self {
            queue,
            pipeline,
            sampler,
            texture,
            vertices,
            target,
            quads: u32::try_from(vertex_count / 6).map_err(|_| "quad count overflow")?,
        })
    }

    /// Encodes and submits one frame, returning `(cpu_submit_ms,
    /// gpu_completion_ms, gpu_execution_ms)`.
    fn submit(&self) -> Result<(f64, f64, Option<f64>), String> {
        let encode_start = Instant::now();
        let command_buffer = self
            .queue
            .commandBuffer()
            .ok_or("could not create a command buffer")?;
        let encoder = command_buffer
            .renderCommandEncoderWithDescriptor(&self.target.pass)
            .ok_or("could not create a render encoder")?;
        encoder.setRenderPipelineState(&self.pipeline);
        // SAFETY: buffer index 0, texture index 0 and sampler index 0 match
        // `BENCH_MSL`; every draw range lies inside the shared vertex buffer.
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&self.vertices), 0, 0);
            encoder.setFragmentTexture_atIndex(Some(&self.texture), 0);
            encoder.setFragmentSamplerState_atIndex(Some(&self.sampler), 0);
            for quad in 0..self.quads {
                encoder.drawPrimitives_vertexStart_vertexCount(
                    MTLPrimitiveType::Triangle,
                    quad as usize * 6,
                    6,
                );
            }
        }
        encoder.endEncoding();
        command_buffer.commit();
        let cpu_submit_ms = encode_start.elapsed().as_secs_f64() * 1000.0;

        let wait_start = Instant::now();
        command_buffer.waitUntilCompleted();
        let gpu_completion_ms = wait_start.elapsed().as_secs_f64() * 1000.0;

        if let Some(error) = command_buffer.error() {
            return Err(format!(
                "GPU command buffer failed: {}",
                error.localizedDescription()
            ));
        }
        let start = command_buffer.GPUStartTime();
        let end = command_buffer.GPUEndTime();
        let gpu_execution_ms = (start > 0.0 && end > start).then_some((end - start) * 1000.0);
        Ok((cpu_submit_ms, gpu_completion_ms, gpu_execution_ms))
    }
}

fn build_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>, String> {
    let source = NSString::from_str(BENCH_MSL);
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

fn build_sampler(
    device: &ProtocolObject<dyn MTLDevice>,
) -> Result<Retained<ProtocolObject<dyn MTLSamplerState>>, String> {
    let descriptor = MTLSamplerDescriptor::new();
    descriptor.setMinFilter(MTLSamplerMinMagFilter::Linear);
    descriptor.setMagFilter(MTLSamplerMinMagFilter::Linear);
    descriptor.setSAddressMode(MTLSamplerAddressMode::ClampToEdge);
    descriptor.setTAddressMode(MTLSamplerAddressMode::ClampToEdge);
    device
        .newSamplerStateWithDescriptor(&descriptor)
        .ok_or_else(|| "could not create a sampler".to_string())
}

fn new_target(
    device: &ProtocolObject<dyn MTLDevice>,
    width: u32,
    height: u32,
) -> Result<Target, String> {
    // SAFETY: valid pixel format and non-zero dimensions.
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
        red: 0.02,
        green: 0.03,
        blue: 0.05,
        alpha: 1.0,
    });
    Ok(Target {
        _texture: texture,
        pass,
    })
}

fn make_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>, String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(format!(
            "texture has {} bytes, expected {expected}",
            rgba.len()
        ));
    }
    // SAFETY: valid pixel format and non-zero dimensions.
    let descriptor = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            MTLPixelFormat::RGBA8Unorm,
            width as usize,
            height as usize,
            false,
        )
    };
    descriptor.setUsage(MTLTextureUsage::ShaderRead);
    descriptor.setStorageMode(MTLStorageMode::Shared);
    let texture = device
        .newTextureWithDescriptor(&descriptor)
        .ok_or("could not create the shared texture")?;
    let pointer =
        NonNull::new(rgba.as_ptr().cast::<c_void>().cast_mut()).ok_or("texture data is empty")?;
    let region = MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width: width as usize,
            height: height as usize,
            depth: 1,
        },
    };
    // SAFETY: the slice is exactly width*height*4 bytes and the region covers
    // the whole texture; storage mode is Shared so a CPU upload is allowed.
    unsafe {
        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            region,
            0,
            pointer,
            width as usize * 4,
        );
    }
    Ok(texture)
}

/// Deterministic 256x256 RGBA8 pattern, uploaded once and shared by every quad.
fn pattern() -> Vec<u8> {
    let size = TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let checker = (x / 32 + y / 32) % 2 == 0;
            let gradient = ((x + y) * 255 / (2 * size)) as u8;
            let base: u8 = if checker { 190 } else { 55 };
            let red = (u16::from(base) + u16::from(gradient) / 2).min(255) as u8;
            rgba.extend_from_slice(&[red, gradient, base, 255]);
        }
    }
    rgba
}

/// Builds `quads * 6` vertices covering the target in a jittered, overlapping
/// grid so the GPU has real fill work, using one reused vertex buffer.
fn build_quad_vertices(quads: u32, width: u32, height: u32) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(quads as usize * 6);
    let cols = (f64::from(quads)).sqrt().ceil().max(1.0) as u32;
    let rows = quads.div_ceil(cols).max(1);
    let cell_w = width as f32 / cols as f32;
    let cell_h = height as f32 / rows as f32;
    let to_ndc = |x: f32, y: f32| -> [f32; 2] {
        [x / width as f32 * 2.0 - 1.0, 1.0 - y / height as f32 * 2.0]
    };
    for quad in 0..quads {
        let col = quad % cols;
        let row = quad / cols;
        let hash = quad.wrapping_mul(2_654_435_761);
        let jitter_x = (f32::from((hash & 0xff) as u8) / 255.0 - 0.5) * cell_w * 0.5;
        let jitter_y = (f32::from(((hash >> 8) & 0xff) as u8) / 255.0 - 0.5) * cell_h * 0.5;
        let center_x = (col as f32 + 0.5) * cell_w + jitter_x;
        let center_y = (row as f32 + 0.5) * cell_h + jitter_y;
        // 1.5x the cell means neighbouring quads overlap, so the scene is not a
        // disjoint tiling with exactly one screen of coverage.
        let half_w = cell_w * 0.75;
        let half_h = cell_h * 0.75;
        let (x0, y0, x1, y1) = (
            center_x - half_w,
            center_y - half_h,
            center_x + half_w,
            center_y + half_h,
        );
        let color = [
            0.25 + f32::from((hash >> 16) as u8) / 510.0,
            0.25 + f32::from((hash >> 24) as u8) / 510.0,
            0.25 + f32::from((hash >> 8) as u8) / 510.0,
            1.0,
        ];
        let p0 = Vertex {
            position: to_ndc(x0, y0),
            uv: [0.0, 0.0],
            color,
        };
        let p1 = Vertex {
            position: to_ndc(x1, y0),
            uv: [1.0, 0.0],
            color,
        };
        let p2 = Vertex {
            position: to_ndc(x1, y1),
            uv: [1.0, 1.0],
            color,
        };
        let p3 = Vertex {
            position: to_ndc(x0, y1),
            uv: [0.0, 1.0],
            color,
        };
        vertices.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
    }
    vertices
}

/// Runs warm-up frames, then the measured frames, and assembles the report.
pub(crate) fn run(config: &BenchConfig) -> Result<BenchReport, String> {
    let capabilities = jkva_gfx::capabilities()?;
    let vertex_bytes = (config.quads as usize)
        .checked_mul(6)
        .and_then(|count| count.checked_mul(std::mem::size_of::<Vertex>()))
        .ok_or("scene is too large to allocate")?;
    if vertex_bytes as u64 > capabilities.max_buffer_length {
        return Err(format!(
            "scene needs a {vertex_bytes} byte vertex buffer, above the device limit of {} bytes",
            capabilities.max_buffer_length
        ));
    }

    let renderer = Renderer::new(config)?;
    for _ in 0..config.warmup_frames {
        let _ = renderer.submit()?;
    }

    let mut samples = Vec::with_capacity(config.frames as usize);
    for frame in 0..config.frames {
        let (cpu_submit_ms, gpu_completion_ms, gpu_execution_ms) = renderer.submit()?;
        samples.push(FrameSample {
            frame,
            cpu_submit_ms,
            gpu_completion_ms,
            gpu_execution_ms,
        });
    }

    let cpu_submit = FrameStats::from_samples(samples.iter().map(|s| s.cpu_submit_ms).collect());
    let gpu_completion =
        FrameStats::from_samples(samples.iter().map(|s| s.gpu_completion_ms).collect());
    let execution_samples = samples
        .iter()
        .filter_map(|sample| sample.gpu_execution_ms)
        .collect::<Vec<_>>();
    let gpu_execution = if execution_samples.is_empty() {
        None
    } else {
        Some(FrameStats::from_samples(execution_samples))
    };
    let gpu_execution_source = gpu_execution
        .as_ref()
        .map(|_| "MTLCommandBuffer.GPUStartTime/GPUEndTime".to_string());

    Ok(BenchReport {
        build: BuildInfo {
            crate_version: env!("CARGO_PKG_VERSION").to_string(),
            profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
        },
        device: DeviceInfo::from_capabilities(&capabilities),
        scene: SceneInfo {
            quads: config.quads,
            draw_calls_per_frame: config.quads,
            vertices_per_frame: config.quads as usize * 6,
            vertex_buffer_bytes: vertex_bytes,
            texture_width: TEXTURE_SIZE,
            texture_height: TEXTURE_SIZE,
            target_width: config.width,
            target_height: config.height,
        },
        config: config.clone(),
        samples,
        cpu_submit,
        gpu_completion,
        gpu_execution,
        gpu_execution_source,
    })
}
