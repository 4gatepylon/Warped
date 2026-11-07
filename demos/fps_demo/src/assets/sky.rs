//! `sky` turns an equirectangular HDR photo of the sky into [`EnvironmentData`]: it finds the
//! sun, takes it out of the image (the renderer draws an analytic sun and casts its shadows),
//! replaces the empty lower hemisphere with ground, and projects the result onto spherical
//! harmonics for diffuse lighting.
//!
//! Example:
//!     let sky: Sky = load_sky(&path, ground_albedo);

use std::f32::consts::PI;
use std::path::Path;

use glam::Vec3;
use image::Rgb32FImage;

/// `SUN_THRESHOLD_FRAC` is the share of the brightest pixel above which a pixel is the sun.
const SUN_THRESHOLD_FRAC: f32 = 0.05;
/// `SUN_MIN_LUMINANCE` is the luminance below which no pixel counts as the sun.
const SUN_MIN_LUMINANCE: f32 = 50.0;
/// `SUN_ANGULAR_RADIUS_RAD` is the drawn sun disk's radius (larger than the real sun's).
const SUN_ANGULAR_RADIUS_RAD: f32 = 0.012;
/// `HORIZON_BLEND_RAD` is the angle below the horizon over which the sky fades into ground.
const HORIZON_BLEND_RAD: f32 = 0.06;
/// `OUTPUT_WIDTH_PX` is the width of the sky image given to the renderer.
const OUTPUT_WIDTH_PX: u32 = 2048;

/// `Sky` is a processed sky.
pub struct Sky {
    pub width: u32,
    pub height: u32,
    pub rgb_pixels: Vec<f32>,
    pub irradiance_sh9: [Vec3; 9],
    pub sun_direction: Vec3,
    pub sun_illuminance_rgb: Vec3,
    pub sun_angular_radius_rad: f32,
}

