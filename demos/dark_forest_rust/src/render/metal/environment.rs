//! `environment` turns the HDR sky into image-based lighting (an environment cube, a prefiltered
//! reflection cube, and a BRDF lookup table) and bakes the arena's sky-visibility volume.
//!
//! Example:
//!     let ibl: IblTextures = build_ibl(&device, &queue, &pipelines, &environment);

use std::ffi::c_void;
use std::ptr::NonNull;

use glam::{UVec3, UVec4, Vec3, Vec4};
use half::f16;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLComputeCommandEncoder,
    MTLComputePipelineState, MTLDevice, MTLPixelFormat, MTLSize, MTLTextureType, MTLTextureUsage,
};

use super::gpu_types::{GpuOccluderBox, GpuSkyVisibilityParams};
use super::pipelines::Pipelines;
use super::resources::{
    Buffer, Texture, TextureSpec, create_buffer, create_texture, mip_count_for, mip_view,
    upload_slices,
};
use crate::render::{EnvironmentData, OccluderBox};

/// `ENVIRONMENT_CUBE_SIZE_PX` is the face size of the sky cube that the sky pass draws.
const ENVIRONMENT_CUBE_SIZE_PX: usize = 1024;
/// `PREFILTERED_CUBE_SIZE_PX` is the face size of the sharpest reflection mip.
const PREFILTERED_CUBE_SIZE_PX: usize = 256;
/// `PREFILTERED_MIP_COUNT` is the number of roughness levels in the reflection cube.
pub const PREFILTERED_MIP_COUNT: usize = 6;
/// `BRDF_LUT_SIZE_PX` is the size of the split-sum BRDF table.
const BRDF_LUT_SIZE_PX: usize = 256;
/// `SKY_VISIBILITY_CELL_M` is the size of one cell of the sky-visibility volume.
const SKY_VISIBILITY_CELL_M: f32 = 0.75;
/// `CUBE_FACE_COUNT` is the number of faces of a cube texture.
const CUBE_FACE_COUNT: usize = 6;
/// `THREADGROUP_SIDE` is the width and height of a 2D compute threadgroup.
const THREADGROUP_SIDE: usize = 8;
/// `VOLUME_THREADGROUP_SIDE` is the side of a 3D compute threadgroup.
const VOLUME_THREADGROUP_SIDE: usize = 4;

/// `IblTextures` is the image-based lighting made from one sky.
pub struct IblTextures {
    pub environment: Texture,
    pub prefiltered: Texture,
    pub brdf_lut: Texture,
}

/// `SkyVisibility` is the baked volume of how much sky each point of the arena sees.
pub struct SkyVisibility {
    pub volume: Texture,
    pub origin_m: Vec3,
    pub size_m: Vec3,
}

/// `dispatch` runs a compute kernel over a grid, with its textures and inline bytes bound.
fn dispatch(
    queue: &ProtocolObject<dyn MTLCommandQueue>,
    pipeline: &ProtocolObject<dyn MTLComputePipelineState>,
    textures: &[&Texture],
    bytes: Option<&[u8]>,
    buffers: &[&Buffer],
    grid: MTLSize,
    group: MTLSize,
) {
    let command_buffer = queue
        .commandBuffer()
        .expect("command buffer creation failed");
    let encoder = command_buffer
        .computeCommandEncoder()
        .expect("compute encoder creation failed");
    encoder.setComputePipelineState(pipeline);
    for (index, texture) in textures.iter().enumerate() {
        unsafe { encoder.setTexture_atIndex(Some(texture), index) };
    }
    for (index, buffer) in buffers.iter().enumerate() {
        unsafe { encoder.setBuffer_offset_atIndex(Some(buffer), 0, index) };
    }
    if let Some(inline_bytes) = bytes {
        let pointer: NonNull<c_void> = NonNull::from(inline_bytes).cast();
        let index: usize = buffers.len();
        unsafe { encoder.setBytes_length_atIndex(pointer, inline_bytes.len(), index) };
    }
    encoder.dispatchThreads_threadsPerThreadgroup(grid, group);
    encoder.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();
}

