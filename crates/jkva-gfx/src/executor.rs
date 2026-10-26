//! Metal executor for the `jkva-gpu` command ABI.
//!
//! This is the first backend for the backend-neutral contract: it translates a
//! validated [`CommandList`] into Metal objects and draw calls. The surface is
//! deliberately small — buffers, render-target textures, one pass at a time,
//! vertex buffer binding and draws with the scene pipeline.
//!
//! Pipeline `0` is the scene pipeline: vertices are `SceneVertex`
//! (position xy, uv, rgba as eight `f32`s) and the fragment shader samples the
//! bound 1×1 white texture. Texture binding and pipelines arrive with the real
//! WebGPU-class API.

use std::collections::HashMap;

use jkva_gpu::{Command, CommandList, ResourceId, TextureUsage};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBuffer, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLDevice,
    MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLRenderCommandEncoder,
    MTLRenderPassDescriptor, MTLRenderPipelineState, MTLResourceOptions, MTLSamplerAddressMode,
    MTLSamplerDescriptor, MTLSamplerMinMagFilter, MTLSamplerState, MTLSize, MTLStorageMode,
    MTLStoreAction, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};

use crate::{Image, TextureData, build_scene_pipeline, make_texture, new_target};

/// Executes `jkva-gpu` command lists on Metal.
pub struct GpuExecutor {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    white: Retained<ProtocolObject<dyn MTLTexture>>,
    buffers: HashMap<ResourceId, Retained<ProtocolObject<dyn MTLBuffer>>>,
    targets: HashMap<ResourceId, Retained<ProtocolObject<dyn MTLTexture>>>,
    sampled: HashMap<ResourceId, Retained<ProtocolObject<dyn MTLTexture>>>,
    current_texture: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,
    bound_vertex: Option<Retained<ProtocolObject<dyn MTLBuffer>>>,
    last_target: Option<ResourceId>,
}

