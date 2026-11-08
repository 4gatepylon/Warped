//! `metal` is the Metal backend of [`Renderer`]: it owns the device, the `CAMetalLayer` from
//! SDL's Metal view, and every GPU resource, and it encodes each frame's passes.
//!
//! A frame is: four shadow cascades, a depth and normal prepass, SSAO and its blur, the forward
//! PBR pass (with sky, decals, transparent surfaces, and particles), the viewmodel, bloom, ACES
//! tone mapping, and FXAA plus the HUD into the drawable.
//!
//! Example:
//!     let mut renderer: MetalRenderer = MetalRenderer::new(layer);
//!     let timing: FrameTiming = renderer.render_frame(&frame, &settings);

mod environment;
mod gpu_types;
mod pipelines;
mod resources;

use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr::NonNull;

use glam::{Mat4, UVec4, Vec2, Vec3, Vec4};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::CGSize;
use objc2_metal::{
    MTLBuffer, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLCreateSystemDefaultDevice, MTLCullMode, MTLDevice, MTLDrawable, MTLIndexType, MTLLoadAction,
    MTLPixelFormat, MTLPrimitiveType, MTLRenderCommandEncoder, MTLRenderPassDescriptor,
    MTLResourceOptions, MTLStoreAction, MTLTexture, MTLTextureType, MTLTextureUsage, MTLViewport,
    MTLWinding,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use self::environment::{
    IblTextures, PREFILTERED_MIP_COUNT, SkyVisibility, bake_sky_visibility, build_ibl,
};
use self::gpu_types::{
    GpuDecal, GpuDrawUniforms, GpuFrameUniforms, GpuHudImage, GpuHudRect, GpuMaterialUniforms,
    GpuParticle, GpuPassUniforms, GpuPointLight,
};
use self::pipelines::{
    AO_FORMAT, DEPTH_FORMAT, DRAWABLE_FORMAT, HDR_FORMAT, LDR_FORMAT, NORMAL_FORMAT, PipelinePair,
    Pipelines,
};
use self::resources::{
    Buffer, Texture, TextureSpec, create_buffer, create_texture, mip_view, upload_slices,
    upload_texture_2d,
};
use crate::render::culling::{CASCADE_COUNT, CascadeCamera, Cascades, Frustum, fit_cascades};
use crate::render::{
    AlphaMode, Atmosphere, DrawItem, EnvironmentData, FrameDescription, FrameTiming, LENS_FACES,
    LensMouth, MAX_LIVE_LENS_SLOT_COUNT, MaterialData, MaterialHandle, MeshData, MeshHandle,
    OccluderBox, Particle, RenderSettings, Renderer, TextureData, TextureHandle,
    pick_live_lens_mouths,
};

/// `FRAMES_IN_FLIGHT` is how many frames the CPU may encode ahead of the GPU.
const FRAMES_IN_FLIGHT: usize = 3;
/// `DYNAMIC_BUFFER_BYTES` is the size of each frame's buffer of joints, decals, and particles.
const DYNAMIC_BUFFER_BYTES: usize = 8 * 1024 * 1024;
/// `DYNAMIC_ALIGNMENT_BYTES` is the alignment of each allocation in the dynamic buffer.
const DYNAMIC_ALIGNMENT_BYTES: usize = 256;
/// `SHADOW_MAP_SIZE_PX` is the size of each shadow cascade.
const SHADOW_MAP_SIZE_PX: u32 = 2048;
/// `SHADOW_DEPTH_BIAS` is the constant depth bias of the shadow pass.
const SHADOW_DEPTH_BIAS: f32 = 0.0004;
/// `SHADOW_SLOPE_BIAS` is the slope-scaled depth bias of the shadow pass.
const SHADOW_SLOPE_BIAS: f32 = 1.6;
/// `VIEWMODEL_NEAR_M` is the near plane of the viewmodel's projection.
const VIEWMODEL_NEAR_M: f32 = 0.01;
/// `BLOOM_LEVEL_COUNT` is the number of mips in the bloom chain.
const BLOOM_LEVEL_COUNT: usize = 6;
/// `MAX_POINT_LIGHT_COUNT` is the number of point lights the shaders read; `common.metal` agrees.
const MAX_POINT_LIGHT_COUNT: usize = 64;
/// `QUAD_VERTEX_COUNT` is the vertex count of a triangle-strip quad.
const QUAD_VERTEX_COUNT: usize = 4;
/// `FULLSCREEN_VERTEX_COUNT` is the vertex count of a full-screen triangle.
const FULLSCREEN_VERTEX_COUNT: usize = 3;
/// `SECONDS_TO_MS` converts seconds to milliseconds.
const SECONDS_TO_MS: f64 = 1000.0;
/// `MATRIX_BYTES` is the size of one joint matrix.
const MATRIX_BYTES: usize = 64;
/// `MAX_LENS_MOVER_COUNT` is how many robots' bounding spheres the lens pass looks for robots in;
/// `post.metal`'s `LensUniforms` agrees.
const MAX_LENS_MOVER_COUNT: usize = 16;
/// `LENS_LIVE_UNUSED_SPLIT_M` puts every point in the last shadow cascade when the lens's live
/// faces are drawn, since the cascades were fitted to the player's view, not theirs.
const LENS_LIVE_UNUSED_SPLIT_M: f32 = 0.0;
const LENS_LIVE_LAST_SPLIT_M: f32 = 1.0e6;

/// `LensLive` is where the robots and lasers near some wormhole mouths are drawn each frame, seen
/// from those mouths' centres: one array slice per live slot, a row of six faces (oriented as
/// `LENS_FACES`), in HDR color (alpha is coverage) and depth. Each live slot is its own render
/// pass, so a frame stores only the slots it uses.
struct LensLive {
    color: Texture,
    depth: Texture,
    face_px: usize,
    mouths: Vec<LensMouth>,
}

/// `CommandBuffer` is a retained Metal command buffer.
type CommandBuffer = Retained<ProtocolObject<dyn MTLCommandBuffer>>;
/// `Encoder` is a Metal render command encoder.
type Encoder = ProtocolObject<dyn MTLRenderCommandEncoder>;

/// `GpuMesh` is a mesh's buffers.
struct GpuMesh {
    vertices: Buffer,
    skin_vertices: Option<Buffer>,
    indices: Buffer,
    index_count: usize,
}

/// `GpuMaterial` is a material's four textures and its factors.
struct GpuMaterial {
    textures: [Texture; 4],
    uniforms: GpuMaterialUniforms,
    alpha_mode: AlphaMode,
}

/// `Targets` are the render targets that depend on the internal resolution.
struct Targets {
    width_px: usize,
    height_px: usize,
    depth: Texture,
    normals: Texture,
    ao: Texture,
    ao_scratch: Texture,
    hdr: Texture,
    bloom_levels: Vec<Texture>,
    ldr: Texture,
    lens: Texture,
}

impl Targets {
    /// `new` creates the targets for one internal resolution.
    fn new(device: &ProtocolObject<dyn MTLDevice>, width_px: usize, height_px: usize) -> Targets {
        let half_width_px: usize = (width_px / 2).max(1);
        let half_height_px: usize = (height_px / 2).max(1);
        let bloom: Texture = create_texture(
            device,
            &TextureSpec {
                mip_count: BLOOM_LEVEL_COUNT,
                ..TextureSpec::render_target(HDR_FORMAT, half_width_px, half_height_px)
            },
            "bloom",
        );
        let bloom_levels: Vec<Texture> = (0..BLOOM_LEVEL_COUNT)
            .map(|level: usize| mip_view(&bloom, MTLTextureType::Type2D, level, 1))
            .collect();
        Targets {
            width_px,
            height_px,
            depth: create_texture(
                device,
                &TextureSpec::render_target(DEPTH_FORMAT, width_px, height_px),
                "depth",
            ),
            normals: create_texture(
                device,
                &TextureSpec::render_target(NORMAL_FORMAT, width_px, height_px),
                "normals",
            ),
            ao: create_texture(
                device,
                &TextureSpec::render_target(AO_FORMAT, half_width_px, half_height_px),
                "ao",
            ),
            ao_scratch: create_texture(
                device,
                &TextureSpec::render_target(AO_FORMAT, half_width_px, half_height_px),
                "ao_scratch",
            ),
            hdr: create_texture(
                device,
                &TextureSpec::render_target(HDR_FORMAT, width_px, height_px),
                "hdr",
            ),
            bloom_levels,
            ldr: create_texture(
                device,
                &TextureSpec::render_target(LDR_FORMAT, width_px, height_px),
                "ldr",
            ),
            lens: create_texture(
                device,
                &TextureSpec::render_target(LDR_FORMAT, half_width_px, half_height_px),
                "lens",
            ),
        }
    }
}

/// `DynamicBuffer` is one frame's bump-allocated upload buffer.
struct DynamicBuffer {
    buffer: Buffer,
    offset_bytes: usize,
}

impl DynamicBuffer {
    /// `push` copies `data` into the buffer and returns its offset; if the buffer is full it
    /// copies as many leading elements as fit.
    fn push<T: Copy>(&mut self, data: &[T]) -> usize {
        let offset_bytes: usize = self.offset_bytes.next_multiple_of(DYNAMIC_ALIGNMENT_BYTES);
        let available_bytes: usize = DYNAMIC_BUFFER_BYTES.saturating_sub(offset_bytes);
        let count: usize = data
            .len()
            .min(available_bytes / std::mem::size_of::<T>().max(1));
        let length_bytes: usize = count * std::mem::size_of::<T>();
        unsafe {
            let destination: *mut u8 = self
                .buffer
                .contents()
                .as_ptr()
                .cast::<u8>()
                .add(offset_bytes);
            std::ptr::copy_nonoverlapping(data.as_ptr().cast::<u8>(), destination, length_bytes);
        }
        self.offset_bytes = offset_bytes + length_bytes;
        offset_bytes.min(DYNAMIC_BUFFER_BYTES - DYNAMIC_ALIGNMENT_BYTES)
    }
}

/// `EnvironmentState` is the sky's lighting and the atmosphere chosen to go with it.
struct EnvironmentState {
    ibl: IblTextures,
    sh9: [Vec4; 9],
    sun_direction: Vec4,
    sun_illuminance: Vec4,
    atmosphere: Atmosphere,
}

/// `FrameBuffers` are the offsets of this frame's uploads in the dynamic buffer.
struct FrameBuffers<'a> {
    dynamic: &'a Buffer,
    joints_offset_bytes: usize,
    frame: &'a GpuFrameUniforms,
    point_lights: &'a [GpuPointLight],
}

