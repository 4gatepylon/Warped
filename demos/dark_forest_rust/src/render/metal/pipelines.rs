//! `pipelines` compiles the MSL shaders and builds every render, compute, and depth state that
//! the Metal renderer uses.
//!
//! Example:
//!     let pipelines: Pipelines = Pipelines::new(&device);

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlendFactor, MTLCompareFunction, MTLComputePipelineState, MTLDataType,
    MTLDepthStencilDescriptor, MTLDepthStencilState, MTLDevice, MTLFunction,
    MTLFunctionConstantValues, MTLLibrary, MTLPixelFormat, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState,
};

/// `SHADER_SOURCE` is every shader file, in dependency order, as one MSL translation unit.
const SHADER_SOURCE: &str = concat!(
    include_str!("../../shaders/common.metal"),
    include_str!("../../shaders/ibl.metal"),
    include_str!("../../shaders/scene.metal"),
    include_str!("../../shaders/post.metal"),
    include_str!("../../shaders/overlay.metal"),
);

/// `HDR_FORMAT` is the format of the lit scene and the bloom chain.
pub const HDR_FORMAT: MTLPixelFormat = MTLPixelFormat::RGBA16Float;
/// `DEPTH_FORMAT` is the format of the scene and shadow depth buffers.
pub const DEPTH_FORMAT: MTLPixelFormat = MTLPixelFormat::Depth32Float;
/// `NORMAL_FORMAT` is the format of the prepass's view-space normals.
pub const NORMAL_FORMAT: MTLPixelFormat = MTLPixelFormat::RGBA16Float;
/// `AO_FORMAT` is the format of the ambient-occlusion targets.
pub const AO_FORMAT: MTLPixelFormat = MTLPixelFormat::R8Unorm;
/// `LDR_FORMAT` is the format of the tone-mapped image before FXAA.
pub const LDR_FORMAT: MTLPixelFormat = MTLPixelFormat::RGBA8Unorm;
/// `DRAWABLE_FORMAT` is the format of the window's drawable.
pub const DRAWABLE_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;

/// `Blend` says how a pipeline blends into its color target.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Blend {
    None,
    Alpha,
    Premultiplied,
    Additive,
}

/// `PipelinePair` is one pipeline in its static and skinned variants.
///
/// Example:
///     encoder.setRenderPipelineState(pair.get(is_skinned));
pub struct PipelinePair {
    pub static_mesh: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub skinned_mesh: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
}

impl PipelinePair {
    /// `get` returns the variant for a draw.
    ///
    /// Example:
    ///     let pipeline = pair.get(draw.joint_offset.is_some());
    pub fn get(&self, is_skinned: bool) -> &ProtocolObject<dyn MTLRenderPipelineState> {
        if is_skinned {
            &self.skinned_mesh
        } else {
            &self.static_mesh
        }
    }
}

/// `DepthStates` are the depth tests and writes that the passes use (reverse Z for the camera).
///
/// Example:
///     encoder.setDepthStencilState(Some(&depth_states.greater_write));
pub struct DepthStates {
    pub less_write: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    pub greater_write: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    pub greater_equal_write: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    pub greater_equal_read: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
}

/// `Pipelines` is every pipeline the renderer uses.
///
/// Example:
///     let pipelines: Pipelines = Pipelines::new(&device);
pub struct Pipelines {
    pub shadow: PipelinePair,
    pub shadow_masked: PipelinePair,
    pub prepass: PipelinePair,
    pub forward_opaque: PipelinePair,
    pub forward_masked: PipelinePair,
    pub forward_blend: PipelinePair,
    pub sky: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub decal: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub particle: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub ssao: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub ssao_blur: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub bloom_downsample: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub bloom_upsample: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub tonemap: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub lens: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub fxaa: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub hud: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub hud_image: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub overlay_rects: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub convert_equirect_to_cube: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    pub prefilter_specular: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    pub integrate_brdf: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    pub bake_sky_visibility: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    pub depth_states: DepthStates,
}

