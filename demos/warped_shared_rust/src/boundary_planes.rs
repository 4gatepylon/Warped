//! `boundary_planes.rs` is how the team boundary planes look in every level (W-148, team boundary
//! planes in every level; W-149, shimmer through boundary planes): the planes stand on the edges
//! of the blue and red zones of each level's map (`regions.zones` and `regions.zone_walls`,
//! `common/maps/README.md`) and glow faintly in their team's colour, and where one covers the
//! screen the picture shimmers like heat haze, a screen-space effect that changes a little every
//! frame. Each level finds where its planes cover the screen; their look and the haze are
//! `boundary_planes.metal` (`BOUNDARY_PLANES_METAL`, which each level prepends to its shader
//! source), and the haze's clock is here.
//!
//! Interface: `BOUNDARY_PLANES_METAL`, `SHIMMER_CLOCK_PERIOD_S`, `get_shimmer_clock_s`.
//!
//! Example: `uniforms.zone_walls.look[3] = get_shimmer_clock_s(world.sky_time_s);`
//!
//! Must know:
//! - Gotchas: the haze's noise repeats only after `SHIMMER_CLOCK_PERIOD_S` (its lattice wraps
//!   then, and each octave rises a whole number of cells a second), so the clock wraps without a
//!   jump and stays small enough for `f32`.

/// `BOUNDARY_PLANES_METAL` is `boundary_planes.metal`, for a level to prepend to its shader source.
pub const BOUNDARY_PLANES_METAL: &str = include_str!("boundary_planes.metal");
/// `SHIMMER_CLOCK_PERIOD_S` is how long the haze takes to repeat exactly: `boundary_planes.metal`'s
/// time lattice (`BOUNDARY_HAZE_TIME_CELLS` cells of `BOUNDARY_HAZE_S_PER_TIME_CELL`) times its
/// space lattice's `BOUNDARY_HAZE_SPACE_CELLS` over the time lattice's.
pub const SHIMMER_CLOCK_PERIOD_S: f64 = 1024.0;

/// `get_shimmer_clock_s` gives the haze's clock at `time_s` (any clock in seconds, such as the UTC
/// time the sky uses): `time_s` wrapped to `SHIMMER_CLOCK_PERIOD_S`.
///
/// Example: `let clock_s: f32 = get_shimmer_clock_s(world.sky_time_s);`
pub fn get_shimmer_clock_s(time_s: f64) -> f32 {
    time_s.rem_euclid(SHIMMER_CLOCK_PERIOD_S) as f32
}

#[cfg(test)]
mod tests {
    //! Tests of the boundary planes' haze, on the GPU (the shader is its only implementation).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Clock | inside the period, past it (wraps), before 0 (wraps up) | `the_clock_wraps` |
    //! | Haze at the clock's wrap | 1 ms before the period and at 0 (nearly the same move: no jump) | `the_haze_is_bounded_smooth_and_changing` (macOS) |
    //! | Size | many pixels and times (within the strength), and not all small | `the_haze_is_bounded_smooth_and_changing` (macOS) |
    //! | Across the screen | neighbouring pixels (moves differ by under a pixel, so the picture never folds over itself) | `the_haze_is_bounded_smooth_and_changing` (macOS) |
    //! | Over time | one frame later (changes a little, under 3 px), a second later (changes clearly) | `the_haze_is_bounded_smooth_and_changing` (macOS) |

    use super::*;