/// `ScenePass` says which pass a list of mesh draws belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScenePass {
    Shadow,
    Prepass,
    Forward,
}

/// `MetalRenderer` renders frames into a `CAMetalLayer` with Metal.
///
/// Example:
///     let mut renderer: MetalRenderer = MetalRenderer::new(layer);
pub struct MetalRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    layer: Retained<CAMetalLayer>,
    pipelines: Pipelines,
    textures: Vec<Texture>,
    meshes: Vec<GpuMesh>,
    materials: Vec<GpuMaterial>,
    white_texture: Texture,
    flat_normal_texture: Texture,
    shadow_map: Texture,
    environment: Option<EnvironmentState>,
    sky_visibility: Option<SkyVisibility>,
    targets: Option<Targets>,
    dynamic_buffers: Vec<DynamicBuffer>,
    frame_index: usize,
    in_flight: VecDeque<CommandBuffer>,
    last_gpu_ms: f64,
    drawable_size_px: (u32, u32),
    lens: Option<(Texture, gpu_types::GpuLensUniforms)>,
    lens_live: Option<LensLive>,
    hud_capture: Option<Texture>,
    is_capturing: bool,
}

/// `as_bytes` views a plain value as its bytes.
fn as_bytes<T: Copy>(value: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}

/// `slice_bytes` views a slice of plain values as its bytes.
fn slice_bytes<T: Copy>(values: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), size_of_val(values)) }
}

/// `set_vertex_bytes` binds a small value inline to a vertex buffer slot.
fn set_vertex_bytes(encoder: &Encoder, bytes: &[u8], index: usize) {
    let pointer: NonNull<c_void> = NonNull::from(bytes).cast();
    unsafe { encoder.setVertexBytes_length_atIndex(pointer, bytes.len(), index) };
}

/// `set_fragment_bytes` binds a small value inline to a fragment buffer slot.
fn set_fragment_bytes(encoder: &Encoder, bytes: &[u8], index: usize) {
    let pointer: NonNull<c_void> = NonNull::from(bytes).cast();
    unsafe { encoder.setFragmentBytes_length_atIndex(pointer, bytes.len(), index) };
}

/// `color_pass` describes a pass with one color target and, optionally, a depth target.
fn color_pass(
    color: &ProtocolObject<dyn MTLTexture>,
    color_load: MTLLoadAction,
    depth: Option<(
        &ProtocolObject<dyn MTLTexture>,
        MTLLoadAction,
        MTLStoreAction,
    )>,
) -> Retained<MTLRenderPassDescriptor> {
    let descriptor: Retained<MTLRenderPassDescriptor> = MTLRenderPassDescriptor::new();
    let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setTexture(Some(color));
    attachment.setLoadAction(color_load);
    attachment.setStoreAction(MTLStoreAction::Store);
    attachment.setClearColor(MTLClearColor {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    });
    if let Some((depth_texture, depth_load, depth_store)) = depth {
        let depth_attachment = descriptor.depthAttachment();
        depth_attachment.setTexture(Some(depth_texture));
        depth_attachment.setLoadAction(depth_load);
        depth_attachment.setStoreAction(depth_store);
        depth_attachment.setClearDepth(0.0);
    }
    descriptor
}

/// `draw_fullscreen` draws one full-screen triangle into a single target.
fn draw_fullscreen(
    command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
    target: &ProtocolObject<dyn MTLTexture>,
    pipeline: &ProtocolObject<dyn objc2_metal::MTLRenderPipelineState>,
    load: MTLLoadAction,
    bind: impl FnOnce(&Encoder),
) {
    let descriptor: Retained<MTLRenderPassDescriptor> = color_pass(target, load, None);
    let encoder = command_buffer
        .renderCommandEncoderWithDescriptor(&descriptor)
        .expect("render encoder creation failed");
    encoder.setRenderPipelineState(pipeline);
    bind(&encoder);
    unsafe {
        encoder.drawPrimitives_vertexStart_vertexCount(
            MTLPrimitiveType::Triangle,
            0,
            FULLSCREEN_VERTEX_COUNT,
        )
    };
    encoder.endEncoding();
}

/// `gpu_particles` converts particles to their GPU layout.
fn gpu_particles(particles: &[Particle]) -> Vec<GpuParticle> {
    particles
        .iter()
        .map(|particle: &Particle| GpuParticle {
            position_size: particle.position_m.extend(particle.size_m),
            stretch_kind: particle.stretch_m.extend(particle.kind as u32 as f32),
            color: particle.color_rgba,
            seed: Vec4::splat(particle.seed),
        })
        .collect()
}

impl MetalRenderer {
    /// `new` sets up Metal on a layer and compiles every pipeline.
    ///
    /// Example:
    ///     let renderer: MetalRenderer = MetalRenderer::new(layer);
    pub fn new(layer: Retained<CAMetalLayer>) -> MetalRenderer {
        let device: Retained<ProtocolObject<dyn MTLDevice>> =
            MTLCreateSystemDefaultDevice().expect("this Mac has no Metal device");
        let queue: Retained<ProtocolObject<dyn MTLCommandQueue>> = device
            .newCommandQueue()
            .expect("command queue creation failed");
        layer.setDevice(Some(&device));
        layer.setPixelFormat(DRAWABLE_FORMAT);
        layer.setFramebufferOnly(true);
        layer.setMaximumDrawableCount(FRAMES_IN_FLIGHT);
        let pipelines: Pipelines = Pipelines::new(&device);
        let pixel_texture = |format: MTLPixelFormat, bytes: &[u8], label: &str| {
            let texture: Texture = create_texture(
                &device,
                &TextureSpec {
                    usage: MTLTextureUsage::ShaderRead,
                    ..TextureSpec::render_target(format, 1, 1)
                },
                label,
            );
            upload_slices(&device, &queue, &texture, bytes, bytes.len(), 1);
            texture
        };
        let white_texture: Texture =
            pixel_texture(MTLPixelFormat::RGBA8Unorm, &[255, 255, 255, 255], "white");
        let flat_normal_texture: Texture =
            pixel_texture(MTLPixelFormat::RG8Unorm, &[128, 128], "flat_normal");
        let shadow_map: Texture = create_texture(
            &device,
            &TextureSpec {
                texture_type: MTLTextureType::Type2DArray,
                array_length: CASCADE_COUNT,
                ..TextureSpec::render_target(
                    DEPTH_FORMAT,
                    SHADOW_MAP_SIZE_PX as usize,
                    SHADOW_MAP_SIZE_PX as usize,
                )
            },
            "shadow_map",
        );
        let dynamic_buffers: Vec<DynamicBuffer> = (0..FRAMES_IN_FLIGHT)
            .map(|_| DynamicBuffer {
                buffer: device
                    .newBufferWithLength_options(
                        DYNAMIC_BUFFER_BYTES,
                        MTLResourceOptions::StorageModeShared
                            | MTLResourceOptions::CPUCacheModeWriteCombined,
                    )
                    .expect("dynamic buffer creation failed"),
                offset_bytes: 0,
            })
            .collect();
        MetalRenderer {
            device,
            queue,
            layer,
            pipelines,
            textures: Vec::new(),
            meshes: Vec::new(),
            materials: Vec::new(),
            white_texture,
            lens: None,
            lens_live: None,
            hud_capture: None,
            is_capturing: false,
            flat_normal_texture,
            shadow_map,
            environment: None,
            sky_visibility: None,
            targets: None,
            dynamic_buffers,
            frame_index: 0,
            in_flight: VecDeque::new(),
            last_gpu_ms: 0.0,
            drawable_size_px: (0, 0),
        }
    }