/// `RenderPipelineSpec` is what differs between the render pipelines.
struct RenderPipelineSpec<'a> {
    vertex: &'a ProtocolObject<dyn MTLFunction>,
    fragment: Option<&'a ProtocolObject<dyn MTLFunction>>,
    color_format: Option<MTLPixelFormat>,
    depth_format: Option<MTLPixelFormat>,
    blend: Blend,
    label: &'a str,
}

/// `function` looks up a shader function, specializing `IS_SKINNED` if `is_skinned` is given.
///
/// Example:
///     let vertex = function(&library, "scene_vertex", Some(true));
fn function(
    library: &ProtocolObject<dyn MTLLibrary>,
    name: &str,
    is_skinned: Option<bool>,
) -> Retained<ProtocolObject<dyn MTLFunction>> {
    let ns_name: Retained<NSString> = NSString::from_str(name);
    match is_skinned {
        None => library
            .newFunctionWithName(&ns_name)
            .unwrap_or_else(|| panic!("shader function `{name}` is missing")),
        Some(is_skinned_value) => {
            let values: Retained<MTLFunctionConstantValues> = MTLFunctionConstantValues::new();
            let value: bool = is_skinned_value;
            let value_pointer: NonNull<c_void> = NonNull::from(&value).cast();
            unsafe { values.setConstantValue_type_atIndex(value_pointer, MTLDataType::Bool, 0) };
            library
                .newFunctionWithName_constantValues_error(&ns_name, &values)
                .unwrap_or_else(|error| panic!("shader function `{name}`: {error}"))
        }
    }
}

/// `render_pipeline` builds one render pipeline.
fn render_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    spec: RenderPipelineSpec,
) -> Retained<ProtocolObject<dyn MTLRenderPipelineState>> {
    let descriptor: Retained<MTLRenderPipelineDescriptor> = MTLRenderPipelineDescriptor::new();
    descriptor.setLabel(Some(&NSString::from_str(spec.label)));
    descriptor.setVertexFunction(Some(spec.vertex));
    descriptor.setFragmentFunction(spec.fragment);
    if let Some(color_format) = spec.color_format {
        let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setPixelFormat(color_format);
        if spec.blend != Blend::None {
            let (source_rgb, destination_rgb, source_alpha, destination_alpha) = match spec.blend {
                Blend::Alpha => (
                    MTLBlendFactor::SourceAlpha,
                    MTLBlendFactor::OneMinusSourceAlpha,
                    MTLBlendFactor::One,
                    MTLBlendFactor::OneMinusSourceAlpha,
                ),
                Blend::Premultiplied => (
                    MTLBlendFactor::One,
                    MTLBlendFactor::OneMinusSourceAlpha,
                    MTLBlendFactor::One,
                    MTLBlendFactor::OneMinusSourceAlpha,
                ),
                Blend::Additive | Blend::None => (
                    MTLBlendFactor::One,
                    MTLBlendFactor::One,
                    MTLBlendFactor::One,
                    MTLBlendFactor::One,
                ),
            };
            attachment.setBlendingEnabled(true);
            attachment.setSourceRGBBlendFactor(source_rgb);
            attachment.setDestinationRGBBlendFactor(destination_rgb);
            attachment.setSourceAlphaBlendFactor(source_alpha);
            attachment.setDestinationAlphaBlendFactor(destination_alpha);
        }
    }
    if let Some(depth_format) = spec.depth_format {
        descriptor.setDepthAttachmentPixelFormat(depth_format);
    }
    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .unwrap_or_else(|error| panic!("pipeline `{}`: {error}", spec.label))
}

/// `scene_pair` builds the static and skinned variants of a pipeline that draws meshes.
fn scene_pair(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    fragment_name: Option<&str>,
    color_format: Option<MTLPixelFormat>,
    blend: Blend,
    label: &str,
) -> PipelinePair {
    let fragment: Option<Retained<ProtocolObject<dyn MTLFunction>>> =
        fragment_name.map(|name: &str| function(library, name, None));
    let mut variants: Vec<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> = Vec::new();
    for is_skinned in [false, true] {
        let vertex: Retained<ProtocolObject<dyn MTLFunction>> =
            function(library, "scene_vertex", Some(is_skinned));
        variants.push(render_pipeline(
            device,
            RenderPipelineSpec {
                vertex: &vertex,
                fragment: fragment.as_deref(),
                color_format,
                depth_format: Some(DEPTH_FORMAT),
                blend,
                label,
            },
        ));
    }
    let skinned_mesh = variants.pop().expect("two variants were built");
    let static_mesh = variants.pop().expect("two variants were built");
    PipelinePair {
        static_mesh,
        skinned_mesh,
    }
}

