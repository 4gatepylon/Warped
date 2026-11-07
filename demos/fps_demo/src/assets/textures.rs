//! `textures` decodes image files into [`TextureData`], in parallel and scaled to a size limit.
//!
//! Example:
//!     let textures: Vec<TextureData> = decode_textures(&requests);

use std::path::PathBuf;

use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};

use crate::render::{TextureData, TextureFormat};

/// `TextureSource` is where a texture's encoded bytes are.
#[derive(Clone)]
pub enum TextureSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// `TextureRequest` is one image to decode, the format to decode it to, and its size limit.
///
/// Example:
///     let request = TextureRequest { source, format: TextureFormat::Rgba8Srgb, max_size_px: 2048 };
#[derive(Clone)]
pub struct TextureRequest {
    pub source: TextureSource,
    pub format: TextureFormat,
    pub max_size_px: u32,
}

/// `decode_texture` decodes one image, shrinking it to fit its size limit.
///
/// Example:
///     let texture: TextureData = decode_texture(&request);
pub fn decode_texture(request: &TextureRequest) -> TextureData {
    let image: DynamicImage = match &request.source {
        TextureSource::File(path) => image::open(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display())),
        TextureSource::Bytes(bytes) => image::load_from_memory(bytes)
            .unwrap_or_else(|error| panic!("cannot decode an embedded image: {error}")),
    };
    let largest_side_px: u32 = image.width().max(image.height());
    let image: DynamicImage = if largest_side_px > request.max_size_px {
        let scale: f32 = request.max_size_px as f32 / largest_side_px as f32;
        let width_px: u32 = ((image.width() as f32 * scale).round() as u32).max(1);
        let height_px: u32 = ((image.height() as f32 * scale).round() as u32).max(1);
        image.resize_exact(width_px, height_px, FilterType::Triangle)
    } else {
        image
    };
    let rgba: RgbaImage = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels: Vec<u8> = match request.format {
        TextureFormat::Rgba8Srgb | TextureFormat::Rgba8Linear => rgba.into_raw(),
        TextureFormat::Rg8Linear => rgba
            .into_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel: &[u8; 4]| [pixel[0], pixel[1]])
            .collect(),
    };
    TextureData {
        width,
        height,
        format: request.format,
        pixels,
    }
}

/// `decode_textures` decodes many images across all CPU cores, keeping their order.
///
/// Example:
///     let textures: Vec<TextureData> = decode_textures(&requests);
pub fn decode_textures(requests: &[TextureRequest]) -> Vec<TextureData> {
    let thread_count: usize = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(requests.len().max(1));
    let chunk_size: usize = requests.len().div_ceil(thread_count).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = requests
            .chunks(chunk_size)
            .map(|chunk: &[TextureRequest]| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(decode_texture)
                        .collect::<Vec<TextureData>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("texture decoding panicked"))
            .collect()
    })
}
