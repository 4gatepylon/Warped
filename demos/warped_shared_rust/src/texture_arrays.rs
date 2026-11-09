//! `texture_arrays.rs` makes mipmapped Metal texture arrays from image files: the surface textures
//! of a level's ground and structures (each level's `voxel_textures.rs`), the maps' models
//! (`render_mesh.rs`) and the guns held in hand (`viewmodel.rs`).
//!
//! Interface: `make_mips`, `make_array`.
//!
//! Example: `let albedo = make_array(device, MTLPixelFormat::RGBA8Unorm_sRGB, 4, &[make_mips(&path, 2048)?])?;`

use std::error::Error;
use std::path::Path;
use std::ptr::NonNull;

use image::RgbaImage;
use image::imageops::FilterType;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLDevice, MTLOrigin, MTLPixelFormat, MTLRegion, MTLSize, MTLStorageMode, MTLTexture,
    MTLTextureDescriptor, MTLTextureType, MTLTextureUsage,
};

use crate::fps_demo_assets::read_asset_file;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_RGBA_PIXEL: usize = 4;

/// `make_mips` decodes one map (read with `read_asset_file`), resizes it to `side_px` square and
/// halves it down to 1x1.
///
/// Example: `let mips = make_mips(&path, 2048)?;`
pub fn make_mips(path: &Path, side_px: u32) -> std::result::Result<Vec<RgbaImage>, String> {
    let mut image: RgbaImage = image::load_from_memory(&read_asset_file(path)?)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8();
    if image.dimensions() != (side_px, side_px) {
        image = image::imageops::resize(&image, side_px, side_px, FilterType::Triangle);
    }
    let mut mips: Vec<RgbaImage> = vec![image];
    while mips[mips.len() - 1].width() > 1 {
        let side: u32 = mips[mips.len() - 1].width() / 2;
        mips.push(image::imageops::resize(
            &mips[mips.len() - 1],
            side,
            side,
            FilterType::Triangle,
        ));
    }
    Ok(mips)
}

/// `make_array` makes a shared texture array and fills it with each layer's mips, keeping the
/// first `channels` bytes of each pixel.
///
/// Example: `let albedo = make_array(device, MTLPixelFormat::RGBA8Unorm_sRGB, 4, &layers)?;`
pub fn make_array(
    device: &ProtocolObject<dyn MTLDevice>,
    format: MTLPixelFormat,
    channels: usize,
    layers: &[Vec<RgbaImage>],
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>> {
    let side: usize = layers.first().map_or(1, |mips| mips[0].width() as usize);
    let descriptor: Retained<MTLTextureDescriptor> = MTLTextureDescriptor::new();
    descriptor.setTextureType(MTLTextureType::Type2DArray);
    descriptor.setPixelFormat(format);
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN UPLOADS OUTSIDE THE TEXTURE WILL HAPPEN
    unsafe {
        descriptor.setWidth(side);
        descriptor.setHeight(side);
        descriptor.setArrayLength(layers.len().max(1));
        descriptor.setMipmapLevelCount(layers.first().map_or(1, Vec::len));
    }
    descriptor.setUsage(MTLTextureUsage::ShaderRead);
    descriptor.setStorageMode(MTLStorageMode::Shared);
    let texture = device
        .newTextureWithDescriptor(&descriptor)
        .ok_or("Metal: no memory for the surface textures")?;
    for (slice, mips) in layers.iter().enumerate() {
        for (level, mip) in mips.iter().enumerate() {
            let w: usize = mip.width() as usize;
            let packed_bytes: Vec<u8>;
            let bytes: &[u8] = if channels == BYTES_PER_RGBA_PIXEL {
                mip.as_raw()
            } else {
                packed_bytes = mip
                    .as_raw()
                    .as_chunks::<BYTES_PER_RGBA_PIXEL>()
                    .0
                    .iter()
                    .flat_map(|p| &p[..channels])
                    .copied()
                    .collect();
                &packed_bytes
            };
            let region = MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize {
                    width: w,
                    height: w,
                    depth: 1,
                },
            };
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN READS PAST THE PIXELS WILL HAPPEN
            unsafe {
                texture.replaceRegion_mipmapLevel_slice_withBytes_bytesPerRow_bytesPerImage(
                    region,
                    level,
                    slice,
                    NonNull::new(bytes.as_ptr().cast_mut().cast()).expect("a slice isn't null"),
                    w * channels,
                    w * w * channels,
                )
            };
        }
    }
    Ok(texture)
}