    #[test]
    fn the_clock_wraps() {
        assert_eq!(get_shimmer_clock_s(3.5), 3.5);
        let utc_s: f64 = 1_794_187_309.25;
        assert!(
            (get_shimmer_clock_s(utc_s) - (utc_s % SHIMMER_CLOCK_PERIOD_S) as f32).abs() < 1e-3
        );
        assert!((get_shimmer_clock_s(-2.0) - 1022.0).abs() < 1e-6);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_haze_is_bounded_smooth_and_changing() {
        use std::ptr::NonNull;

        use objc2_foundation::NSString;
        use objc2_metal::{
            MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
            MTLComputeCommandEncoder, MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary,
            MTLResourceOptions, MTLSize,
        };

        const KERNEL: &str = "kernel void haze_test(device float4 *samples [[buffer(0)]], \
            uint i [[thread_position_in_grid]]) { float4 s = samples[i]; \
            samples[i] = float4(get_boundary_haze_px(s.xy, 1440.0, s.z, float4(1.0, 0.03, 3.0, 1.0)), 0.0, 0.0); }";
        const HAZE_PX: f32 = 14.0;
        const ROUNDING_PX: f32 = 1e-3;
        const MAX_NEIGHBOUR_CHANGE_PX: f32 = 1.0;
        const MAX_FRAME_CHANGE_PX: f32 = 3.0;
        const FRAME_S: f32 = 1.0 / 60.0;
        const PERIOD_S: f32 = SHIMMER_CLOCK_PERIOD_S as f32;
        let mut samples: Vec<[f32; 4]> = Vec::new();
        for i in 0..400 {
            let (x, y, t) = (
                (i * 37 % 2560) as f32,
                (i * 53 % 1440) as f32,
                (i as f32 * 2.3) % 900.0,
            );
            for (dx, dt) in [(0.0, 0.0), (1.0, 0.0), (0.0, FRAME_S), (0.0, 1.0)] {
                samples.push([x + dx, y, t + dt, 0.0]);
            }
            samples.push([x, y, PERIOD_S - 1e-3, 0.0]);
            samples.push([x, y, 0.0, 0.0]);
        }
        let device = MTLCreateSystemDefaultDevice().expect("a Metal GPU");
        let source: String = format!("{BOUNDARY_PLANES_METAL}\n{KERNEL}");
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(&source), None)
            .expect("boundary_planes.metal compiles");
        let function = library
            .newFunctionWithName(&NSString::from_str("haze_test"))
            .expect("the test kernel");
        let pipeline = device
            .newComputePipelineStateWithFunction_error(&function)
            .expect("a pipeline");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY PAST THE SAMPLES WILL HAPPEN
        let buffer = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new(samples.as_ptr().cast_mut().cast()).expect("a slice isn't null"),
                std::mem::size_of_val(samples.as_slice()),
                MTLResourceOptions::StorageModeShared,
            )
        }
        .expect("a buffer");
        let queue = device.newCommandQueue().expect("a command queue");
        let command_buffer = queue.commandBuffer().expect("a command buffer");
        let encoder = command_buffer
            .computeCommandEncoder()
            .expect("a compute encoder");
        encoder.setComputePipelineState(&pipeline);
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE KERNEL READING THE WRONG BUFFER WILL HAPPEN
        unsafe { encoder.setBuffer_offset_atIndex(Some(&buffer), 0, 0) };
        let one: MTLSize = MTLSize {
            width: 1,
            height: 1,
            depth: 1,
        };
        encoder.dispatchThreads_threadsPerThreadgroup(
            MTLSize {
                width: samples.len(),
                ..one
            },
            one,
        );
        encoder.endEncoding();
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE SAMPLES WILL HAPPEN
        let moves: &[[f32; 4]] =
            unsafe { std::slice::from_raw_parts(buffer.contents().as_ptr().cast(), samples.len()) };
        let distance = |a: &[f32; 4], b: &[f32; 4]| (a[0] - b[0]).hypot(a[1] - b[1]);
        let mut largest_px: f32 = 0.0;
        let mut later_sum_px: f32 = 0.0;
        for group in moves.chunks(6) {
            let [
                here,
                right,
                next_frame,
                second_later,
                before_wrap,
                after_wrap,
            ] = [0, 1, 2, 3, 4, 5].map(|k| &group[k]);
            assert!(
                here[0].abs() <= HAZE_PX + ROUNDING_PX && here[1].abs() <= HAZE_PX + ROUNDING_PX,
                "{here:?}"
            );
            largest_px = largest_px.max(here[0].abs());
            assert!(
                distance(here, right) < MAX_NEIGHBOUR_CHANGE_PX,
                "{here:?} {right:?}"
            );
            assert!(
                distance(here, next_frame) < MAX_FRAME_CHANGE_PX,
                "{here:?} {next_frame:?}"
            );
            later_sum_px += distance(here, second_later);
            assert!(
                distance(before_wrap, after_wrap) < 0.2,
                "{before_wrap:?} {after_wrap:?}"
            );
        }
        assert!(largest_px > 0.4 * HAZE_PX, "{largest_px}");
        assert!(later_sum_px / 400.0 > 0.2 * HAZE_PX, "{later_sum_px}");
    }
}