/// `build_ibl` uploads the sky and runs the kernels that make its lighting textures.
///
/// Example:
///     let ibl: IblTextures = build_ibl(&device, &queue, &pipelines, &environment);
pub fn build_ibl(
    device: &ProtocolObject<dyn MTLDevice>,
    queue: &ProtocolObject<dyn MTLCommandQueue>,
    pipelines: &Pipelines,
    environment: &EnvironmentData,
) -> IblTextures {
    let width: usize = environment.width as usize;
    let height: usize = environment.height as usize;
    let mut half_pixels: Vec<f16> = Vec::with_capacity(width * height * 4);
    for rgb in environment.rgb_pixels.as_chunks::<3>().0 {
        half_pixels.extend(rgb.iter().map(|value: &f32| f16::from_f32(*value)));
        half_pixels.push(f16::ONE);
    }
    let equirect: Texture = create_texture(
        device,
        &TextureSpec {
            usage: MTLTextureUsage::ShaderRead,
            ..TextureSpec::render_target(MTLPixelFormat::RGBA16Float, width, height)
        },
        "equirect",
    );
    let half_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(half_pixels.as_ptr().cast::<u8>(), half_pixels.len() * 2)
    };
    upload_slices(device, queue, &equirect, half_bytes, 8, 1);

    let cube_spec = |size: usize, mip_count: usize| TextureSpec {
        texture_type: MTLTextureType::TypeCube,
        format: MTLPixelFormat::RGBA16Float,
        width: size,
        height: size,
        depth: 1,
        array_length: 1,
        mip_count,
        usage: MTLTextureUsage::ShaderRead
            | MTLTextureUsage::ShaderWrite
            | MTLTextureUsage::RenderTarget,
    };
    let environment_cube: Texture = create_texture(
        device,
        &cube_spec(
            ENVIRONMENT_CUBE_SIZE_PX,
            mip_count_for(ENVIRONMENT_CUBE_SIZE_PX, 1),
        ),
        "environment",
    );
    let group: MTLSize = MTLSize {
        width: THREADGROUP_SIDE,
        height: THREADGROUP_SIDE,
        depth: 1,
    };
    let environment_level_0: Texture = mip_view(
        &environment_cube,
        MTLTextureType::TypeCube,
        0,
        CUBE_FACE_COUNT,
    );
    dispatch(
        queue,
        &pipelines.convert_equirect_to_cube,
        &[&equirect, &environment_level_0],
        None,
        &[],
        MTLSize {
            width: ENVIRONMENT_CUBE_SIZE_PX,
            height: ENVIRONMENT_CUBE_SIZE_PX,
            depth: CUBE_FACE_COUNT,
        },
        group,
    );
    let command_buffer = queue
        .commandBuffer()
        .expect("command buffer creation failed");
    let blit = command_buffer
        .blitCommandEncoder()
        .expect("blit encoder creation failed");
    objc2_metal::MTLBlitCommandEncoder::generateMipmapsForTexture(&*blit, &environment_cube);
    blit.endEncoding();
    command_buffer.commit();

    let prefiltered: Texture = create_texture(
        device,
        &cube_spec(PREFILTERED_CUBE_SIZE_PX, PREFILTERED_MIP_COUNT),
        "prefiltered",
    );
    for level in 0..PREFILTERED_MIP_COUNT {
        let level_view: Texture = mip_view(
            &prefiltered,
            MTLTextureType::TypeCube,
            level,
            CUBE_FACE_COUNT,
        );
        let roughness: f32 = level as f32 / (PREFILTERED_MIP_COUNT - 1) as f32;
        let size: usize = (PREFILTERED_CUBE_SIZE_PX >> level).max(1);
        dispatch(
            queue,
            &pipelines.prefilter_specular,
            &[&environment_cube, &level_view],
            Some(&roughness.to_ne_bytes()),
            &[],
            MTLSize {
                width: size,
                height: size,
                depth: CUBE_FACE_COUNT,
            },
            group,
        );
    }

    let brdf_lut: Texture = create_texture(
        device,
        &TextureSpec {
            usage: MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite,
            ..TextureSpec::render_target(
                MTLPixelFormat::RG16Float,
                BRDF_LUT_SIZE_PX,
                BRDF_LUT_SIZE_PX,
            )
        },
        "brdf_lut",
    );
    dispatch(
        queue,
        &pipelines.integrate_brdf,
        &[&brdf_lut],
        None,
        &[],
        MTLSize {
            width: BRDF_LUT_SIZE_PX,
            height: BRDF_LUT_SIZE_PX,
            depth: 1,
        },
        group,
    );
    IblTextures {
        environment: environment_cube,
        prefiltered,
        brdf_lut,
    }
}

/// `bake_sky_visibility` bakes how much sky each cell of the arena sees past the occluders.
///
/// Example:
///     let sky: SkyVisibility = bake_sky_visibility(&device, &queue, &pipelines, &boxes, min, max);
pub fn bake_sky_visibility(
    device: &ProtocolObject<dyn MTLDevice>,
    queue: &ProtocolObject<dyn MTLCommandQueue>,
    pipelines: &Pipelines,
    occluders: &[OccluderBox],
    bounds_min_m: Vec3,
    bounds_max_m: Vec3,
) -> SkyVisibility {
    let counts: UVec3 = ((bounds_max_m - bounds_min_m) / SKY_VISIBILITY_CELL_M)
        .ceil()
        .as_uvec3()
        .max(UVec3::ONE);
    let size_m: Vec3 = counts.as_vec3() * SKY_VISIBILITY_CELL_M;
    let boxes: Vec<GpuOccluderBox> = occluders
        .iter()
        .map(|occluder: &OccluderBox| GpuOccluderBox {
            min_m: occluder.min_m.extend(0.0),
            max_m: occluder.max_m.extend(0.0),
        })
        .collect();
    let params: GpuSkyVisibilityParams = GpuSkyVisibilityParams {
        origin_m: bounds_min_m.extend(0.0),
        cell_size_m: Vec4::splat(SKY_VISIBILITY_CELL_M),
        counts: UVec4::new(counts.x, counts.y, counts.z, boxes.len() as u32),
    };
    let volume: Texture = create_texture(
        device,
        &TextureSpec {
            texture_type: MTLTextureType::Type3D,
            format: MTLPixelFormat::R16Float,
            width: counts.x as usize,
            height: counts.y as usize,
            depth: counts.z as usize,
            array_length: 1,
            mip_count: 1,
            usage: MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite,
        },
        "sky_visibility",
    );
    let box_buffer: Buffer = create_buffer(device, &boxes);
    let params_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            (&params as *const GpuSkyVisibilityParams).cast::<u8>(),
            std::mem::size_of::<GpuSkyVisibilityParams>(),
        )
    };
    dispatch(
        queue,
        &pipelines.bake_sky_visibility,
        &[&volume],
        Some(params_bytes),
        &[&box_buffer],
        MTLSize {
            width: counts.x as usize,
            height: counts.y as usize,
            depth: counts.z as usize,
        },
        MTLSize {
            width: VOLUME_THREADGROUP_SIDE,
            height: VOLUME_THREADGROUP_SIDE,
            depth: VOLUME_THREADGROUP_SIDE,
        },
    );
    SkyVisibility {
        volume,
        origin_m: bounds_min_m,
        size_m,
    }
}
