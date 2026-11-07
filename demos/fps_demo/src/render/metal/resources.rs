//! `resources` creates Metal textures and buffers and uploads data into them.
//!
//! Example:
//!     let texture = upload_texture_2d(&device, &queue, &texture_data);

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSRange;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLDevice, MTLOrigin, MTLPixelFormat, MTLResource, MTLResourceOptions, MTLSize, MTLStorageMode,
    MTLTexture, MTLTextureDescriptor, MTLTextureType, MTLTextureUsage,
};

use crate::render::{TextureData, TextureFormat};

/// `Texture` is a retained Metal texture.
pub type Texture = Retained<ProtocolObject<dyn MTLTexture>>;
/// `Buffer` is a retained Metal buffer.
pub type Buffer = Retained<ProtocolObject<dyn MTLBuffer>>;

/// `TextureSpec` describes a GPU-only texture to create.
///
/// Example:
///     let spec: TextureSpec = TextureSpec::render_target(MTLPixelFormat::RGBA16Float, 64, 64);
pub struct TextureSpec {
    pub texture_type: MTLTextureType,
    pub format: MTLPixelFormat,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub array_length: usize,
    pub mip_count: usize,
    pub usage: MTLTextureUsage,
}

impl TextureSpec {
    /// `render_target` is a 2D texture that passes render into and later passes sample.
    ///
    /// Example:
    ///     let spec: TextureSpec = TextureSpec::render_target(MTLPixelFormat::R8Unorm, 8, 8);
    pub fn render_target(format: MTLPixelFormat, width: usize, height: usize) -> TextureSpec {
        TextureSpec {
            texture_type: MTLTextureType::Type2D,
            format,
            width: width.max(1),
            height: height.max(1),
            depth: 1,
            array_length: 1,
            mip_count: 1,
            usage: MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead,
        }
    }
}

/// `mip_count_for` returns the length of a full mip chain for a size.
///
/// Example:
///     assert_eq!(mip_count_for(256, 128), 9);
pub fn mip_count_for(width: usize, height: usize) -> usize {
    (usize::BITS - width.max(height).max(1).leading_zeros()) as usize
}

/// `create_texture` creates a private texture from a spec.
///
/// Example:
///     let texture: Texture = create_texture(&device, &spec, "hdr");
pub fn create_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    spec: &TextureSpec,
    label: &str,
) -> Texture {
    let descriptor: Retained<MTLTextureDescriptor> = MTLTextureDescriptor::new();
    descriptor.setTextureType(spec.texture_type);
    descriptor.setPixelFormat(spec.format);
    unsafe {
        descriptor.setWidth(spec.width);
        descriptor.setHeight(spec.height);
        descriptor.setDepth(spec.depth);
        descriptor.setArrayLength(spec.array_length);
        descriptor.setMipmapLevelCount(spec.mip_count);
    }
    descriptor.setUsage(spec.usage);
    descriptor.setStorageMode(MTLStorageMode::Private);
    let texture: Texture = device
        .newTextureWithDescriptor(&descriptor)
        .unwrap_or_else(|| panic!("texture `{label}` creation failed"));
    texture.setLabel(Some(&objc2_foundation::NSString::from_str(label)));
    texture
}

/// `mip_view` returns a view of one mip level (all slices) of a texture.
///
/// Example:
///     let level_view: Texture = mip_view(&bloom, MTLTextureType::Type2D, 2, 1);
pub fn mip_view(
    texture: &ProtocolObject<dyn MTLTexture>,
    texture_type: MTLTextureType,
    level: usize,
    slice_count: usize,
) -> Texture {
    unsafe {
        texture.newTextureViewWithPixelFormat_textureType_levels_slices(
            texture.pixelFormat(),
            texture_type,
            NSRange::new(level, 1),
            NSRange::new(0, slice_count),
        )
    }
    .expect("texture view creation failed")
}

