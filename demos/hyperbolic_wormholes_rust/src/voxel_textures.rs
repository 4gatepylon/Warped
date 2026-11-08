//! `voxel_textures.rs` gives the voxel renderer high-resolution surfaces: it loads the fps_demo's
//! Poly Haven 2K PBR sets (`make download-assets`) into mipmapped Metal texture arrays (albedo,
//! normal, and ambient occlusion with roughness and metalness), and says which set, tiling and
//! metalness each map material uses. The sets aren't in git, so without them `check_surface_textures`
//! and `SurfaceTextures::Load` fail with the command that downloads them.
//!
//! Interface: `get_material_surface`, `check_surface_textures`, `SurfaceTextures::Load`.
//!
//! Example: `let textures = SurfaceTextures::Load(&device)?;`

use std::error::Error;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use image::RgbaImage;
use image::imageops::FilterType;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLDevice, MTLOrigin, MTLPixelFormat, MTLRegion, MTLSize, MTLStorageMode, MTLTexture,
    MTLTextureDescriptor, MTLTextureType, MTLTextureUsage,
};
use rayon::prelude::*;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `SURFACE_SETS` is the Poly Haven sets, in the order of the arrays' layers.
const SURFACE_SETS: [&str; 8] = [
    "moon_01",
    "concrete_wall_008",
    "metal_plate",
    "corrugated_iron_02",
    "rusty_metal_02",
    "concrete_pavement_02",
    "concrete_floor_worn_001",
    "moon_meteor_02",
];
const SIZE_PX: u32 = 2048;
const MEAN_LEVEL: usize = 7;

/// `MATERIAL_SURFACES` is each textured material's name, layer, blocks per repeat and metalness.
const MATERIAL_SURFACES: [(&str, usize, f32, f32); 13] = [
    ("regolith", 0, 2.6, 0.0),
    ("dark regolith", 7, 2.6, 0.0),
    ("basalt", 1, 2.0, 0.0),
    ("boulder", 1, 2.0, 0.0),
    ("hull", 2, 2.0, 0.25),
    ("metal", 3, 2.0, 1.0),
    ("solar panel", 2, 1.0, 0.5),
    ("gold foil", 2, 1.0, 1.0),
    ("red paint", 4, 2.0, 0.3),
    ("blue paint", 4, 2.0, 0.3),
    ("hazard paint", 4, 2.0, 0.3),
    ("landing pad", 5, 4.0, 0.0),
    ("floor", 6, 4.0, 0.0),
];

/// `get_material_surface` gives the shader's surface of a material: its layer (-1 for none),
/// blocks per repeat and metalness.
///
/// Example: `let surface = get_material_surface("hull");`
pub fn get_material_surface(name: &str) -> [f32; 4] {
    MATERIAL_SURFACES
        .iter()
        .find(|s| s.0 == name)
        .map_or([-1.0, 1.0, 0.0, 0.0], |s| [s.1 as f32, s.2, s.3, 0.0])
}

/// `SurfaceTextures` is the albedo (sRGB), normal (RG) and AO-roughness-metalness arrays, and each
/// layer's mean linear albedo with w = 1 (all zero without the downloaded sets).
pub struct SurfaceTextures {
    pub albedo: Retained<ProtocolObject<dyn MTLTexture>>,
    pub normal: Retained<ProtocolObject<dyn MTLTexture>>,
    pub arm: Retained<ProtocolObject<dyn MTLTexture>>,
    pub means: Vec<[f32; 4]>,
}

