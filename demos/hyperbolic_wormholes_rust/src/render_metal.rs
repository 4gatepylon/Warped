//! `render_metal.rs` ray-traces the scene on the GPU with Metal: it compiles `render_metal.metal`
//! (a port of `render_cpu.rs`) at start-up, copies each frame's scene rows and uniforms to the
//! GPU, runs one thread per pixel, and copies the pixels back for SDL to show.
//!
//! Interface: `MetalRenderer::New`, `MetalRenderer::render`, `MetalRenderer::last_gpu_ms`.
//!
//! Example: `let mut renderer = MetalRenderer::New()?; renderer.render(&data, &mut pixels)?;`
//!
//! Must know:
//! - Performance: it waits for each frame to finish, so the GPU is idle while the CPU packs the
//!   next frame; `last_gpu_ms` is the GPU's own time for the last frame.

use std::error::Error;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder, MTLCommandQueue,
    MTLComputeCommandEncoder, MTLComputePipelineState, MTLCreateSystemDefaultDevice, MTLDevice,
    MTLLibrary, MTLResourceOptions, MTLSize,
};

use crate::scene_data::{
    ROW_COUNT, ROW_TEXEL_COUNT, SceneData, TEXEL_BEAM_CHUNKS, TEXEL_BEAM_POINTS, TEXEL_BOXES,
    TEXEL_CLUSTERS, TEXEL_GLUINGS, TEXEL_GROUND, TEXEL_MOUTHS, TEXEL_SPOT, TEXEL_SUN,
    TEXEL_TO_WORLD, TEXELS_PER_BEAM_CHUNK, TEXELS_PER_BOX, TEXELS_PER_CLUSTER, TEXELS_PER_GLUING,
    Uniforms,
};
use crate::world::{MATERIAL_GLASS, MATERIAL_GLOW, MAX_MOUTH_COUNT};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SHADER_SOURCE: &str = include_str!("render_metal.metal");
const KERNEL_NAME: &str = "trace_pixels";
const BYTES_PER_PIXEL: usize = 4;
const BYTES_PER_TEXEL: usize = 16;
const THREADGROUP_SIDE: usize = 8;
const MS_PER_S: f64 = 1000.0;

/// `MetalRenderer` holds the GPU, the compiled ray tracer, and the buffers for the scene rows and
/// the pixels.
pub struct MetalRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    rows: Retained<ProtocolObject<dyn MTLBuffer>>,
    pixels: Option<(Retained<ProtocolObject<dyn MTLBuffer>>, usize)>,
    pub last_gpu_ms: f64,
}

/// `fill_shader_constants` replaces each placeholder in the shader source (a name between double
/// underscores) with the Rust constant of
/// that name, so the shader's row layout always matches `scene_data.rs`.
///
/// Example: `let source = fill_shader_constants(SHADER_SOURCE);`
fn fill_shader_constants(source: &str) -> String {
    let constants: [(&str, usize); 18] = [
        ("MAX_MOUTH_COUNT", MAX_MOUTH_COUNT),
        ("ROW_TEXEL_COUNT", ROW_TEXEL_COUNT),
        ("TEXEL_TO_WORLD", TEXEL_TO_WORLD),
        ("TEXEL_GROUND", TEXEL_GROUND),
        ("TEXEL_SUN", TEXEL_SUN),
        ("TEXEL_MOUTHS", TEXEL_MOUTHS),
        ("TEXEL_GLUINGS", TEXEL_GLUINGS),
        ("TEXELS_PER_GLUING", TEXELS_PER_GLUING),
        ("TEXEL_BOXES", TEXEL_BOXES),
        ("TEXELS_PER_BOX", TEXELS_PER_BOX),
        ("TEXEL_CLUSTERS", TEXEL_CLUSTERS),
        ("TEXELS_PER_CLUSTER", TEXELS_PER_CLUSTER),
        ("TEXEL_SPOT", TEXEL_SPOT),
        ("TEXEL_BEAM_CHUNKS", TEXEL_BEAM_CHUNKS),
        ("TEXELS_PER_BEAM_CHUNK", TEXELS_PER_BEAM_CHUNK),
        ("TEXEL_BEAM_POINTS", TEXEL_BEAM_POINTS),
        ("MATERIAL_GLASS", MATERIAL_GLASS as usize),
        ("MATERIAL_GLOW", MATERIAL_GLOW as usize),
    ];
    let mut filled: String = source.to_string();
    for (name, value) in constants {
        filled = filled.replace(&format!("__{name}__"), &value.to_string());
    }
    assert!(
        !filled.contains("__"),
        "render_metal.metal has a placeholder render_metal.rs doesn't fill"
    );
    filled
}