/// `create_buffer` creates a shared buffer holding a copy of `data`.
///
/// Example:
///     let buffer: Buffer = create_buffer(&device, &vertices);
pub fn create_buffer<T: Copy>(device: &ProtocolObject<dyn MTLDevice>, data: &[T]) -> Buffer {
    let length_bytes: usize = std::mem::size_of_val(data).max(1);
    if data.is_empty() {
        return device
            .newBufferWithLength_options(length_bytes, MTLResourceOptions::StorageModeShared)
            .expect("buffer creation failed");
    }
    let pointer: NonNull<c_void> = NonNull::from(data).cast();
    unsafe {
        device.newBufferWithBytes_length_options(
            pointer,
            length_bytes,
            MTLResourceOptions::StorageModeShared,
        )
    }
    .expect("buffer creation failed")
}

/// `pixel_format_for` returns the Metal format and bytes per pixel of a texture format.
///
/// Example:
///     let (format, bytes) = pixel_format_for(TextureFormat::Rg8Linear);
fn pixel_format_for(format: TextureFormat) -> (MTLPixelFormat, usize) {
    match format {
        TextureFormat::Rgba8Srgb => (MTLPixelFormat::RGBA8Unorm_sRGB, 4),
        TextureFormat::Rgba8Linear => (MTLPixelFormat::RGBA8Unorm, 4),
        TextureFormat::Rg8Linear => (MTLPixelFormat::RG8Unorm, 2),
    }
}

/// `upload_slices` copies tightly packed slices (faces, layers, or depth slices) into level 0 of a
/// texture, then generates the rest of its mip chain if it has one.
///
/// The copy runs on the GPU queue without waiting; later work on the queue sees it.
///
/// Example:
///     upload_slices(&device, &queue, &texture, &bytes, 4, 1);
pub fn upload_slices(
    device: &ProtocolObject<dyn MTLDevice>,
    queue: &ProtocolObject<dyn MTLCommandQueue>,
    texture: &ProtocolObject<dyn MTLTexture>,
    bytes: &[u8],
    bytes_per_pixel: usize,
    slice_count: usize,
) {
    let width: usize = texture.width();
    let height: usize = texture.height();
    let depth: usize = texture.depth();
    let bytes_per_row: usize = width * bytes_per_pixel;
    let bytes_per_image: usize = bytes_per_row * height;
    let staging: Buffer = create_buffer(device, bytes);
    let command_buffer = queue
        .commandBuffer()
        .expect("command buffer creation failed");
    let blit = command_buffer
        .blitCommandEncoder()
        .expect("blit encoder creation failed");
    for slice in 0..slice_count {
        unsafe {
            blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &staging,
                slice * bytes_per_image * depth,
                bytes_per_row,
                bytes_per_image,
                MTLSize { width, height, depth },
                texture,
                slice,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
            );
        }
    }
    if texture.mipmapLevelCount() > 1 {
        blit.generateMipmapsForTexture(texture);
    }
    blit.endEncoding();
    command_buffer.commit();
}

/// `upload_texture_2d` creates a mipmapped material texture from decoded pixels.
///
/// Example:
///     let texture: Texture = upload_texture_2d(&device, &queue, &texture_data);
pub fn upload_texture_2d(
    device: &ProtocolObject<dyn MTLDevice>,
    queue: &ProtocolObject<dyn MTLCommandQueue>,
    data: &TextureData,
) -> Texture {
    let (format, bytes_per_pixel) = pixel_format_for(data.format);
    let width: usize = data.width as usize;
    let height: usize = data.height as usize;
    let spec: TextureSpec = TextureSpec {
        texture_type: MTLTextureType::Type2D,
        format,
        width,
        height,
        depth: 1,
        array_length: 1,
        mip_count: mip_count_for(width, height),
        usage: MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget,
    };
    let texture: Texture = create_texture(device, &spec, "material");
    upload_slices(device, queue, &texture, &data.pixels, bytes_per_pixel, 1);
    texture
}

/// `tests` checks texture mip-chain lengths.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | texture size | 1x1 | `test_mip_count_for` |
/// | texture size | non-square | `test_mip_count_for` |
/// | texture size | large square | `test_mip_count_for` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_mip_count_for` checks full mip-chain lengths.
    #[test]
    fn test_mip_count_for() {
        assert_eq!(mip_count_for(1, 1), 1);
        assert_eq!(mip_count_for(256, 128), 9);
        assert_eq!(mip_count_for(2048, 2048), 12);
    }
}