/// `compute_pipeline` builds one compute pipeline.
fn compute_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    name: &str,
) -> Retained<ProtocolObject<dyn MTLComputePipelineState>> {
    let kernel: Retained<ProtocolObject<dyn MTLFunction>> = function(library, name, None);
    device
        .newComputePipelineStateWithFunction_error(&kernel)
        .unwrap_or_else(|error| panic!("compute pipeline `{name}`: {error}"))
}

/// `depth_state` builds one depth test.
fn depth_state(
    device: &ProtocolObject<dyn MTLDevice>,
    compare: MTLCompareFunction,
    is_write_enabled: bool,
) -> Retained<ProtocolObject<dyn MTLDepthStencilState>> {
    let descriptor: Retained<MTLDepthStencilDescriptor> = MTLDepthStencilDescriptor::new();
    descriptor.setDepthCompareFunction(compare);
    descriptor.setDepthWriteEnabled(is_write_enabled);
    device
        .newDepthStencilStateWithDescriptor(&descriptor)
        .expect("depth state creation failed")
}

impl Pipelines {
    /// `new` compiles the shaders and builds every pipeline; it panics with the compiler's
    /// message if a shader doesn't compile.
    ///
    /// Example:
    ///     let pipelines: Pipelines = Pipelines::new(&device);
    pub fn new(device: &ProtocolObject<dyn MTLDevice>) -> Pipelines {
        let library: Retained<ProtocolObject<dyn MTLLibrary>> = device
            .newLibraryWithSource_options_error(&NSString::from_str(SHADER_SOURCE), None)
            .unwrap_or_else(|error| panic!("shader compilation failed: {error}"));
        let fullscreen: Retained<ProtocolObject<dyn MTLFunction>> =
            function(&library, "fullscreen_vertex", None);
        let fullscreen_pipeline = |fragment_name: &str, format: MTLPixelFormat, blend: Blend| {
            let fragment: Retained<ProtocolObject<dyn MTLFunction>> =
                function(&library, fragment_name, None);
            render_pipeline(
                device,
                RenderPipelineSpec {
                    vertex: &fullscreen,
                    fragment: Some(&fragment),
                    color_format: Some(format),
                    depth_format: None,
                    blend,
                    label: fragment_name,
                },
            )
        };
        let hdr_pipeline = |vertex_name: &str, fragment_name: &str, blend: Blend| {
            let vertex: Retained<ProtocolObject<dyn MTLFunction>> =
                function(&library, vertex_name, None);
            let fragment: Retained<ProtocolObject<dyn MTLFunction>> =
                function(&library, fragment_name, None);
            render_pipeline(
                device,
                RenderPipelineSpec {
                    vertex: &vertex,
                    fragment: Some(&fragment),
                    color_format: Some(HDR_FORMAT),
                    depth_format: Some(DEPTH_FORMAT),
                    blend,
                    label: fragment_name,
                },
            )
        };
        let hud_vertex: Retained<ProtocolObject<dyn MTLFunction>> =
            function(&library, "hud_vertex", None);
        let hud_fragment: Retained<ProtocolObject<dyn MTLFunction>> =
            function(&library, "hud_fragment", None);
        let hud_image_vertex: Retained<ProtocolObject<dyn MTLFunction>> =
            function(&library, "hud_image_vertex", None);
        let hud_image_fragment: Retained<ProtocolObject<dyn MTLFunction>> =
            function(&library, "hud_image_fragment", None);
        Pipelines {
            shadow: scene_pair(device, &library, None, None, Blend::None, "shadow"),
            shadow_masked: scene_pair(
                device,
                &library,
                Some("shadow_mask_fragment"),
                None,
                Blend::None,
                "shadow_masked",
            ),
            prepass: scene_pair(
                device,
                &library,
                Some("prepass_fragment"),
                Some(NORMAL_FORMAT),
                Blend::None,
                "prepass",
            ),
            forward_opaque: scene_pair(
                device,
                &library,
                Some("forward_fragment"),
                Some(HDR_FORMAT),
                Blend::None,
                "forward_opaque",
            ),
            forward_masked: scene_pair(
                device,
                &library,
                Some("forward_masked_fragment"),
                Some(HDR_FORMAT),
                Blend::None,
                "forward_masked",
            ),
            forward_blend: scene_pair(
                device,
                &library,
                Some("forward_fragment"),
                Some(HDR_FORMAT),
                Blend::Alpha,
                "forward_blend",
            ),
            sky: hdr_pipeline("sky_vertex", "sky_fragment", Blend::None),
            decal: hdr_pipeline("decal_vertex", "decal_fragment", Blend::Alpha),
            particle: hdr_pipeline("particle_vertex", "particle_fragment", Blend::Premultiplied),
            ssao: fullscreen_pipeline("ssao_fragment", AO_FORMAT, Blend::None),
            ssao_blur: fullscreen_pipeline("ssao_blur_fragment", AO_FORMAT, Blend::None),
            bloom_downsample: fullscreen_pipeline(
                "bloom_downsample_fragment",
                HDR_FORMAT,
                Blend::None,
            ),
            bloom_upsample: fullscreen_pipeline(
                "bloom_upsample_fragment",
                HDR_FORMAT,
                Blend::Additive,
            ),
            tonemap: fullscreen_pipeline("tonemap_fragment", LDR_FORMAT, Blend::None),
            lens: fullscreen_pipeline("lens_fragment", LDR_FORMAT, Blend::None),
            fxaa: fullscreen_pipeline("fxaa_fragment", DRAWABLE_FORMAT, Blend::None),
            overlay_rects: fullscreen_pipeline(
                "overlay_rects_fragment",
                DRAWABLE_FORMAT,
                Blend::Premultiplied,
            ),
            hud: render_pipeline(
                device,
                RenderPipelineSpec {
                    vertex: &hud_vertex,
                    fragment: Some(&hud_fragment),
                    color_format: Some(DRAWABLE_FORMAT),
                    depth_format: None,
                    blend: Blend::Alpha,
                    label: "hud",
                },
            ),
            hud_image: render_pipeline(
                device,
                RenderPipelineSpec {
                    vertex: &hud_image_vertex,
                    fragment: Some(&hud_image_fragment),
                    color_format: Some(DRAWABLE_FORMAT),
                    depth_format: None,
                    blend: Blend::Alpha,
                    label: "hud image",
                },
            ),
            convert_equirect_to_cube: compute_pipeline(
                device,
                &library,
                "convert_equirect_to_cube",
            ),
            prefilter_specular: compute_pipeline(device, &library, "prefilter_specular"),
            integrate_brdf: compute_pipeline(device, &library, "integrate_brdf"),
            bake_sky_visibility: compute_pipeline(device, &library, "bake_sky_visibility"),
            depth_states: DepthStates {
                less_write: depth_state(device, MTLCompareFunction::Less, true),
                greater_write: depth_state(device, MTLCompareFunction::Greater, true),
                greater_equal_write: depth_state(device, MTLCompareFunction::GreaterEqual, true),
                greater_equal_read: depth_state(device, MTLCompareFunction::GreaterEqual, false),
            },
        }
    }
}

/// `tests` checks that the shaders compile into pipelines.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | pipeline | every pipeline in `Pipelines` | `test_shaders_compile` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_shaders_compile` compiles every shader and builds every pipeline on this Mac's GPU.
    #[test]
    fn test_shaders_compile() {
        let device: Retained<ProtocolObject<dyn MTLDevice>> =
            objc2_metal::MTLCreateSystemDefaultDevice().expect("this Mac has no Metal device");
        let _pipelines: Pipelines = Pipelines::new(&device);
    }
}