    /// `device_name` returns the GPU's name.
    ///
    /// Example:
    ///     println!("{}", renderer.device_name());
    pub fn device_name(&self) -> String {
        self.device.name().to_string()
    }

    /// `wait_for_frame_slot` blocks until fewer than `FRAMES_IN_FLIGHT` frames are on the GPU,
    /// recording the GPU time of the frame it waited for.
    fn wait_for_frame_slot(&mut self) {
        while self.in_flight.len() >= FRAMES_IN_FLIGHT {
            let command_buffer: CommandBuffer =
                self.in_flight.pop_front().expect("queue was not empty");
            command_buffer.waitUntilCompleted();
            let gpu_s: f64 = command_buffer.GPUEndTime() - command_buffer.GPUStartTime();
            if gpu_s > 0.0 {
                self.last_gpu_ms = gpu_s * SECONDS_TO_MS;
            }
        }
    }

    /// `read_texture_bytes` waits for the frames in flight and copies a 4-byte-per-pixel texture
    /// back: (width_px, height_px, bytes, rows top first).
    fn read_texture_bytes(&mut self, texture: &Texture) -> (u32, u32, Vec<u8>) {
        for command_buffer in self.in_flight.drain(..) {
            command_buffer.waitUntilCompleted();
        }
        let (width_px, height_px) = (texture.width(), texture.height());
        let bytes_per_row: usize = width_px * 4;
        let readback = self
            .device
            .newBufferWithLength_options(
                bytes_per_row * height_px,
                MTLResourceOptions::StorageModeShared,
            )
            .expect("readback buffer creation failed");
        let command_buffer = self
            .queue
            .commandBuffer()
            .expect("command buffer creation failed");
        let blit = command_buffer
            .blitCommandEncoder()
            .expect("blit encoder creation failed");
        unsafe {
            objc2_metal::MTLBlitCommandEncoder::copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(&*blit,
                texture,
                0,
                0,
                objc2_metal::MTLOrigin { x: 0, y: 0, z: 0 },
                objc2_metal::MTLSize { width: width_px, height: height_px, depth: 1 },
                &readback,
                0,
                bytes_per_row,
                bytes_per_row * height_px,
            );
        }
        blit.endEncoding();
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        let rgba: Vec<u8> = unsafe {
            std::slice::from_raw_parts(
                readback.contents().as_ptr().cast::<u8>(),
                bytes_per_row * height_px,
            )
        }
        .to_vec();
        (width_px as u32, height_px as u32, rgba)
    }