impl MetalRenderer {
    /// `New` opens the default GPU, compiles the ray tracer and allocates the scene buffer.
    ///
    /// Example: `let renderer = MetalRenderer::New()?;`
    #[allow(non_snake_case)]
    pub fn New() -> Result<MetalRenderer> {
        let device: Retained<ProtocolObject<dyn MTLDevice>> =
            MTLCreateSystemDefaultDevice().ok_or("Metal: this machine has no GPU Metal can use")?;
        let source: Retained<NSString> = NSString::from_str(&fill_shader_constants(SHADER_SOURCE));
        let library: Retained<ProtocolObject<dyn MTLLibrary>> = device
            .newLibraryWithSource_options_error(&source, None)
            .map_err(|error| {
                format!(
                    "Metal: the shader didn't compile: {}",
                    error.localizedDescription()
                )
            })?;
        let function = library
            .newFunctionWithName(&NSString::from_str(KERNEL_NAME))
            .ok_or(format!(
                "Metal: the shader has no kernel named {KERNEL_NAME}"
            ))?;
        let pipeline: Retained<ProtocolObject<dyn MTLComputePipelineState>> = device
            .newComputePipelineStateWithFunction_error(&function)
            .map_err(|error| {
                format!(
                    "Metal: the pipeline failed: {}",
                    error.localizedDescription()
                )
            })?;
        let queue: Retained<ProtocolObject<dyn MTLCommandQueue>> =
            device.newCommandQueue().ok_or("Metal: no command queue")?;
        let rows: Retained<ProtocolObject<dyn MTLBuffer>> = device
            .newBufferWithLength_options(
                ROW_COUNT * ROW_TEXEL_COUNT * BYTES_PER_TEXEL,
                MTLResourceOptions::StorageModeShared,
            )
            .ok_or("Metal: no memory for the scene rows")?;
        Ok(MetalRenderer {
            device,
            queue,
            pipeline,
            rows,
            pixels: None,
            last_gpu_ms: 0.0,
        })
    }

    /// `render` draws one frame's packed scene into `pixels` (RGBA, top row first) and waits for
    /// the GPU to finish.
    ///
    /// Example: `renderer.render(&data, &mut pixels)?;`
    pub fn render(&mut self, data: &SceneData, pixels: &mut [u8]) -> Result<()> {
        let (width, height) = (
            data.uniforms.width_px as usize,
            data.uniforms.height_px as usize,
        );
        let pixel_bytes: usize = width * height * BYTES_PER_PIXEL;
        if pixels.len() != pixel_bytes {
            return Err(format!(
                "Metal: the pixel buffer has {} bytes, not {pixel_bytes} for {width}x{height}",
                pixels.len()
            )
            .into());
        }
        if self
            .pixels
            .as_ref()
            .is_none_or(|(_, bytes)| *bytes != pixel_bytes)
        {
            let buffer = self
                .device
                .newBufferWithLength_options(pixel_bytes, MTLResourceOptions::StorageModeShared)
                .ok_or("Metal: no memory for the pixels")?;
            self.pixels = Some((buffer, pixel_bytes));
        }
        let (pixel_buffer, _) = self
            .pixels
            .as_ref()
            .expect("the pixel buffer was just made");
        let row_bytes: usize = data.rows.len() * BYTES_PER_TEXEL;
        assert!(
            row_bytes <= self.rows.length(),
            "Metal: the scene rows don't fit their buffer"
        );
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A WRITE PAST THE ROWS BUFFER WILL HAPPEN
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.rows.as_ptr().cast::<u8>(),
                self.rows.contents().as_ptr().cast::<u8>(),
                row_bytes,
            )
        };
        let command_buffer = self
            .queue
            .commandBuffer()
            .ok_or("Metal: no command buffer")?;
        let encoder = command_buffer
            .computeCommandEncoder()
            .ok_or("Metal: no compute encoder")?;
        encoder.setComputePipelineState(&self.pipeline);
        let uniforms: NonNull<Uniforms> = NonNull::from(&data.uniforms);
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&self.rows), 0, 0);
            encoder.setBytes_length_atIndex(uniforms.cast(), std::mem::size_of::<Uniforms>(), 1);
            encoder.setBuffer_offset_atIndex(Some(pixel_buffer), 0, 2);
        }
        encoder.dispatchThreads_threadsPerThreadgroup(
            MTLSize {
                width,
                height,
                depth: 1,
            },
            MTLSize {
                width: THREADGROUP_SIDE,
                height: THREADGROUP_SIDE,
                depth: 1,
            },
        );
        encoder.endEncoding();
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        if command_buffer.status() != MTLCommandBufferStatus::Completed {
            let reason: String = command_buffer.error().map_or("unknown".to_string(), |e| {
                e.localizedDescription().to_string()
            });
            return Err(format!("Metal: the frame failed: {reason}").into());
        }
        self.last_gpu_ms = (command_buffer.GPUEndTime() - command_buffer.GPUStartTime()) * MS_PER_S;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE PIXEL BUFFER WILL HAPPEN
        unsafe {
            std::ptr::copy_nonoverlapping(
                pixel_buffer.contents().as_ptr().cast::<u8>(),
                pixels.as_mut_ptr(),
                pixel_bytes,
            )
        };
        Ok(())
    }
}
