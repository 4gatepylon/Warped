//! `render_metal.rs` ray-traces the scene on the GPU with Metal: it compiles `render_metal.metal`
//! (a port of `render_cpu.rs`) at start-up, copies each frame's scene rows and uniforms to the
//! GPU, runs one thread per pixel into a texture, and, when the output is larger than the render
//! size, upscales it with MetalFX's spatial scaler. The result is either read back (for the
//! benchmark's PNGs) or copied straight into a window's `CAMetalLayer` drawable.
//!
//! Interface: `MetalRenderer::New`, `MetalRenderer::render_to_pixels`,
//! `MetalRenderer::render_to_layer`, `MetalRenderer::get_device`, `MetalRenderer::last_gpu_ms`.
//!
//! Example: `let mut renderer = MetalRenderer::New()?; renderer.render_to_pixels(&data, size, &mut pixels)?;`
//!
//! Must know:
//! - Performance: it waits for each frame to finish, so the GPU is idle while the CPU packs the
//!   next frame; `last_gpu_ms` is the GPU's own time for the last frame, upscaling included.

use std::error::Error;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder,
    MTLCommandQueue, MTLComputeCommandEncoder, MTLComputePipelineState,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary, MTLOrigin, MTLPixelFormat,
    MTLResourceOptions, MTLSize, MTLStorageMode, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};
use objc2_metal_fx::{MTLFXSpatialScaler, MTLFXSpatialScalerBase, MTLFXSpatialScalerDescriptor};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use crate::scene_data::{
    ROW_COUNT, ROW_TEXEL_COUNT, SceneData, TEXEL_BEAM_CHUNKS, TEXEL_BEAM_POINTS, TEXEL_BOXES,
    TEXEL_CLUSTER_INFO, TEXEL_CLUSTER_ORDER, TEXEL_CLUSTERS, TEXEL_GLUINGS, TEXEL_GROUND,
    TEXEL_MOUTHS, TEXEL_SPOT, TEXEL_SUN, TEXEL_TO_WORLD, TEXELS_PER_BEAM_CHUNK, TEXELS_PER_BOX,
    TEXELS_PER_CLUSTER, TEXELS_PER_GLUING, Uniforms,
};
use crate::world::{MATERIAL_GLASS, MATERIAL_GLOW, MAX_CLUSTER_COUNT, MAX_MOUTH_COUNT};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `Texture` is a Metal texture and its size in pixels.
type Texture = (Retained<ProtocolObject<dyn MTLTexture>>, (usize, usize));

/// `Scaler` is a MetalFX spatial scaler with the input and output sizes it was made for.
type Scaler = (
    Retained<ProtocolObject<dyn MTLFXSpatialScaler>>,
    (usize, usize),
    (usize, usize),
);

/// `Frame` is an encoded frame: its command buffer (not yet committed) and the texture that will
/// hold its final image.
type Frame = (
    Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    Retained<ProtocolObject<dyn MTLTexture>>,
);

const SHADER_SOURCE: &str = include_str!("render_metal.metal");
const KERNEL_NAME: &str = "trace_pixels";
const BYTES_PER_PIXEL: usize = 4;
const BYTES_PER_TEXEL: usize = 16;
const THREADGROUP_SIDE: usize = 8;
const MS_PER_S: f64 = 1000.0;
const PIXEL_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;
const _: () = assert!(
    MAX_CLUSTER_COUNT <= 32,
    "the shader keeps candidate clusters in a 32-bit mask"
);

/// `MetalRenderer` holds the GPU, the compiled ray tracer, the scene buffer, and the textures and
/// MetalFX scaler for the current render and output sizes.
pub struct MetalRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    rows: Retained<ProtocolObject<dyn MTLBuffer>>,
    render_texture: Option<Texture>,
    output_texture: Option<Texture>,
    scaler: Option<Scaler>,
    readback: Option<(Retained<ProtocolObject<dyn MTLBuffer>>, usize)>,
    pub last_gpu_ms: f64,
}