    /// `draw_meshes` encodes a list of mesh draws for one pass.
    #[allow(clippy::too_many_arguments)]
    fn draw_meshes(
        &self,
        encoder: &Encoder,
        pipeline_pair: &PipelinePair,
        draws: &[&DrawItem],
        pass: ScenePass,
        buffers: &FrameBuffers,
        is_viewmodel: bool,
    ) {
        let environment: &EnvironmentState = self.environment.as_ref().expect("no environment");
        for draw in draws {
            let mesh: &GpuMesh = &self.meshes[draw.mesh.0 as usize];
            let material: &GpuMaterial = &self.materials[draw.material.0 as usize];
            let skin_vertices: Option<&Buffer> = mesh
                .skin_vertices
                .as_ref()
                .filter(|_| draw.joint_offset.is_some());
            encoder.setRenderPipelineState(pipeline_pair.get(skin_vertices.is_some()));
            let uniforms: GpuDrawUniforms = GpuDrawUniforms {
                model: draw.model,
                tint: draw.tint,
                flags: UVec4::new(0, u32::from(is_viewmodel), 0, 0),
            };
            unsafe { encoder.setVertexBuffer_offset_atIndex(Some(&mesh.vertices), 0, 0) };
            set_vertex_bytes(encoder, as_bytes(&uniforms), 3);
            if let (Some(skin), Some(joint_offset)) = (skin_vertices, draw.joint_offset) {
                unsafe {
                    encoder.setVertexBuffer_offset_atIndex(Some(skin), 0, 4);
                    encoder.setVertexBuffer_offset_atIndex(
                        Some(buffers.dynamic),
                        buffers.joints_offset_bytes + joint_offset as usize * MATRIX_BYTES,
                        5,
                    );
                }
            }
            if pass != ScenePass::Prepass {
                set_fragment_bytes(encoder, as_bytes(&uniforms), 3);
                set_fragment_bytes(encoder, as_bytes(&material.uniforms), 4);
                for (index, texture) in material.textures.iter().enumerate() {
                    unsafe { encoder.setFragmentTexture_atIndex(Some(texture), index) };
                }
            }
            let _ = environment;
            unsafe {
                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                    MTLPrimitiveType::Triangle,
                    mesh.index_count,
                    MTLIndexType::UInt32,
                    &mesh.indices,
                    0,
                );
            }
        }
    }

    /// `encode_lens_live` draws the lens's live slots: the robots and lasers near the mouths that
    /// `pick_live_lens_mouths` picks, seen from each mouth's centre, into `live`. It returns
    /// `static_uniforms` with those slots and the robots' bounding spheres filled in.
    #[allow(clippy::too_many_arguments)]
    fn encode_lens_live(
        &self,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        frame: &FrameDescription,
        settings: &RenderSettings,
        targets: &Targets,
        cascades: &Cascades,
        buffers: &FrameBuffers,
        camera_frustum: &Frustum,
        (particles_offset_bytes, particle_count): (usize, usize),
        live: &LensLive,
        static_uniforms: &gpu_types::GpuLensUniforms,
    ) -> gpu_types::GpuLensUniforms {
        let mut uniforms: gpu_types::GpuLensUniforms = *static_uniforms;
        let is_visible: Vec<bool> = live
            .mouths
            .iter()
            .map(|mouth| camera_frustum.contains_sphere(mouth.centre_m, mouth.reach_m))
            .collect();
        let movers: Vec<(Vec3, Vec3)> = frame
            .lens_draws
            .iter()
            .map(|draw| (draw.bounds_center_m, draw.bounds_center_m))
            .chain(frame.lens_particles.iter().map(|particle| {
                let half_m: Vec3 = particle.stretch_m * 0.5;
                (particle.position_m - half_m, particle.position_m + half_m)
            }))
            .collect();
        let slots: Vec<usize> =
            pick_live_lens_mouths(&live.mouths, frame.camera.position_m, &is_visible, &movers);
        if slots.is_empty() {
            return uniforms;
        }
        let mut spheres: Vec<Vec4> = Vec::new();
        for draw in frame.lens_draws {
            let sphere: Vec4 = draw.bounds_center_m.extend(draw.bounds_radius_m);
            if !spheres.contains(&sphere) {
                spheres.push(sphere);
            }
        }
        let lanes_per_vec4: usize = 4;
        for (slot, &mouth) in slots.iter().enumerate() {
            let mouth_data: &LensMouth = &live.mouths[mouth];
            let is_mover_within_reach: bool = spheres.iter().any(|sphere| {
                sphere.truncate().distance(mouth_data.centre_m) < mouth_data.reach_m + sphere.w
            });
            let sign: f32 = if is_mover_within_reach { -1.0 } else { 1.0 };
            uniforms.live_slots[mouth / lanes_per_vec4][mouth % lanes_per_vec4] =
                sign * (slot + 1) as f32;
        }
        let get_slot_distance_m = |sphere: &Vec4| -> f32 {
            slots
                .iter()
                .map(|&mouth| live.mouths[mouth].centre_m.distance(sphere.truncate()))
                .fold(f32::MAX, f32::min)
        };
        spheres.sort_by(|a, b| get_slot_distance_m(a).total_cmp(&get_slot_distance_m(b)));
        spheres.truncate(MAX_LENS_MOVER_COUNT);
        uniforms.movers[..spheres.len()].copy_from_slice(&spheres);
        uniforms.live_params = Vec4::new(0.0, spheres.len() as f32, frame.camera.near_m, 0.0);

        let face_px: f32 = live.face_px as f32;
        let alpha_mode_of = |draw: &DrawItem| self.materials[draw.material.0 as usize].alpha_mode;
        for (slot, &mouth) in slots.iter().enumerate() {
            let descriptor: Retained<MTLRenderPassDescriptor> = color_pass(
                &live.color,
                MTLLoadAction::Clear,
                Some((&live.depth, MTLLoadAction::Clear, MTLStoreAction::Store)),
            );
            let color_attachment =
                unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
            color_attachment.setSlice(slot);
            color_attachment.setClearColor(MTLClearColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.0,
            });
            descriptor.depthAttachment().setSlice(slot);
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&descriptor)
                .expect("render encoder creation failed");
            encoder.setFrontFacingWinding(MTLWinding::CounterClockwise);
            let centre_m: Vec3 = live.mouths[mouth].centre_m;
            for (face, (forward, up)) in LENS_FACES.iter().enumerate() {
                let camera: crate::render::Camera = crate::render::Camera {
                    view: Mat4::look_to_rh(centre_m, *forward, *up),
                    position_m: centre_m,
                    fov_y_rad: std::f32::consts::FRAC_PI_2,
                    ..frame.camera
                };
                let proj: Mat4 = Mat4::perspective_infinite_reverse_rh(
                    std::f32::consts::FRAC_PI_2,
                    1.0,
                    camera.near_m,
                );
                let face_frame: FrameDescription = FrameDescription { camera, ..*frame };
                let mut face_uniforms: GpuFrameUniforms =
                    self.frame_uniforms(&face_frame, settings, targets, cascades, proj, proj);
                face_uniforms.screen_size =
                    Vec4::new(face_px, face_px, face_px.recip(), face_px.recip());
                face_uniforms.render_params.z = 0.0;
                face_uniforms.cascade_split_far_m = Vec4::new(
                    LENS_LIVE_UNUSED_SPLIT_M,
                    LENS_LIVE_UNUSED_SPLIT_M,
                    LENS_LIVE_UNUSED_SPLIT_M,
                    LENS_LIVE_LAST_SPLIT_M,
                );
                let face_frustum: Frustum = Frustum::from_view_proj(face_uniforms.view_proj);
                let draws: Vec<&DrawItem> = frame
                    .lens_draws
                    .iter()
                    .filter(|draw| {
                        face_frustum.contains_sphere(draw.bounds_center_m, draw.bounds_radius_m)
                    })
                    .collect();
                if draws.is_empty() && particle_count == 0 {
                    continue;
                }
                encoder.setViewport(MTLViewport {
                    originX: f64::from(face_px) * face as f64,
                    originY: 0.0,
                    width: f64::from(face_px),
                    height: f64::from(face_px),
                    znear: 0.0,
                    zfar: 1.0,
                });
                let face_buffers: FrameBuffers = FrameBuffers {
                    frame: &face_uniforms,
                    ..*buffers
                };
                self.bind_scene_inputs(&encoder, &face_buffers, targets);
                let face_pass: GpuPassUniforms = GpuPassUniforms {
                    view_proj: face_uniforms.view_proj,
                    flags: Vec4::ZERO,
                };
                set_vertex_bytes(&encoder, as_bytes(&face_pass), 2);
                encoder.setCullMode(MTLCullMode::Back);
                encoder
                    .setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_write));
                for (pipeline_pair, alpha_mode) in [
                    (&self.pipelines.forward_opaque, AlphaMode::Opaque),
                    (&self.pipelines.forward_masked, AlphaMode::Mask),
                ] {
                    let mode_draws: Vec<&DrawItem> = draws
                        .iter()
                        .copied()
                        .filter(|draw| alpha_mode_of(draw) == alpha_mode)
                        .collect();
                    self.draw_meshes(
                        &encoder,
                        pipeline_pair,
                        &mode_draws,
                        ScenePass::Forward,
                        &face_buffers,
                        false,
                    );
                }
                encoder.setCullMode(MTLCullMode::None);
                encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_read));
                let blended: Vec<&DrawItem> = draws
                    .iter()
                    .copied()
                    .filter(|draw| alpha_mode_of(draw) == AlphaMode::Blend)
                    .collect();
                self.draw_meshes(
                    &encoder,
                    &self.pipelines.forward_blend,
                    &blended,
                    ScenePass::Forward,
                    &face_buffers,
                    false,
                );
                if particle_count > 0 {
                    encoder.setRenderPipelineState(&self.pipelines.particle);
                    unsafe {
                        encoder.setVertexBuffer_offset_atIndex(
                            Some(buffers.dynamic),
                            particles_offset_bytes,
                            0,
                        );
                        encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                            MTLPrimitiveType::TriangleStrip,
                            0,
                            QUAD_VERTEX_COUNT,
                            particle_count,
                        );
                    }
                }
            }
            encoder.endEncoding();
        }
        uniforms
    }

    /// `bind_scene_inputs` binds the frame, the lights, and the lighting textures that every
    /// forward-shaded draw reads.
    fn bind_scene_inputs(&self, encoder: &Encoder, buffers: &FrameBuffers, targets: &Targets) {
        let environment: &EnvironmentState = self.environment.as_ref().expect("no environment");
        let sky_visibility: &SkyVisibility = self.sky_visibility.as_ref().expect("no occluders");
        set_vertex_bytes(encoder, as_bytes(buffers.frame), 1);
        set_fragment_bytes(encoder, as_bytes(buffers.frame), 1);
        set_fragment_bytes(encoder, slice_bytes(buffers.point_lights), 5);
        unsafe {
            encoder.setFragmentTexture_atIndex(Some(&self.shadow_map), 4);
            encoder.setFragmentTexture_atIndex(Some(&sky_visibility.volume), 5);
            encoder.setFragmentTexture_atIndex(Some(&environment.ibl.prefiltered), 6);
            encoder.setFragmentTexture_atIndex(Some(&environment.ibl.brdf_lut), 7);
            encoder.setFragmentTexture_atIndex(Some(&targets.ao), 8);
        }
    }

    /// `frame_uniforms` fills the per-frame uniforms.
    fn frame_uniforms(
        &self,
        frame: &FrameDescription,
        settings: &RenderSettings,
        targets: &Targets,
        cascades: &Cascades,
        proj: Mat4,
        viewmodel_proj: Mat4,
    ) -> GpuFrameUniforms {
        let environment: &EnvironmentState = self.environment.as_ref().expect("no environment");
        let sky_visibility: &SkyVisibility = self.sky_visibility.as_ref().expect("no occluders");
        let atmosphere: &Atmosphere = &environment.atmosphere;
        let view: Mat4 = frame.camera.view;
        let view_proj: Mat4 = proj * view;
        let width_px: f32 = targets.width_px as f32;
        let height_px: f32 = targets.height_px as f32;
        GpuFrameUniforms {
            view_proj,
            view,
            proj,
            inv_view_proj: view_proj.inverse(),
            inv_proj: proj.inverse(),
            viewmodel_view_proj: viewmodel_proj * view,
            cascade_view_proj: cascades.view_proj,
            cascade_split_far_m: Vec4::from_array(cascades.split_far_m),
            cascade_texel_size_m: Vec4::from_array(cascades.texel_size_m),
            camera_position_m: frame.camera.position_m.extend(frame.time_s),
            sun_direction: environment.sun_direction,
            sun_illuminance: environment.sun_illuminance,
            sh9: environment.sh9,
            fog_color: atmosphere
                .fog_color_rgb
                .extend(atmosphere.fog_density_per_m),
            fog_params: Vec4::new(
                atmosphere.fog_height_falloff_per_m,
                atmosphere.fog_base_height_m,
                atmosphere.fog_sun_inscatter_frac,
                atmosphere.fog_max_opacity_frac,
            ),
            sky_visibility_origin_m: sky_visibility.origin_m.extend(0.0),
            sky_visibility_inv_size_m: sky_visibility.size_m.recip().extend(0.0),
            screen_size: Vec4::new(width_px, height_px, 1.0 / width_px, 1.0 / height_px),
            render_params: Vec4::new(
                atmosphere.exposure,
                if settings.is_bloom_enabled {
                    atmosphere.bloom_mix_frac
                } else {
                    0.0
                },
                if settings.is_ssao_enabled { 1.0 } else { 0.0 },
                PREFILTERED_MIP_COUNT as f32,
            ),
            post_params: Vec4::new(
                0.0,
                1.0 / SHADOW_MAP_SIZE_PX as f32,
                atmosphere.ibl_intensity_frac,
                frame.point_lights.len().min(MAX_POINT_LIGHT_COUNT) as f32,
            ),
            bounce_color: atmosphere.bounce_color_rgb.extend(0.0),
            effect_params: Vec4::new(
                if settings.is_color_grade_enabled {
                    1.0
                } else {
                    0.0
                },
                if settings.is_volumetric_haze_enabled {
                    1.0
                } else {
                    0.0
                },
                if settings.is_material_response_enabled {
                    1.0
                } else {
                    0.0
                },
                BLOOM_LEVEL_COUNT as f32,
            ),
        }
    }

    /// `ensure_targets` recreates the render targets when the internal resolution changes.
    fn ensure_targets(&mut self, render_scale_frac: f32) {
        let (drawable_width_px, drawable_height_px) = self.drawable_size_px;
        let width_px: usize =
            ((drawable_width_px as f32 * render_scale_frac).round() as usize).max(1);
        let height_px: usize =
            ((drawable_height_px as f32 * render_scale_frac).round() as usize).max(1);
        let is_current: bool = self.targets.as_ref().is_some_and(|targets: &Targets| {
            targets.width_px == width_px && targets.height_px == height_px
        });
        if !is_current {
            self.targets = Some(Targets::new(&self.device, width_px, height_px));
        }
    }
}

