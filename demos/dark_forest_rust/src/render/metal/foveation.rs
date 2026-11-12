//! `foveation` builds the rasterization rate map that draws the scene at full resolution around
//! the crosshair and at a lower rate toward the screen's edges (W-136, sharp centre, soft edges):
//! Metal's variable rasterization rate, so the scene passes shade fewer pixels where the player
//! looks least. The resolve pass (`resolve_fragment` in `post.metal`) shows the lower-rate edges
//! through a smooth B-spline, so they are soft but never stepped (W-138, no aliasing).
//!
//! Example:
//!     let rate_map: Option<RateMap> = RateMap::new(&device, 3440, 1440);
//!     let (sharp_px, soft_px): (f32, f32) = get_circle_px(3440, 1440);

use std::ptr::NonNull;

use glam::Vec2;
use objc2::AllocAnyThread;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBuffer, MTLCoordinate2D, MTLDevice, MTLRasterizationRateLayerDescriptor,
    MTLRasterizationRateMap, MTLRasterizationRateMapDescriptor, MTLResourceOptions, MTLSize,
};

/// `RateMap` is a rasterization rate map with its parameter buffer, which shaders decode; the
/// physical size of the textures its passes draw into; and how far a screen pixel in the
/// full-rate centre lies from its physical pixel.
pub struct RateMap {
    pub map: Retained<ProtocolObject<dyn MTLRasterizationRateMap>>,
    pub parameters: Retained<ProtocolObject<dyn MTLBuffer>>,
    pub physical_width_px: usize,
    pub physical_height_px: usize,
    pub centre_offset_px: Vec2,
}

/// `QUALITY_SAMPLE_COUNT` is how many quality samples the map has along each axis.
const QUALITY_SAMPLE_COUNT: usize = 64;
/// `SHARP_RADIUS_FRAC_OF_HEIGHT` is the half width of the full-rate centre around the crosshair,
/// as a share of the screen's height (288 px of a 1440 px screen).
const SHARP_RADIUS_FRAC_OF_HEIGHT: f32 = 0.2;
/// `EDGE_QUALITY_FRAC` is the rasterization quality the rate eases down to toward the edges (Metal
/// turns quality into a rate: 0.3 gives about a third of the pixels along that axis).
const EDGE_QUALITY_FRAC: f32 = 0.3;
/// `EDGE_REACH_FRAC` is how far out along each axis, as a share of that half of the screen, the
/// rate has reached `EDGE_QUALITY_FRAC`.
const EDGE_REACH_FRAC: f32 = 0.7;
/// `SOFT_RAMP_FRAC` is how far out, as a share of the way from the sharp centre to the nearer
/// axis's edge reach, the resolve pass's smooth upsampling has fully replaced the sharp tap.
const SOFT_RAMP_FRAC: f32 = 0.5;

/// `get_falloff_px` is where the full rate ends and where it reaches `EDGE_QUALITY_FRAC` along an
/// axis `size_px` long, in screen pixels from the centre, for a screen `height_px` high.
fn get_falloff_px(size_px: usize, height_px: usize) -> (f32, f32) {
    let sharp_px: f32 = SHARP_RADIUS_FRAC_OF_HEIGHT * height_px as f32;
    let edge_px: f32 = EDGE_REACH_FRAC * 0.5 * size_px as f32;
    (sharp_px, edge_px.max(sharp_px + 1.0))
}

/// `get_circle_px` is the sharp circle's radius and where the resolve pass's smooth upsampling
/// is complete, in screen pixels from the centre, for a `width_px` by `height_px` screen.
pub fn get_circle_px(width_px: usize, height_px: usize) -> (f32, f32) {
    let (sharp_px, edge_px): (f32, f32) = get_falloff_px(width_px.min(height_px), height_px);
    (sharp_px, sharp_px + SOFT_RAMP_FRAC * (edge_px - sharp_px))
}

/// `get_quality` is the rasterization quality at `offset_px` from the screen's centre along one
/// axis: 1 within `sharp_px`, easing to `edge_quality` at `edge_px` and beyond.
fn get_quality(offset_px: f32, sharp_px: f32, edge_px: f32, edge_quality: f32) -> f32 {
    let t: f32 = ((offset_px - sharp_px) / (edge_px - sharp_px)).clamp(0.0, 1.0);
    1.0 + (edge_quality - 1.0) * t * t * (3.0 - 2.0 * t)
}

/// `get_axis_qualities` is the quality samples along an axis `size_px` long, centre to centre.
fn get_axis_qualities(size_px: usize, height_px: usize) -> Vec<f32> {
    let (sharp_px, edge_px): (f32, f32) = get_falloff_px(size_px, height_px);
    (0..QUALITY_SAMPLE_COUNT)
        .map(|i: usize| {
            let at_px: f32 = (i as f32 + 0.5) / QUALITY_SAMPLE_COUNT as f32 * size_px as f32;
            let offset_px: f32 = (at_px - 0.5 * size_px as f32).abs();
            get_quality(offset_px, sharp_px, edge_px, EDGE_QUALITY_FRAC)
        })
        .collect()
}