impl GpuExecutor {
    /// Creates the executor, or returns why Metal is unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error when the device, queue, pipeline, sampler or white
    /// texture cannot be created.
    pub fn new() -> Result<Self, String> {
        let device = objc2_metal::MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
        let queue = device
            .newCommandQueue()
            .ok_or("could not create a command queue")?;
        let pipeline = build_scene_pipeline(&device)?;
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
            pipeline,
            sampler,
            white,
            buffers: HashMap::new(),
            targets: HashMap::new(),
            sampled: HashMap::new(),
            current_texture: None,
            command_buffer: None,
            encoder: None,
            bound_vertex: None,
            last_target: None,
        })
    }

    /// Executes a validated command list and waits for the GPU.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown resource, a missing vertex buffer, a
    /// malformed pass sequence or a Metal allocation failure.
    pub fn execute(&mut self, list: &CommandList) -> Result<(), String> {
        for command in &list.commands {
            match command {
                Command::CreateBuffer { id, size, .. } => {
                    let buffer = self
                        .device
                        .newBufferWithLength_options(
                            *size as usize,
                            MTLResourceOptions::StorageModeShared,
                        )
                        .ok_or("could not create the buffer")?;
                    self.buffers.insert(*id, buffer);
                    self.targets.remove(id);
                    self.sampled.remove(id);
                }
                Command::WriteBuffer { id, offset, data } => {
                    let buffer = self
                        .buffers
                        .get(id)
                        .ok_or_else(|| format!("write to unknown buffer {}", id.0))?;
                    let base = buffer.contents();
                    // SAFETY: the ABI validator checked that `offset + data.len()`
                    // fits inside the buffer, whose storage is Shared.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            data.as_ptr(),
                            base.as_ptr().cast::<u8>().add(*offset as usize),
                            data.len(),
                        );
                    }
                }
                Command::CreateTexture {
                    id,
                    width,
                    height,
                    usage,
                    ..
                } => {
                    let texture = match usage {
                        TextureUsage::RenderTarget => {
                            new_target(&self.device, *width, *height, [0.0, 0.0, 0.0, 1.0])?.texture
                        }
                        TextureUsage::Sampled => sampled_texture(&self.device, *width, *height)?,
                    };
                    match usage {
                        TextureUsage::RenderTarget => {
                            self.targets.insert(*id, texture);
                        }
                        TextureUsage::Sampled => {
                            self.sampled.insert(*id, texture);
                        }
                    }
                    self.buffers.remove(id);
                }
                Command::WriteTexture { id, data } => {
                    let texture = self
                        .sampled
                        .get(id)
                        .ok_or_else(|| format!("unknown sampled texture {}", id.0))?;
                    let width = texture.width();
                    let height = texture.height();
                    let region = MTLRegion {
                        origin: MTLOrigin { x: 0, y: 0, z: 0 },
                        size: MTLSize {
                            width,
                            height,
                            depth: 1,
                        },
                    };
                    let pixels =
                        std::ptr::NonNull::new(data.as_ptr().cast_mut().cast::<std::ffi::c_void>())
                            .ok_or("empty texture upload")?;
                    // SAFETY: the ABI validator checked `data` is exactly
                    // width*height*4 bytes and the texture is Shared.
                    unsafe {
                        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                            region,
                            0,
                            pixels,
                            width as usize * 4,
                        );
                    }
                }
                Command::BindTexture { id, .. } => {
                    let texture = self
                        .sampled
                        .get(id)
                        .ok_or_else(|| format!("unknown sampled texture {}", id.0))?;
                    self.current_texture = Some(texture.clone());
                }
                Command::BindVertexBuffer { id, .. } => {
                    let buffer = self
                        .buffers
                        .get(id)
                        .ok_or_else(|| format!("unknown vertex buffer {}", id.0))?;
                    self.bound_vertex = Some(buffer.clone());
                }
                Command::BeginPass { target, clear } => {
                    if self.encoder.is_some() {
                        return Err("a render pass is already open".into());
                    }
                    let texture = self
                        .targets
                        .get(target)
                        .ok_or_else(|| format!("unknown render target {}", target.0))?
                        .clone();
                    let pass = MTLRenderPassDescriptor::renderPassDescriptor();
                    // SAFETY: the pass always owns attachment 0.
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
                    let command_buffer = self
                        .queue
                        .commandBuffer()
                        .ok_or("could not create a command buffer")?;
                    let encoder = command_buffer
                        .renderCommandEncoderWithDescriptor(&pass)
                        .ok_or("could not create a render encoder")?;
                    encoder.setRenderPipelineState(&self.pipeline);
                    // SAFETY: the sampler and the white texture are bound at the
                    // indices the scene shader declares.
                    unsafe {
                        encoder.setFragmentSamplerState_atIndex(Some(&self.sampler), 0);
                        encoder.setFragmentTexture_atIndex(Some(&self.white), 0);
                    }
                    self.command_buffer = Some(command_buffer);
                    self.encoder = Some(encoder);
                    self.last_target = Some(*target);
                }
                Command::Draw {
                    pipeline,
                    first,
                    vertices,
                    instances,
                } => {
                    if *pipeline != 0 {
                        return Err(format!("unknown pipeline {pipeline}"));
                    }
                    if *instances != 1 {
                        return Err("instanced draws are not implemented yet".into());
                    }
                    let encoder = self
                        .encoder
                        .as_ref()
                        .ok_or("draw without an open render pass")?;
                    let buffer = self
                        .bound_vertex
                        .as_ref()
                        .ok_or("draw without a bound vertex buffer")?;
                    // SAFETY: the buffer is bound at slot 0, matching the scene
                    // pipeline vertex descriptor, and the range comes from the ABI.
                    unsafe {
                        encoder.setVertexBuffer_offset_atIndex(Some(buffer), 0, 0);
                        let texture = self.current_texture.as_ref().unwrap_or(&self.white);
                        encoder.setFragmentTexture_atIndex(Some(texture), 0);
                        encoder.drawPrimitives_vertexStart_vertexCount(
                            MTLPrimitiveType::Triangle,
                            *first as usize,
                            *vertices as usize,
                        );
                    }
                }
                Command::EndPass => {
                    let encoder = self.encoder.take().ok_or("no render pass to end")?;
                    encoder.endEncoding();
                    let command_buffer = self.command_buffer.take().ok_or("no command buffer")?;
                    command_buffer.commit();
                    command_buffer.waitUntilCompleted();
                    self.bound_vertex = None;
                }
            }
        }
        Ok(())
    }

    /// Reads a render-target texture back to the CPU.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown texture or an overflow in the readback size.
    pub fn read_texture(&self, id: ResourceId) -> Result<Image, String> {
        let texture = self
            .targets
            .get(&id)
            .or_else(|| self.sampled.get(&id))
            .ok_or_else(|| format!("unknown texture {}", id.0))?;
        let width = texture.width() as u32;
        let height = texture.height() as u32;
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
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
        // SAFETY: the buffer is exactly `bytes` long and the region covers the
        // texture, whose storage mode is Shared.
        unsafe {
            texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
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
}

fn sampled_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    width: u32,
    height: u32,
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>, String> {
    if width == 0 || height == 0 {
        return Err("texture dimensions must be non-zero".into());
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
    device
        .newTextureWithDescriptor(&descriptor)
        .ok_or_else(|| "could not create the sampled texture".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SceneVertex;
    use jkva_gpu::{BufferUsage, Command, CommandList, TextureFormat, TextureUsage};

    #[test]
    fn a_triangle_from_a_command_list_renders() {
        let Ok(mut executor) = GpuExecutor::new() else {
            return;
        };
        let vertices = [
            SceneVertex {
                position: [-1.0, -1.0],
                uv: [0.0, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            SceneVertex {
                position: [3.0, -1.0],
                uv: [0.0, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            SceneVertex {
                position: [-1.0, 3.0],
                uv: [0.0, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
        ];
        // SAFETY: SceneVertex is repr(C) and all fields are f32, so the byte view
        // is exactly the vertex data the pipeline expects.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr().cast::<u8>(),
                std::mem::size_of_val(&vertices),
            )
        };
        let mut list = CommandList::new();
        list.push(Command::CreateBuffer {
            id: ResourceId(1),
            size: bytes.len() as u64,
            usage: BufferUsage::Vertex,
        });
        list.push(Command::WriteBuffer {
            id: ResourceId(1),
            offset: 0,
            data: bytes.to_vec(),
        });
        list.push(Command::CreateTexture {
            id: ResourceId(2),
            width: 8,
            height: 8,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsage::RenderTarget,
        });
        list.push(Command::BeginPass {
            target: ResourceId(2),
            clear: [0.0, 0.0, 0.0, 1.0],
        });
        list.push(Command::BindVertexBuffer {
            slot: 0,
            id: ResourceId(1),
        });
        list.push(Command::Draw {
            pipeline: 0,
            first: 0,
            vertices: 3,
            instances: 1,
        });
        list.push(Command::EndPass);
        executor.execute(&list).unwrap();
        let image = executor.read_texture(ResourceId(2)).unwrap();
        assert_eq!(image.pixel(4, 4), Some([255, 0, 0, 255]));
    }

    #[test]
    fn a_bound_sampled_texture_tints_the_draw() {
        let Ok(mut executor) = GpuExecutor::new() else {
            return;
        };
        let vertices = [
            SceneVertex {
                position: [-1.0, -1.0],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            SceneVertex {
                position: [3.0, -1.0],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            SceneVertex {
                position: [-1.0, 3.0],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
        ];
        // SAFETY: SceneVertex is repr(C) and all fields are f32.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr().cast::<u8>(),
                std::mem::size_of_val(&vertices),
            )
        };
        let mut list = CommandList::new();
        list.push(Command::CreateBuffer {
            id: ResourceId(1),
            size: bytes.len() as u64,
            usage: BufferUsage::Vertex,
        });
        list.push(Command::WriteBuffer {
            id: ResourceId(1),
            offset: 0,
            data: bytes.to_vec(),
        });
        list.push(Command::CreateTexture {
            id: ResourceId(2),
            width: 1,
            height: 1,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsage::Sampled,
        });
        list.push(Command::WriteTexture {
            id: ResourceId(2),
            data: vec![0, 255, 0, 255],
        });
        list.push(Command::CreateTexture {
            id: ResourceId(3),
            width: 8,
            height: 8,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsage::RenderTarget,
        });
        list.push(Command::BeginPass {
            target: ResourceId(3),
            clear: [0.0, 0.0, 0.0, 1.0],
        });
        list.push(Command::BindVertexBuffer {
            slot: 0,
            id: ResourceId(1),
        });
        list.push(Command::BindTexture {
            slot: 0,
            id: ResourceId(2),
        });
        list.push(Command::Draw {
            pipeline: 0,
            first: 0,
            vertices: 3,
            instances: 1,
        });
        list.push(Command::EndPass);
        executor.execute(&list).unwrap();
        let image = executor.read_texture(ResourceId(3)).unwrap();
        assert_eq!(image.pixel(4, 4), Some([0, 255, 0, 255]));
    }
}