impl Renderer for MetalRenderer {
    fn set_wormhole_lens(&mut self, lens: Option<&crate::render::WormholeLens>) {
        self.lens = lens.map(|lens| {
            let mouth_count: usize = lens.mouths.len();
            assert!(
                mouth_count <= crate::render::MAX_LENS_MOUTH_COUNT,
                "the lens takes at most {} mouths, not {mouth_count}",
                crate::render::MAX_LENS_MOUTH_COUNT
            );
            let mut uniforms: gpu_types::GpuLensUniforms = gpu_types::GpuLensUniforms::default();
            for (index, mouth) in lens.mouths.iter().enumerate() {
                uniforms.centres[index] = mouth.centre_m.extend(mouth.lens_throat_m);
                uniforms.shapes[index] = Vec4::new(
                    mouth.reach_m,
                    mouth.lambda_squared,
                    mouth.partner as f32,
                    mouth.floor_m,
                );
            }
            uniforms.params = Vec4::new(mouth_count as f32, 0.0, mouth_count as f32, 0.0);
            (
                upload_texture_2d(&self.device, &self.queue, &lens.atlas),
                uniforms,
            )
        });
        self.lens_live = lens.map(|lens| {
            let face_px: usize = lens.atlas.width as usize / LENS_FACES.len();
            let slot_spec = |format: MTLPixelFormat| -> TextureSpec {
                TextureSpec {
                    texture_type: MTLTextureType::Type2DArray,
                    array_length: MAX_LIVE_LENS_SLOT_COUNT,
                    ..TextureSpec::render_target(format, face_px * LENS_FACES.len(), face_px)
                }
            };
            LensLive {
                color: create_texture(&self.device, &slot_spec(HDR_FORMAT), "live lens color"),
                depth: create_texture(&self.device, &slot_spec(DEPTH_FORMAT), "live lens depth"),
                face_px,
                mouths: lens.mouths.clone(),
            }
        });
    }

    fn set_is_capturing_hud(&mut self, is_capturing_hud: bool) {
        let (width_px, height_px) = self.drawable_size_px;
        self.hud_capture = is_capturing_hud.then(|| {
            create_texture(
                &self.device,
                &TextureSpec::render_target(DRAWABLE_FORMAT, width_px as usize, height_px as usize),
                "hud capture",
            )
        });
    }

    fn read_hud_frame_rgba(&mut self) -> (u32, u32, Vec<u8>) {
        let capture: Texture = self.hud_capture.clone().expect("the HUD is being captured");
        let (width_px, height_px, mut bgra) = self.read_texture_bytes(&capture);
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        (width_px, height_px, bgra)
    }

    fn set_is_capturing(&mut self, is_capturing: bool) {
        self.is_capturing = is_capturing;
    }

    fn read_frame_rgba(&mut self) -> (u32, u32, Vec<u8>) {
        let ldr: Texture = self
            .targets
            .as_ref()
            .expect("a frame was rendered")
            .ldr
            .clone();
        self.read_texture_bytes(&ldr)
    }

    fn read_frame_depth(&mut self) -> (u32, u32, Vec<f32>) {
        let depth: Texture = self
            .targets
            .as_ref()
            .expect("a frame was rendered")
            .depth
            .clone();
        let (width_px, height_px, bytes) = self.read_texture_bytes(&depth);
        let depths: Vec<f32> = bytes
            .as_chunks::<{ size_of::<f32>() }>()
            .0
            .iter()
            .map(|chunk: &[u8; 4]| f32::from_ne_bytes(*chunk))
            .collect();
        (width_px, height_px, depths)
    }

    fn create_texture(&mut self, texture: &TextureData) -> TextureHandle {
        self.textures
            .push(upload_texture_2d(&self.device, &self.queue, texture));
        TextureHandle(self.textures.len() as u32 - 1)
    }

    fn create_mesh(&mut self, mesh: &MeshData) -> MeshHandle {
        self.meshes.push(GpuMesh {
            vertices: create_buffer(&self.device, &mesh.vertices),
            skin_vertices: mesh
                .skin_vertices
                .as_ref()
                .map(|skin| create_buffer(&self.device, skin)),
            indices: create_buffer(&self.device, &mesh.indices),
            index_count: mesh.indices.len(),
        });
        MeshHandle(self.meshes.len() as u32 - 1)
    }

    fn create_material(&mut self, material: &MaterialData) -> MaterialHandle {
        let texture_or = |handle: Option<TextureHandle>, fallback: &Texture| -> Texture {
            handle
                .map(|texture_handle: TextureHandle| {
                    self.textures[texture_handle.0 as usize].clone()
                })
                .unwrap_or_else(|| fallback.clone())
        };
        let alpha_cutoff: f32 = match material.alpha_mode {
            AlphaMode::Mask => 0.5,
            AlphaMode::Opaque | AlphaMode::Blend => 0.0,
        };
        self.materials.push(GpuMaterial {
            textures: [
                texture_or(material.base_color_texture, &self.white_texture),
                texture_or(material.normal_texture, &self.flat_normal_texture),
                texture_or(
                    material.occlusion_roughness_metal_texture,
                    &self.white_texture,
                ),
                texture_or(material.emissive_texture, &self.white_texture),
            ],
            uniforms: GpuMaterialUniforms {
                base_color_factor: material.base_color_factor,
                emissive_factor: material.emissive_factor.extend(0.0),
                params: Vec4::new(
                    material.roughness_factor,
                    material.metallic_factor,
                    alpha_cutoff,
                    if material.has_occlusion_in_red {
                        1.0
                    } else {
                        0.0
                    },
                ),
            },
            alpha_mode: material.alpha_mode,
        });
        MaterialHandle(self.materials.len() as u32 - 1)
    }

    fn set_environment(&mut self, environment: &EnvironmentData) {
        let ibl: IblTextures = build_ibl(&self.device, &self.queue, &self.pipelines, environment);
        let mut sh9: [Vec4; 9] = [Vec4::ZERO; 9];
        for (coefficient, value) in sh9.iter_mut().zip(environment.irradiance_sh9.iter()) {
            *coefficient = value.extend(0.0);
        }
        self.environment = Some(EnvironmentState {
            ibl,
            sh9,
            sun_direction: environment
                .sun_direction
                .normalize()
                .extend(environment.sun_angular_radius_rad),
            sun_illuminance: environment.sun_illuminance_rgb.extend(0.0),
            atmosphere: environment.atmosphere,
        });
    }

    fn set_occluders(&mut self, occluders: &[OccluderBox], bounds_min_m: Vec3, bounds_max_m: Vec3) {
        self.sky_visibility = Some(bake_sky_visibility(
            &self.device,
            &self.queue,
            &self.pipelines,
            occluders,
            bounds_min_m,
            bounds_max_m,
        ));
    }