/// `fill_shader_constants` replaces each placeholder in the shader source (a name between double
/// underscores) with the Rust constant of that name, so the shader's row layout always matches
/// `scene_data.rs`.
///
/// Example: `let source = fill_shader_constants(SHADER_SOURCE);`
fn fill_shader_constants(source: &str) -> String {
    let constants: [(&str, usize); 20] = [
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
        ("TEXEL_CLUSTER_INFO", TEXEL_CLUSTER_INFO),
        ("TEXEL_CLUSTER_ORDER", TEXEL_CLUSTER_ORDER),
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
            render_texture: None,
            output_texture: None,
            scaler: None,
            readback: None,
            last_gpu_ms: 0.0,
        })
    }

    /// `get_device` gives the GPU, so a window's layer can use the same one.
    ///
    /// Example: `layer.setDevice(Some(renderer.get_device()));`
    pub fn get_device(&self) -> &ProtocolObject<dyn MTLDevice> {
        &self.device
    }

    /// `make_texture` makes a private BGRA texture of a size with the given uses.
    ///
    /// Example: `let texture = self.make_texture((640, 400), MTLTextureUsage::ShaderWrite)?;`
    fn make_texture(&self, size_px: (usize, usize), usage: MTLTextureUsage) -> Result<Texture> {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A TEXTURE OF THE WRONG FORMAT OR SIZE WILL HAPPEN
        let descriptor: Retained<MTLTextureDescriptor> = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                PIXEL_FORMAT,
                size_px.0,
                size_px.1,
                false,
            )
        };
        descriptor.setUsage(usage);
        descriptor.setStorageMode(MTLStorageMode::Private);
        let texture = self
            .device
            .newTextureWithDescriptor(&descriptor)
            .ok_or(format!(
                "Metal: no memory for a {}x{} texture",
                size_px.0, size_px.1
            ))?;
        Ok((texture, size_px))
    }

    /// `prepare_upscaling` makes the MetalFX scaler and its output texture for a pair of sizes, if
    /// the current ones are for another pair.
    ///
    /// Example: `self.prepare_upscaling((1280, 800), (2560, 1600))?;`
    fn prepare_upscaling(
        &mut self,
        render_px: (usize, usize),
        output_px: (usize, usize),
    ) -> Result<()> {
        if self
            .scaler
            .as_ref()
            .is_some_and(|(_, input, output)| *input == render_px && *output == output_px)
        {
            return Ok(());
        }
        let descriptor: Retained<MTLFXSpatialScalerDescriptor> =
            unsafe { MTLFXSpatialScalerDescriptor::new() };
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A SCALER THAT DOESN'T MATCH ITS TEXTURES WILL HAPPEN
        let scaler = unsafe {
            descriptor.setInputWidth(render_px.0);
            descriptor.setInputHeight(render_px.1);
            descriptor.setOutputWidth(output_px.0);
            descriptor.setOutputHeight(output_px.1);
            descriptor.setColorTextureFormat(PIXEL_FORMAT);
            descriptor.setOutputTextureFormat(PIXEL_FORMAT);
            descriptor.newSpatialScalerWithDevice(&self.device)
        }
        .ok_or("MetalFX: this GPU has no spatial scaler")?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN AN OUTPUT TEXTURE THE SCALER CAN'T WRITE WILL HAPPEN
        let usage: MTLTextureUsage =
            unsafe { scaler.outputTextureUsage() } | MTLTextureUsage::ShaderRead;
        self.output_texture = Some(self.make_texture(output_px, usage)?);
        self.scaler = Some((scaler, render_px, output_px));
        Ok(())
    }

    /// `encode_frame` encodes one frame: the scene rows are copied to the GPU, the ray tracer runs
    /// at the scene's render size, and MetalFX upscales the result when `output_px` is larger.
    ///
    /// Example: `let (command_buffer, texture) = self.encode_frame(&data, (2560, 1600))?;`
    fn encode_frame(&mut self, data: &SceneData, output_px: (usize, usize)) -> Result<Frame> {
        let render_px: (usize, usize) = (
            data.uniforms.width_px as usize,
            data.uniforms.height_px as usize,
        );
        let is_upscaled: bool = output_px != render_px;
        if self
            .render_texture
            .as_ref()
            .is_none_or(|(_, size)| *size != render_px)
        {
            self.render_texture = Some(self.make_texture(
                render_px,
                MTLTextureUsage::ShaderWrite | MTLTextureUsage::ShaderRead,
            )?);
        }
        if is_upscaled {
            self.prepare_upscaling(render_px, output_px)?;
        }
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
        let (render_texture, _) = self
            .render_texture
            .as_ref()
            .expect("the render texture was just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&self.rows), 0, 0);
            encoder.setBytes_length_atIndex(uniforms.cast(), std::mem::size_of::<Uniforms>(), 1);
            encoder.setTexture_atIndex(Some(render_texture), 0);
        }
        encoder.dispatchThreads_threadsPerThreadgroup(
            MTLSize {
                width: render_px.0,
                height: render_px.1,
                depth: 1,
            },
            MTLSize {
                width: THREADGROUP_SIDE,
                height: THREADGROUP_SIDE,
                depth: 1,
            },
        );
        encoder.endEncoding();
        if !is_upscaled {
            return Ok((command_buffer, render_texture.clone()));
        }
        let (scaler, _, _) = self.scaler.as_ref().expect("the scaler was just made");
        let (output_texture, _) = self
            .output_texture
            .as_ref()
            .expect("the output texture was just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN MetalFX READING OR WRITING THE WRONG TEXTURE WILL HAPPEN
        unsafe {
            scaler.setColorTexture(Some(render_texture));
            scaler.setOutputTexture(Some(output_texture));
            scaler.encodeToCommandBuffer(&command_buffer);
        }
        Ok((command_buffer, output_texture.clone()))
    }

    /// `finish` commits a command buffer, waits for it, checks it worked and records its GPU time.
    ///
    /// Example: `self.finish(&command_buffer)?;`
    fn finish(&mut self, command_buffer: &ProtocolObject<dyn MTLCommandBuffer>) -> Result<()> {
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        if command_buffer.status() != MTLCommandBufferStatus::Completed {
            let reason: String = command_buffer
                .error()
                .map_or("unknown".to_string(), |error| {
                    error.localizedDescription().to_string()
                });
            return Err(format!("Metal: the frame failed: {reason}").into());
        }
        self.last_gpu_ms = (command_buffer.GPUEndTime() - command_buffer.GPUStartTime()) * MS_PER_S;
        Ok(())
    }

    /// `render_to_pixels` draws one frame at `output_px` (upscaled with MetalFX if larger than the
    /// render size) and reads it back into `pixels` (RGBA, top row first).
    ///
    /// Example: `renderer.render_to_pixels(&data, (2560, 1600), &mut pixels)?;`
    pub fn render_to_pixels(
        &mut self,
        data: &SceneData,
        output_px: (usize, usize),
        pixels: &mut [u8],
    ) -> Result<()> {
        let pixel_bytes: usize = output_px.0 * output_px.1 * BYTES_PER_PIXEL;
        if pixels.len() != pixel_bytes {
            return Err(format!(
                "Metal: the pixel buffer has {} bytes, not {pixel_bytes}",
                pixels.len()
            )
            .into());
        }
        if self
            .readback
            .as_ref()
            .is_none_or(|(_, bytes)| *bytes != pixel_bytes)
        {
            let buffer = self
                .device
                .newBufferWithLength_options(pixel_bytes, MTLResourceOptions::StorageModeShared)
                .ok_or("Metal: no memory to read the pixels back")?;
            self.readback = Some((buffer, pixel_bytes));
        }
        let (command_buffer, texture) = self.encode_frame(data, output_px)?;
        let (readback, _) = self
            .readback
            .as_ref()
            .expect("the readback buffer was just made");
        let blit = command_buffer
            .blitCommandEncoder()
            .ok_or("Metal: no blit encoder")?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY PAST THE READBACK BUFFER WILL HAPPEN
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                &texture,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize { width: output_px.0, height: output_px.1, depth: 1 },
                readback,
                0,
                output_px.0 * BYTES_PER_PIXEL,
                pixel_bytes,
            );
        }
        blit.endEncoding();
        self.finish(&command_buffer)?;
        let (readback, _) = self
            .readback
            .as_ref()
            .expect("the readback buffer was just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE READBACK BUFFER WILL HAPPEN
        unsafe {
            std::ptr::copy_nonoverlapping(
                readback.contents().as_ptr().cast::<u8>(),
                pixels.as_mut_ptr(),
                pixel_bytes,
            )
        };
        for pixel in pixels.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
            pixel.swap(0, 2);
        }
        Ok(())
    }

    /// `render_to_layer` draws one frame straight into the next drawable of a window's
    /// `CAMetalLayer`, upscaled with MetalFX to the drawable's size, and presents it. It returns
    /// false if the layer had no drawable to give.
    ///
    /// Example: `renderer.render_to_layer(&data, &layer)?;`
    pub fn render_to_layer(&mut self, data: &SceneData, layer: &CAMetalLayer) -> Result<bool> {
        let Some(drawable) = layer.nextDrawable() else {
            return Ok(false);
        };
        let target = drawable.texture();
        let output_px: (usize, usize) = (target.width(), target.height());
        let (command_buffer, texture) = self.encode_frame(data, output_px)?;
        let blit = command_buffer
            .blitCommandEncoder()
            .ok_or("Metal: no blit encoder")?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY BETWEEN TEXTURES OF DIFFERENT SIZES WILL HAPPEN
        unsafe { blit.copyFromTexture_toTexture(&texture, &target) };
        blit.endEncoding();
        command_buffer.presentDrawable(ProtocolObject::from_ref(&*drawable));
        self.finish(&command_buffer)?;
        Ok(true)
    }
}