impl RateMap {
    /// `new` makes the rate map for a `width_px` by `height_px` screen, or `None` where the GPU
    /// has no rasterization rate maps.
    pub fn new(
        device: &ProtocolObject<dyn MTLDevice>,
        width_px: usize,
        height_px: usize,
    ) -> Option<RateMap> {
        if !unsafe { device.supportsRasterizationRateMapWithLayerCount(1) } {
            return None;
        }
        let mut horizontal: Vec<f32> = get_axis_qualities(width_px, height_px);
        let mut vertical: Vec<f32> = get_axis_qualities(height_px, height_px);
        let layer: Retained<MTLRasterizationRateLayerDescriptor> = unsafe {
            MTLRasterizationRateLayerDescriptor::initWithSampleCount_horizontal_vertical(
                MTLRasterizationRateLayerDescriptor::alloc(),
                MTLSize {
                    width: QUALITY_SAMPLE_COUNT,
                    height: QUALITY_SAMPLE_COUNT,
                    depth: 0,
                },
                NonNull::new(horizontal.as_mut_ptr()).expect("a vector's pointer is not null"),
                NonNull::new(vertical.as_mut_ptr()).expect("a vector's pointer is not null"),
            )
        };
        let descriptor: Retained<MTLRasterizationRateMapDescriptor> = unsafe {
            MTLRasterizationRateMapDescriptor::rasterizationRateMapDescriptorWithScreenSize_layer(
                MTLSize {
                    width: width_px,
                    height: height_px,
                    depth: 0,
                },
                &layer,
            )
        };
        let map = device.newRasterizationRateMapWithDescriptor(&descriptor)?;
        let physical: MTLSize = unsafe { map.physicalSizeForLayer(0) };
        let parameters = device.newBufferWithLength_options(
            map.parameterBufferSizeAndAlign().size,
            MTLResourceOptions::StorageModeShared,
        )?;
        unsafe { map.copyParameterDataToBuffer_offset(&parameters, 0) };
        let centre_px: MTLCoordinate2D = MTLCoordinate2D {
            x: (width_px / 2) as f32 + 0.5,
            y: (height_px / 2) as f32 + 0.5,
        };
        let centre_physical_px: MTLCoordinate2D =
            unsafe { map.mapScreenToPhysicalCoordinates_forLayer(centre_px, 0) };
        Some(RateMap {
            map,
            parameters,
            physical_width_px: physical.width,
            physical_height_px: physical.height,
            centre_offset_px: Vec2::new(
                (centre_px.x - centre_physical_px.x).round(),
                (centre_px.y - centre_physical_px.y).round(),
            ),
        })
    }
}

/// `tests` checks the quality falloff and the resolve's circle.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | offset | inside the sharp centre | `test_quality_is_full_inside_the_centre` |
/// | offset | between the centre and the edge reach | `test_quality_falls_toward_the_edge` |
/// | offset | at or past the edge reach | `test_quality_falls_toward_the_edge` |
/// | screen | ultrawide 3440x1440 | `test_circle_ramp_lies_inside_the_edge_reach` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_quality_is_full_inside_the_centre` checks full quality up to the centre's half width.
    #[test]
    fn test_quality_is_full_inside_the_centre() {
        assert_eq!(get_quality(0.0, 288.0, 1204.0, 0.3), 1.0);
        assert_eq!(get_quality(288.0, 288.0, 1204.0, 0.3), 1.0);
    }

    /// `test_quality_falls_toward_the_edge` checks that quality falls steadily from the centre to
    /// the edge quality, and stays there beyond.
    #[test]
    fn test_quality_falls_toward_the_edge() {
        let qualities: Vec<f32> = (0..=10)
            .map(|i: usize| get_quality(288.0 + 91.6 * i as f32, 288.0, 1204.0, 0.3))
            .collect();
        assert!(qualities.windows(2).all(|pair| pair[1] < pair[0]));
        assert!((qualities[10] - 0.3).abs() < 1e-5);
        assert!((get_quality(1720.0, 288.0, 1204.0, 0.3) - 0.3).abs() < 1e-6);
    }

    /// `test_circle_ramp_lies_inside_the_edge_reach` checks the resolve's ramp on an ultrawide
    /// screen: it starts at the sharp centre and ends before the vertical edge reach.
    #[test]
    fn test_circle_ramp_lies_inside_the_edge_reach() {
        let (sharp_px, soft_px): (f32, f32) = get_circle_px(3440, 1440);
        assert_eq!(sharp_px, 288.0);
        assert!(soft_px > sharp_px && soft_px < EDGE_REACH_FRAC * 720.0);
    }
}