    fn resize(&mut self, width_px: u32, height_px: u32) {
        if self.drawable_size_px != (width_px, height_px) {
            self.drawable_size_px = (width_px, height_px);
            self.layer.setDrawableSize(CGSize {
                width: f64::from(width_px),
                height: f64::from(height_px),
            });
        }
    }

    fn render_frame(&mut self, frame: &FrameDescription, settings: &RenderSettings) -> FrameTiming {
        let start: std::time::Instant = std::time::Instant::now();
        self.wait_for_frame_slot();
        let slot_wait_ms: f64 = start.elapsed().as_secs_f64() * SECONDS_TO_MS;
        self.layer.setDisplaySyncEnabled(settings.is_vsync_enabled);
        self.ensure_targets(settings.render_scale_frac.clamp(0.25, 2.0));
        let targets: Targets = self.targets.take().expect("targets were just created");
        let timing: FrameTiming = self.encode_frame(frame, settings, &targets);
        self.targets = Some(targets);
        FrameTiming {
            slot_wait_ms,
            cpu_render_ms: start.elapsed().as_secs_f64() * SECONDS_TO_MS,
            ..timing
        }
    }
}

impl MetalRenderer {
    /// `encode_frame` encodes and submits every pass of one frame.
    fn encode_frame(
        &mut self,
        frame: &FrameDescription,
        settings: &RenderSettings,
        targets: &Targets,
    ) -> FrameTiming {
        let environment: &EnvironmentState = self.environment.as_ref().expect("no environment");
        let aspect: f32 = targets.width_px as f32 / targets.height_px as f32;
        let proj: Mat4 = Mat4::perspective_infinite_reverse_rh(
            frame.camera.fov_y_rad,
            aspect,
            frame.camera.near_m,
        );
        let viewmodel_proj: Mat4 = Mat4::perspective_infinite_reverse_rh(
            frame.camera.viewmodel_fov_y_rad,
            aspect,
            VIEWMODEL_NEAR_M,
        );
        let cascades: Cascades = fit_cascades(
            &CascadeCamera {
                inverse_view: frame.camera.view.inverse(),
                fov_y_rad: frame.camera.fov_y_rad,
                aspect,
            },
            environment.sun_direction.truncate(),
            settings.shadow_distance_m.min(frame.camera.far_m),
            SHADOW_MAP_SIZE_PX,
        );
        let frame_uniforms: GpuFrameUniforms =
            self.frame_uniforms(frame, settings, targets, &cascades, proj, viewmodel_proj);
        let mut point_lights: Vec<GpuPointLight> = frame
            .point_lights
            .iter()
            .take(MAX_POINT_LIGHT_COUNT)
            .map(|light| GpuPointLight {
                position_radius: light.position_m.extend(light.radius_m),
                color: light.color_rgb.extend(0.0),
            })
            .collect();
        if point_lights.is_empty() {
            point_lights.push(GpuPointLight::default());
        }

        let slot: usize = self.frame_index % FRAMES_IN_FLIGHT;
        self.frame_index += 1;
        let dynamic: &mut DynamicBuffer = &mut self.dynamic_buffers[slot];
        dynamic.offset_bytes = 0;
        let joints_offset_bytes: usize = dynamic.push(frame.joint_matrices);
        let decals: Vec<GpuDecal> = frame
            .decals
            .iter()
            .map(|decal| GpuDecal {
                position_size: decal.position_m.extend(decal.size_m),
                normal_seed: decal.normal.extend(decal.seed),
                tangent: decal.tangent.extend(0.0),
            })
            .collect();
        let decals_offset_bytes: usize = dynamic.push(&decals);
        let world_particles: Vec<GpuParticle> = gpu_particles(frame.world_particles);
        let world_particles_offset_bytes: usize = dynamic.push(&world_particles);
        let viewmodel_particles: Vec<GpuParticle> = gpu_particles(frame.viewmodel_particles);
        let viewmodel_particles_offset_bytes: usize = dynamic.push(&viewmodel_particles);
        let lens_particles: Vec<GpuParticle> = gpu_particles(frame.lens_particles);
        let lens_particles_offset_bytes: usize = dynamic.push(&lens_particles);
        let hud_rects: Vec<GpuHudRect> = frame
            .hud_rects
            .iter()
            .map(|rect| GpuHudRect {
                rect_px: Vec4::new(rect.x_px, rect.y_px, rect.width_px, rect.height_px),
                color: rect.color_rgba,
            })
            .collect();
        let hud_offset_bytes: usize = dynamic.push(&hud_rects);
        let overlay_offset_bytes: usize = dynamic.push(frame.overlay_rects);
        let hud_images: Vec<(GpuHudImage, usize)> = frame
            .hud_images
            .iter()
            .map(|image| {
                let gpu_image: GpuHudImage = GpuHudImage {
                    rect_px: [
                        image.x_px,
                        image.y_px,
                        image.width_px as f32 * image.scale,
                        image.height_px as f32 * image.scale,
                    ],
                    size_px: [image.width_px, image.height_px],
                    padding: [0, 0],
                };
                (gpu_image, dynamic.push(image.pixels_rgba))
            })
            .collect();
        let dynamic_buffer: Buffer = dynamic.buffer.clone();
        let buffers: FrameBuffers = FrameBuffers {
            dynamic: &dynamic_buffer,
            joints_offset_bytes,
            frame: &frame_uniforms,
            point_lights: &point_lights,
        };

        let command_buffer: CommandBuffer = self
            .queue
            .commandBuffer()
            .expect("command buffer creation failed");
        self.encode_shadows(&command_buffer, frame, &cascades, &buffers);

        let camera_frustum: Frustum = Frustum::from_view_proj(frame_uniforms.view_proj);
        let visible: Vec<&DrawItem> = frame
            .world_draws
            .iter()
            .filter(|draw| {
                camera_frustum.contains_sphere(draw.bounds_center_m, draw.bounds_radius_m)
            })
            .collect();
        let alpha_mode_of = |draw: &DrawItem| self.materials[draw.material.0 as usize].alpha_mode;
        let opaque: Vec<&DrawItem> = visible
            .iter()
            .copied()
            .filter(|draw| alpha_mode_of(draw) == AlphaMode::Opaque)
            .collect();
        let masked: Vec<&DrawItem> = visible
            .iter()
            .copied()
            .filter(|draw| alpha_mode_of(draw) == AlphaMode::Mask)
            .collect();
        let camera_position_m: Vec3 = frame.camera.position_m;
        let sort_back_to_front = |draws: &mut Vec<&DrawItem>| {
            draws.sort_by(|a, b| {
                let distance_a: f32 = a.bounds_center_m.distance_squared(camera_position_m);
                let distance_b: f32 = b.bounds_center_m.distance_squared(camera_position_m);
                distance_b.total_cmp(&distance_a)
            });
        };
        let mut blended: Vec<&DrawItem> = visible
            .iter()
            .copied()
            .filter(|draw| alpha_mode_of(draw) == AlphaMode::Blend)
            .collect();
        sort_back_to_front(&mut blended);

        let world_pass: GpuPassUniforms = GpuPassUniforms {
            view_proj: frame_uniforms.view_proj,
            flags: Vec4::ZERO,
        };
        {
            let descriptor: Retained<MTLRenderPassDescriptor> = color_pass(
                &targets.normals,
                MTLLoadAction::Clear,
                Some((&targets.depth, MTLLoadAction::Clear, MTLStoreAction::Store)),
            );
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&descriptor)
                .expect("render encoder creation failed");
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_write));
            encoder.setCullMode(MTLCullMode::Back);
            encoder.setFrontFacingWinding(MTLWinding::CounterClockwise);
            set_vertex_bytes(&encoder, as_bytes(&frame_uniforms), 1);
            set_fragment_bytes(&encoder, as_bytes(&frame_uniforms), 1);
            set_vertex_bytes(&encoder, as_bytes(&world_pass), 2);
            self.draw_meshes(
                &encoder,
                &self.pipelines.prepass,
                &opaque,
                ScenePass::Prepass,
                &buffers,
                false,
            );
            encoder.endEncoding();
        }

        if settings.is_ssao_enabled {
            draw_fullscreen(
                &command_buffer,
                &targets.ao,
                &self.pipelines.ssao,
                MTLLoadAction::DontCare,
                |encoder: &Encoder| {
                    set_fragment_bytes(encoder, as_bytes(&frame_uniforms), 1);
                    unsafe {
                        encoder.setFragmentTexture_atIndex(Some(&targets.depth), 0);
                        encoder.setFragmentTexture_atIndex(Some(&targets.normals), 1);
                    }
                },
            );
            for (source, destination, direction) in [
                (&targets.ao, &targets.ao_scratch, Vec2::X),
                (&targets.ao_scratch, &targets.ao, Vec2::Y),
            ] {
                draw_fullscreen(
                    &command_buffer,
                    destination,
                    &self.pipelines.ssao_blur,
                    MTLLoadAction::DontCare,
                    |encoder: &Encoder| {
                        set_fragment_bytes(encoder, as_bytes(&frame_uniforms), 1);
                        set_fragment_bytes(encoder, as_bytes(&direction), 0);
                        unsafe {
                            encoder.setFragmentTexture_atIndex(Some(source), 0);
                            encoder.setFragmentTexture_atIndex(Some(&targets.depth), 1);
                        }
                    },
                );
            }
        }

        {
            let descriptor: Retained<MTLRenderPassDescriptor> = color_pass(
                &targets.hdr,
                MTLLoadAction::Clear,
                Some((
                    &targets.depth,
                    MTLLoadAction::Load,
                    MTLStoreAction::DontCare,
                )),
            );
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&descriptor)
                .expect("render encoder creation failed");
            encoder.setCullMode(MTLCullMode::Back);
            encoder.setFrontFacingWinding(MTLWinding::CounterClockwise);
            self.bind_scene_inputs(&encoder, &buffers, targets);
            set_vertex_bytes(&encoder, as_bytes(&world_pass), 2);
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_write));
            for (pipeline_pair, draws) in [
                (&self.pipelines.forward_opaque, &opaque),
                (&self.pipelines.forward_masked, &masked),
            ] {
                self.draw_meshes(
                    &encoder,
                    pipeline_pair,
                    draws,
                    ScenePass::Forward,
                    &buffers,
                    false,
                );
            }

            encoder.setCullMode(MTLCullMode::None);
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_read));
            encoder.setRenderPipelineState(&self.pipelines.sky);
            unsafe {
                encoder.setFragmentTexture_atIndex(Some(&environment.ibl.environment), 0);
                encoder.drawPrimitives_vertexStart_vertexCount(
                    MTLPrimitiveType::Triangle,
                    0,
                    FULLSCREEN_VERTEX_COUNT,
                );
            }

            if !decals.is_empty() {
                encoder.setRenderPipelineState(&self.pipelines.decal);
                unsafe {
                    encoder.setVertexBuffer_offset_atIndex(
                        Some(&dynamic_buffer),
                        decals_offset_bytes,
                        0,
                    );
                    encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                        MTLPrimitiveType::TriangleStrip,
                        0,
                        QUAD_VERTEX_COUNT,
                        decals.len(),
                    );
                }
            }
            self.draw_meshes(
                &encoder,
                &self.pipelines.forward_blend,
                &blended,
                ScenePass::Forward,
                &buffers,
                false,
            );
            if !world_particles.is_empty() {
                encoder.setRenderPipelineState(&self.pipelines.particle);
                unsafe {
                    encoder.setVertexBuffer_offset_atIndex(
                        Some(&dynamic_buffer),
                        world_particles_offset_bytes,
                        0,
                    );
                    encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                        MTLPrimitiveType::TriangleStrip,
                        0,
                        QUAD_VERTEX_COUNT,
                        world_particles.len(),
                    );
                }
            }
            encoder.endEncoding();
        }

        if !frame.viewmodel_draws.is_empty() || !viewmodel_particles.is_empty() {
            let descriptor: Retained<MTLRenderPassDescriptor> = color_pass(
                &targets.hdr,
                MTLLoadAction::Load,
                Some((
                    &targets.depth,
                    MTLLoadAction::Clear,
                    MTLStoreAction::DontCare,
                )),
            );
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&descriptor)
                .expect("render encoder creation failed");
            encoder.setCullMode(MTLCullMode::Back);
            encoder.setFrontFacingWinding(MTLWinding::CounterClockwise);
            self.bind_scene_inputs(&encoder, &buffers, targets);
            let viewmodel_pass: GpuPassUniforms = GpuPassUniforms {
                view_proj: frame_uniforms.viewmodel_view_proj,
                flags: Vec4::ZERO,
            };
            set_vertex_bytes(&encoder, as_bytes(&viewmodel_pass), 2);
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_write));
            let (viewmodel_masked, viewmodel_opaque): (Vec<&DrawItem>, Vec<&DrawItem>) = frame
                .viewmodel_draws
                .iter()
                .filter(|draw| alpha_mode_of(draw) != AlphaMode::Blend)
                .partition(|draw| alpha_mode_of(draw) == AlphaMode::Mask);
            let mut viewmodel_blended: Vec<&DrawItem> = frame
                .viewmodel_draws
                .iter()
                .filter(|draw| alpha_mode_of(draw) == AlphaMode::Blend)
                .collect();
            sort_back_to_front(&mut viewmodel_blended);
            for (pipeline_pair, draws) in [
                (&self.pipelines.forward_opaque, &viewmodel_opaque),
                (&self.pipelines.forward_masked, &viewmodel_masked),
            ] {
                self.draw_meshes(
                    &encoder,
                    pipeline_pair,
                    draws,
                    ScenePass::Forward,
                    &buffers,
                    true,
                );
            }
            encoder.setCullMode(MTLCullMode::None);
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_read));
            self.draw_meshes(
                &encoder,
                &self.pipelines.forward_blend,
                &viewmodel_blended,
                ScenePass::Forward,
                &buffers,
                true,
            );
            if !viewmodel_particles.is_empty() {
                encoder.setRenderPipelineState(&self.pipelines.particle);
                unsafe {
                    encoder.setVertexBuffer_offset_atIndex(
                        Some(&dynamic_buffer),
                        viewmodel_particles_offset_bytes,
                        0,
                    );
                    encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                        MTLPrimitiveType::TriangleStrip,
                        0,
                        QUAD_VERTEX_COUNT,
                        viewmodel_particles.len(),
                    );
                }
            }
            encoder.endEncoding();
        }

        if settings.is_bloom_enabled {
            for level in 0..BLOOM_LEVEL_COUNT {
                let source: &Texture = if level == 0 {
                    &targets.hdr
                } else {
                    &targets.bloom_levels[level - 1]
                };
                let is_first_level: f32 = if level == 0 { 1.0 } else { 0.0 };
                draw_fullscreen(
                    &command_buffer,
                    &targets.bloom_levels[level],
                    &self.pipelines.bloom_downsample,
                    MTLLoadAction::DontCare,
                    |encoder: &Encoder| {
                        set_fragment_bytes(encoder, as_bytes(&is_first_level), 0);
                        unsafe { encoder.setFragmentTexture_atIndex(Some(source), 0) };
                    },
                );
            }
            for level in (0..BLOOM_LEVEL_COUNT - 1).rev() {
                let source: &Texture = &targets.bloom_levels[level + 1];
                draw_fullscreen(
                    &command_buffer,
                    &targets.bloom_levels[level],
                    &self.pipelines.bloom_upsample,
                    MTLLoadAction::Load,
                    |encoder: &Encoder| unsafe {
                        encoder.setFragmentTexture_atIndex(Some(source), 0)
                    },
                );
            }
        }
        let bloom_source: &Texture = if settings.is_bloom_enabled {
            &targets.bloom_levels[0]
        } else {
            &targets.hdr
        };
        if let Some((lens_atlas, lens_uniforms)) = &self.lens {
            let live: &LensLive = self.lens_live.as_ref().expect("a lens has live slots");
            let frame_lens_uniforms: gpu_types::GpuLensUniforms = self.encode_lens_live(
                &command_buffer,
                frame,
                settings,
                targets,
                &cascades,
                &buffers,
                &camera_frustum,
                (lens_particles_offset_bytes, lens_particles.len()),
                live,
                lens_uniforms,
            );
            draw_fullscreen(
                &command_buffer,
                &targets.lens,
                &self.pipelines.lens,
                MTLLoadAction::DontCare,
                |encoder: &Encoder| {
                    set_fragment_bytes(encoder, as_bytes(&frame_uniforms), 1);
                    set_fragment_bytes(encoder, as_bytes(&frame_lens_uniforms), 2);
                    unsafe {
                        encoder.setFragmentTexture_atIndex(Some(lens_atlas), 0);
                        encoder.setFragmentTexture_atIndex(Some(&live.color), 1);
                        encoder.setFragmentTexture_atIndex(Some(&live.depth), 2);
                    }
                },
            );
        }
        draw_fullscreen(
            &command_buffer,
            &targets.ldr,
            &self.pipelines.tonemap,
            MTLLoadAction::DontCare,
            |encoder: &Encoder| {
                set_fragment_bytes(encoder, as_bytes(&frame_uniforms), 1);
                let mut lens_uniforms: gpu_types::GpuLensUniforms = self
                    .lens
                    .as_ref()
                    .map_or_else(gpu_types::GpuLensUniforms::default, |(_, uniforms)| {
                        *uniforms
                    });
                lens_uniforms.params.y = if self.is_capturing { 1.0 } else { 0.0 };
                set_fragment_bytes(encoder, as_bytes(&lens_uniforms), 2);
                unsafe {
                    encoder.setFragmentTexture_atIndex(Some(&targets.hdr), 0);
                    encoder.setFragmentTexture_atIndex(Some(bloom_source), 1);
                    encoder.setFragmentTexture_atIndex(Some(&targets.depth), 2);
                    encoder.setFragmentTexture_atIndex(Some(&targets.lens), 3);
                }
            },
        );

        let drawable_start: std::time::Instant = std::time::Instant::now();
        let drawable = match &self.hud_capture {
            Some(_) => None,
            None => self.layer.nextDrawable(),
        };
        let drawable_wait_ms: f64 = drawable_start.elapsed().as_secs_f64() * SECONDS_TO_MS;
        let output_texture: Option<Retained<ProtocolObject<dyn MTLTexture>>> =
            match (&self.hud_capture, &drawable) {
                (Some(capture), _) => Some(capture.clone()),
                (None, Some(drawable)) => Some(drawable.texture()),
                (None, None) => None,
            };
        if let Some(drawable_texture) = output_texture {
            let descriptor: Retained<MTLRenderPassDescriptor> =
                color_pass(&drawable_texture, MTLLoadAction::DontCare, None);
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&descriptor)
                .expect("render encoder creation failed");
            encoder.setRenderPipelineState(&self.pipelines.fxaa);
            let is_fxaa_enabled: f32 = if settings.is_fxaa_enabled { 1.0 } else { 0.0 };
            set_fragment_bytes(&encoder, as_bytes(&is_fxaa_enabled), 0);
            set_fragment_bytes(&encoder, as_bytes(&frame.hit_effect), 1);
            let output_size_px: Vec2 = Vec2::new(
                drawable_texture.width() as f32,
                drawable_texture.height() as f32,
            );
            set_fragment_bytes(&encoder, as_bytes(&output_size_px), 2);
            unsafe {
                encoder.setFragmentTexture_atIndex(Some(&targets.ldr), 0);
                encoder.drawPrimitives_vertexStart_vertexCount(
                    MTLPrimitiveType::Triangle,
                    0,
                    FULLSCREEN_VERTEX_COUNT,
                );
            }
            if !hud_rects.is_empty() {
                let window_size_px: Vec2 = Vec2::new(
                    drawable_texture.width() as f32,
                    drawable_texture.height() as f32,
                );
                encoder.setRenderPipelineState(&self.pipelines.hud);
                set_vertex_bytes(&encoder, as_bytes(&window_size_px), 1);
                unsafe {
                    encoder.setVertexBuffer_offset_atIndex(
                        Some(&dynamic_buffer),
                        hud_offset_bytes,
                        0,
                    );
                    encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                        MTLPrimitiveType::TriangleStrip,
                        0,
                        QUAD_VERTEX_COUNT,
                        hud_rects.len(),
                    );
                }
            }
            if !frame.overlay_rects.is_empty() {
                let window_size_px: Vec2 = Vec2::new(
                    drawable_texture.width() as f32,
                    drawable_texture.height() as f32,
                );
                let rect_count: u32 = frame.overlay_rects.len() as u32;
                encoder.setRenderPipelineState(&self.pipelines.overlay_rects);
                set_fragment_bytes(&encoder, as_bytes(&rect_count), 1);
                set_fragment_bytes(&encoder, as_bytes(&window_size_px), 2);
                unsafe {
                    encoder.setFragmentBuffer_offset_atIndex(
                        Some(&dynamic_buffer),
                        overlay_offset_bytes,
                        0,
                    );
                    encoder.drawPrimitives_vertexStart_vertexCount(
                        MTLPrimitiveType::Triangle,
                        0,
                        FULLSCREEN_VERTEX_COUNT,
                    );
                }
            }
            if !hud_images.is_empty() {
                let window_size_px: Vec2 = Vec2::new(
                    drawable_texture.width() as f32,
                    drawable_texture.height() as f32,
                );
                encoder.setRenderPipelineState(&self.pipelines.hud_image);
                set_vertex_bytes(&encoder, as_bytes(&window_size_px), 1);
                for (gpu_image, offset_bytes) in &hud_images {
                    set_vertex_bytes(&encoder, as_bytes(gpu_image), 0);
                    set_fragment_bytes(&encoder, as_bytes(gpu_image), 0);
                    unsafe {
                        encoder.setFragmentBuffer_offset_atIndex(
                            Some(&dynamic_buffer),
                            *offset_bytes,
                            1,
                        );
                        encoder.drawPrimitives_vertexStart_vertexCount(
                            MTLPrimitiveType::TriangleStrip,
                            0,
                            QUAD_VERTEX_COUNT,
                        );
                    }
                }
            }
            encoder.endEncoding();
            if let Some(drawable) = &drawable {
                let metal_drawable: &ProtocolObject<dyn MTLDrawable> =
                    ProtocolObject::from_ref(&**drawable);
                command_buffer.presentDrawable(metal_drawable);
            }
        }
        command_buffer.commit();
        self.in_flight.push_back(command_buffer);
        FrameTiming {
            gpu_ms: self.last_gpu_ms,
            drawable_wait_ms,
            ..FrameTiming::default()
        }
    }

    /// `encode_shadows` renders the shadow casters into each cascade.
    fn encode_shadows(
        &self,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        frame: &FrameDescription,
        cascades: &Cascades,
        buffers: &FrameBuffers,
    ) {
        for cascade in 0..CASCADE_COUNT {
            let descriptor: Retained<MTLRenderPassDescriptor> = MTLRenderPassDescriptor::new();
            let depth_attachment = descriptor.depthAttachment();
            depth_attachment.setTexture(Some(&self.shadow_map));
            depth_attachment.setSlice(cascade);
            depth_attachment.setLoadAction(MTLLoadAction::Clear);
            depth_attachment.setStoreAction(MTLStoreAction::Store);
            depth_attachment.setClearDepth(1.0);
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&descriptor)
                .expect("render encoder creation failed");
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.less_write));
            encoder.setCullMode(MTLCullMode::None);
            encoder.setDepthBias_slopeScale_clamp(SHADOW_DEPTH_BIAS, SHADOW_SLOPE_BIAS, 0.0);
            let pass: GpuPassUniforms = GpuPassUniforms {
                view_proj: cascades.view_proj[cascade],
                flags: Vec4::ZERO,
            };
            set_vertex_bytes(&encoder, as_bytes(buffers.frame), 1);
            set_vertex_bytes(&encoder, as_bytes(&pass), 2);
            let frustum: Frustum = Frustum::from_view_proj(cascades.view_proj[cascade]);
            let (masked_casters, opaque_casters): (Vec<&DrawItem>, Vec<&DrawItem>) = frame
                .world_draws
                .iter()
                .filter(|draw| {
                    draw.does_cast_shadow
                        && self.materials[draw.material.0 as usize].alpha_mode != AlphaMode::Blend
                        && frustum.contains_sphere(draw.bounds_center_m, draw.bounds_radius_m)
                })
                .partition(|draw| {
                    self.materials[draw.material.0 as usize].alpha_mode == AlphaMode::Mask
                });
            for (pipeline_pair, casters) in [
                (&self.pipelines.shadow, &opaque_casters),
                (&self.pipelines.shadow_masked, &masked_casters),
            ] {
                self.draw_meshes(
                    &encoder,
                    pipeline_pair,
                    casters,
                    ScenePass::Shadow,
                    buffers,
                    false,
                );
            }
            encoder.endEncoding();
        }
    }
}

impl Drop for MetalRenderer {
    fn drop(&mut self) {
        for command_buffer in self.in_flight.drain(..) {
            command_buffer.waitUntilCompleted();
        }
    }
}