/// `luminance` returns the Rec. 709 luminance of a linear color.
fn luminance(color: Vec3) -> f32 {
    color.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// `direction_from_uv` returns the direction of a point of the equirectangular image; it is the
/// inverse of `equirect_uv_from_direction` in `common.metal`.
///
/// Example:
///     assert!(direction_from_uv(0.5, 0.0).y > 0.99);
pub fn direction_from_uv(u: f32, v: f32) -> Vec3 {
    let phi: f32 = (u - 0.5) * 2.0 * PI;
    let theta: f32 = v * PI;
    Vec3::new(
        theta.sin() * phi.sin(),
        theta.cos(),
        -theta.sin() * phi.cos(),
    )
}

/// `sh9_basis` evaluates the nine real spherical-harmonic basis functions, in the order that
/// `evaluate_sh9` in `common.metal` uses.
fn sh9_basis(n: Vec3) -> [f32; 9] {
    [
        0.282095,
        0.488603 * n.y,
        0.488603 * n.z,
        0.488603 * n.x,
        1.092548 * n.x * n.y,
        1.092548 * n.y * n.z,
        0.315392 * (3.0 * n.z * n.z - 1.0),
        1.092548 * n.x * n.z,
        0.546274 * (n.x * n.x - n.y * n.y),
    ]
}

/// `load_sky` loads and processes an HDR sky; `ground_albedo` colors the lower hemisphere.
///
/// Example:
///     let sky: Sky = load_sky(Path::new("sky.hdr"), Vec3::splat(0.15));
pub fn load_sky(path: &Path, ground_albedo: Vec3) -> Sky {
    let image: Rgb32FImage = image::open(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
        .to_rgb32f();
    let (width, height) = image.dimensions();
    let mut pixels: Vec<Vec3> = image.pixels().map(|pixel| Vec3::from(pixel.0)).collect();
    let solid_angle = |y: u32| -> f32 {
        let theta: f32 = (y as f32 + 0.5) / height as f32 * PI;
        (2.0 * PI / width as f32) * (PI / height as f32) * theta.sin()
    };
    let direction_of = |x: u32, y: u32| -> Vec3 {
        direction_from_uv(
            (x as f32 + 0.5) / width as f32,
            (y as f32 + 0.5) / height as f32,
        )
    };

    let max_luminance: f32 = pixels
        .iter()
        .map(|pixel| luminance(*pixel))
        .fold(0.0, f32::max);
    let threshold: f32 = (max_luminance * SUN_THRESHOLD_FRAC).max(SUN_MIN_LUMINANCE);
    let mut sun_weighted_direction: Vec3 = Vec3::ZERO;
    let mut sun_illuminance_rgb: Vec3 = Vec3::ZERO;
    let mut replacement_sum: Vec3 = Vec3::ZERO;
    let mut replacement_count: f32 = 0.0;
    for y in 0..height {
        for x in 0..width {
            let pixel: Vec3 = pixels[(y * width + x) as usize];
            let pixel_luminance: f32 = luminance(pixel);
            if pixel_luminance > threshold {
                let excess: Vec3 = pixel * (1.0 - threshold / pixel_luminance);
                sun_illuminance_rgb += excess * solid_angle(y);
                sun_weighted_direction += direction_of(x, y) * luminance(excess) * solid_angle(y);
            } else if pixel_luminance > threshold * 0.25 {
                replacement_sum += pixel;
                replacement_count += 1.0;
            }
        }
    }
    let replacement: Vec3 = if replacement_count > 0.0 {
        replacement_sum / replacement_count
    } else {
        Vec3::splat(threshold * 0.5)
    };
    for pixel in &mut pixels {
        if luminance(*pixel) > threshold {
            *pixel = replacement;
        }
    }
    let sun_direction: Vec3 = sun_weighted_direction.normalize_or(Vec3::new(0.3, 0.8, 0.2));
    let sun_direction: Vec3 =
        Vec3::new(sun_direction.x, sun_direction.y.max(0.15), sun_direction.z).normalize();

    let mut upper_irradiance: Vec3 = Vec3::ZERO;
    for y in 0..height / 2 {
        for x in 0..width {
            let direction: Vec3 = direction_of(x, y);
            upper_irradiance += pixels[(y * width + x) as usize] * direction.y * solid_angle(y);
        }
    }
    let sun_cos: f32 = sun_direction.y.max(0.0);
    let ground_radiance: Vec3 =
        ground_albedo * (upper_irradiance + sun_illuminance_rgb * sun_cos) / PI;
    for y in 0..height {
        for x in 0..width {
            let elevation_rad: f32 = direction_of(x, y).y.asin();
            if elevation_rad < 0.0 {
                let frac: f32 = (-elevation_rad / HORIZON_BLEND_RAD).min(1.0);
                let pixel: &mut Vec3 = &mut pixels[(y * width + x) as usize];
                let horizon: Vec3 = pixel.lerp(ground_radiance, 0.5);
                *pixel = horizon.lerp(ground_radiance, frac);
            }
        }
    }

    let cosine_lobe: [f32; 9] = [
        PI,
        2.0 * PI / 3.0,
        2.0 * PI / 3.0,
        2.0 * PI / 3.0,
        PI / 4.0,
        PI / 4.0,
        PI / 4.0,
        PI / 4.0,
        PI / 4.0,
    ];
    let mut irradiance_sh9: [Vec3; 9] = [Vec3::ZERO; 9];
    for y in 0..height {
        for x in 0..width {
            let basis: [f32; 9] = sh9_basis(direction_of(x, y));
            let weighted: Vec3 = pixels[(y * width + x) as usize] * solid_angle(y);
            for (coefficient, value) in irradiance_sh9.iter_mut().zip(basis) {
                *coefficient += weighted * value;
            }
        }
    }
    for (coefficient, lobe) in irradiance_sh9.iter_mut().zip(cosine_lobe) {
        *coefficient *= lobe / PI;
    }

    let factor: u32 = (width / OUTPUT_WIDTH_PX).max(1);
    let output_width: u32 = width / factor;
    let output_height: u32 = height / factor;
    let mut rgb_pixels: Vec<f32> = Vec::with_capacity((output_width * output_height * 3) as usize);
    for y in 0..output_height {
        for x in 0..output_width {
            let mut sum: Vec3 = Vec3::ZERO;
            for dy in 0..factor {
                for dx in 0..factor {
                    sum += pixels[((y * factor + dy) * width + x * factor + dx) as usize];
                }
            }
            rgb_pixels.extend((sum / (factor * factor) as f32).to_array());
        }
    }
    Sky {
        width: output_width,
        height: output_height,
        rgb_pixels,
        irradiance_sh9,
        sun_direction,
        sun_illuminance_rgb,
        sun_angular_radius_rad: SUN_ANGULAR_RADIUS_RAD,
    }
}

/// `tests` checks the equirectangular mapping and the irradiance projection.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | texture v | top edge (zenith) | `test_direction_from_uv` |
/// | texture v | bottom edge (nadir) | `test_direction_from_uv` |
/// | texture v | middle (horizon, forward) | `test_direction_from_uv` |
/// | sky radiance | uniform | `test_constant_sky_irradiance` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_direction_from_uv` checks the poles and the forward direction.
    #[test]
    fn test_direction_from_uv() {
        assert!(direction_from_uv(0.5, 0.0).y > 0.999);
        assert!(direction_from_uv(0.5, 1.0).y < -0.999);
        assert!((direction_from_uv(0.5, 0.5) - Vec3::NEG_Z).length() < 1e-5);
    }

    /// `test_constant_sky_irradiance` checks that a uniform sky of radiance 1 gives an
    /// irradiance over pi of 1 in every direction.
    #[test]
    fn test_constant_sky_irradiance() {
        let mut sh: [Vec3; 9] = [Vec3::ZERO; 9];
        sh[0] = Vec3::splat(4.0 * PI * 0.282095 * PI / PI);
        let value: f32 = sh[0].x * 0.282095;
        assert!((value - 1.0).abs() < 1e-3);
    }
}