/// `make_mips` decodes one map, resizes it to `side_px` square and halves it down to 1x1.
///
/// Example: `let mips = make_mips(&path, 2048)?;`
pub(crate) fn make_mips(path: &Path, side_px: u32) -> std::result::Result<Vec<RgbaImage>, String> {
    let mut image: RgbaImage = image::open(path)
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

/// `get_mean_albedo` gives the mean linear colour of an sRGB image.
///
/// Example: `let mean = get_mean_albedo(&mips[MEAN_LEVEL]);`
fn get_mean_albedo(image: &RgbaImage) -> [f32; 4] {
    let mut sum: [f32; 3] = [0.0; 3];
    for pixel in image.pixels() {
        for c in 0..3 {
            sum[c] += (f32::from(pixel[c]) / 255.0).powf(2.2);
        }
    }
    let n: f32 = (image.width() * image.height()) as f32;
    [sum[0] / n, sum[1] / n, sum[2] / n, 1.0]
}

/// `make_array` makes a shared texture array and fills it with each layer's mips, keeping the
/// first `channels` bytes of each pixel.
///
/// Example: `let albedo = make_array(device, MTLPixelFormat::RGBA8Unorm_sRGB, 4, &layers)?;`
pub(crate) fn make_array(
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
            let bytes: Vec<u8> = mip
                .pixels()
                .flat_map(|p| p.0[..channels].to_vec())
                .collect();
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
                    NonNull::new(bytes.as_ptr().cast_mut().cast()).expect("a Vec isn't null"),
                    w * channels,
                    w * w * channels,
                )
            };
        }
    }
    Ok(texture)
}

/// `get_fps_demo_folder` gives `demos/fps_demo/`, whose `make download-assets` downloads the sets.
///
/// Example: `let folder: PathBuf = get_fps_demo_folder();`
fn get_fps_demo_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the demo's folder sits in demos/")
        .join("fps_demo")
}

/// `get_surface_texture_paths` gives every map file of the sets, in the arrays' order: all the
/// albedos, then the normals, then the ambient occlusion, roughness and metalness maps.
///
/// Example: `let paths: Vec<PathBuf> = get_surface_texture_paths();`
fn get_surface_texture_paths() -> Vec<PathBuf> {
    let dir: PathBuf = get_fps_demo_folder().join(".assets/textures");
    ["diff", "nor_gl", "arm"]
        .iter()
        .flat_map(|map| {
            SURFACE_SETS
                .iter()
                .map(|id| dir.join(id).join(format!("{id}_{map}_2k.jpg")))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `check_surface_textures` fails, naming a missing file and the command that downloads the sets,
/// when any map file of the sets is missing.
///
/// Example: `check_surface_textures()?;`
pub fn check_surface_textures() -> Result<()> {
    match get_surface_texture_paths().into_iter().find(|path| !path.exists()) {
        Some(path) => Err(format!(
            "Lunar Surface needs its textures, which aren't in git, and {} is missing. Download them once with: cd {} && make download-assets",
            path.display(),
            get_fps_demo_folder().display()
        )
        .into()),
        None => Ok(()),
    }
}

impl SurfaceTextures {
    /// `Load` reads the sets from the fps_demo's `.assets/textures/` onto `device`, and fails like
    /// `check_surface_textures` when any map is missing.
    ///
    /// Example: `let textures = SurfaceTextures::Load(&device)?;`
    #[allow(non_snake_case)]
    pub fn Load(device: &ProtocolObject<dyn MTLDevice>) -> Result<SurfaceTextures> {
        check_surface_textures()?;
        let maps: Vec<Vec<RgbaImage>> = get_surface_texture_paths()
            .par_iter()
            .map(|p| make_mips(p, SIZE_PX))
            .collect::<std::result::Result<_, _>>()?;
        let n: usize = SURFACE_SETS.len();
        Ok(SurfaceTextures {
            albedo: make_array(device, MTLPixelFormat::RGBA8Unorm_sRGB, 4, &maps[..n])?,
            normal: make_array(device, MTLPixelFormat::RG8Unorm, 2, &maps[n..2 * n])?,
            arm: make_array(device, MTLPixelFormat::RGBA8Unorm, 4, &maps[2 * n..])?,
            means: maps[..n]
                .iter()
                .map(|mips| get_mean_albedo(&mips[MEAN_LEVEL]))
                .collect(),
        })
    }
}
