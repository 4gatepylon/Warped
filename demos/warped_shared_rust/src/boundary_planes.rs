//! `boundary_planes.rs` is how the team boundary planes look in every level (W-148, team boundary
//! planes in every level; W-149, shimmer through boundary planes): the planes stand on the edges
//! of the blue and red zones of each level's map (`regions.zones` and `regions.zone_walls`,
//! `common/maps/README.md`), glow faintly in their team's colour, and what is seen through one
//! shimmers a little until you cross it. Each level finds where its planes are; their look and the
//! shimmer are `boundary_planes.metal` (`BOUNDARY_PLANES_METAL`, which each level prepends to its
//! shader source), and the shimmer's clock and a CPU copy of it for tests are here.
//!
//! Interface: `BOUNDARY_PLANES_METAL`, `SHIMMER_CLOCK_PERIOD_S`, `get_shimmer_clock_s`,
//! `get_boundary_shimmer`, `BOUNDARY_SHIMMER_RAD`.
//!
//! Example: `uniforms.zone_walls.look[3] = get_shimmer_clock_s(world.sky_time_s);`
//!
//! Must know:
//! - Gotchas: the shimmer's waves each repeat a whole number of times in
//!   `SHIMMER_CLOCK_PERIOD_S`, so the clock wraps without a jump and stays small enough for `f32`.

/// `BOUNDARY_PLANES_METAL` is `boundary_planes.metal`, for a level to prepend to its shader source.
pub const BOUNDARY_PLANES_METAL: &str = include_str!("boundary_planes.metal");
/// `SHIMMER_CLOCK_PERIOD_S` is how long the shimmer takes to repeat exactly.
pub const SHIMMER_CLOCK_PERIOD_S: f64 = 10.0;
/// `BOUNDARY_SHIMMER_RAD` is `boundary_planes.metal`'s largest turn of the view through a plane.
pub const BOUNDARY_SHIMMER_RAD: f32 = 0.0025;
const SHORTEST_WAVE_M: f32 = 0.6;
const WAVES_PER_M: [f32; 4] = [1.1, 0.83, 0.71, 1.27];
const WAVE_HZ: [f32; 4] = [0.9, 0.7, 1.1, 0.8];

/// `get_shimmer_clock_s` gives the shimmer's clock at `time_s` (any clock in seconds, such as the
/// UTC time the sky uses): `time_s` wrapped to `SHIMMER_CLOCK_PERIOD_S`.
///
/// Example: `let clock_s: f32 = get_shimmer_clock_s(world.sky_time_s);`
pub fn get_shimmer_clock_s(time_s: f64) -> f32 {
    time_s.rem_euclid(SHIMMER_CLOCK_PERIOD_S) as f32
}

/// `smoothstep` is Metal's: 0 below `low`, 1 above `high`, and a smooth cubic between.
///
/// Example: `let fade = smoothstep(0.15, 0.3, footprint_m);`
fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t: f32 = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `get_boundary_shimmer` is `boundary_planes.metal`'s function of the same name on the CPU: how
/// far what is seen through a plane is turned (radians across and up the plane) where the view
/// crosses it at `on_plane_m`, at the shimmer clock `clock_s`, faded out where one pixel covers
/// `footprint_m` of the plane, more than about half the shortest wave.
///
/// Example: `let [across_rad, up_rad] = get_boundary_shimmer([2.0, 1.5], 3.0, 0.01);`
pub fn get_boundary_shimmer(on_plane_m: [f32; 2], clock_s: f32, footprint_m: f32) -> [f32; 2] {
    let [x, y] = on_plane_m;
    let along: [f32; 4] = [x + y, x - y, y, x];
    let phase: [f32; 4] = std::array::from_fn(|i| {
        std::f32::consts::TAU * (WAVES_PER_M[i] * along[i] + WAVE_HZ[i] * clock_s)
    });
    let wave: [f32; 2] = [
        0.5 * (phase[0].sin() + phase[1].sin()),
        0.5 * (phase[2].sin() + phase[3].cos()),
    ];
    let fade: f32 = 1.0 - smoothstep(0.25 * SHORTEST_WAVE_M, 0.5 * SHORTEST_WAVE_M, footprint_m);
    wave.map(|w| BOUNDARY_SHIMMER_RAD * fade * w)
}

#[cfg(test)]
mod tests {
    //! Tests of the boundary planes' shimmer.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Clock | inside the period, past it (wraps), before 0 (wraps up) | `the_clock_wraps_without_a_jump` |
    //! | Shimmer over the period | at 0 and at the period (the same turn: no jump when the clock wraps) | `the_clock_wraps_without_a_jump` |
    //! | Size | every place and time (within the largest turn), a fine pixel (full), a pixel over half the shortest wave (none) | `the_shimmer_stays_small_and_fades_where_pixels_are_coarse` |
    //! | Smoothness | neighbouring points 1 cm apart (turns differ little: no flicker between pixels) | `the_shimmer_is_smooth` |
    //! | Shader against CPU | several places and times (within float rounding) | `the_shader_matches_the_cpu` (macOS) |

    use super::*;

    const FINE_FOOTPRINT_M: f32 = 0.001;
    const SMOOTH_STEP_M: f32 = 0.01;

    /// `get_samples` gives places on a plane (meters) and clock times (seconds) to test at.
    ///
    /// Example: `for ([x, y], clock_s) in get_samples() { ... }`
    fn get_samples() -> Vec<([f32; 2], f32)> {
        (0..200)
            .map(|i| {
                let k: f32 = i as f32;
                (
                    [(k * 0.37).sin() * 30.0, (k * 0.11) % 6.0],
                    (k * 0.29) % 10.0,
                )
            })
            .collect()
    }

    #[test]
    fn the_clock_wraps_without_a_jump() {
        assert_eq!(get_shimmer_clock_s(3.5), 3.5);
        assert!((get_shimmer_clock_s(1_794_187_309.25) - 9.25).abs() < 1e-3);
        assert!((get_shimmer_clock_s(-2.0) - 8.0).abs() < 1e-6);
        for (place, _) in get_samples() {
            let start: [f32; 2] = get_boundary_shimmer(place, 0.0, FINE_FOOTPRINT_M);
            let end: [f32; 2] =
                get_boundary_shimmer(place, SHIMMER_CLOCK_PERIOD_S as f32, FINE_FOOTPRINT_M);
            for i in 0..2 {
                assert!(
                    (start[i] - end[i]).abs() < 1e-6,
                    "{place:?}: {start:?} {end:?}"
                );
            }
        }
    }

    #[test]
    fn the_shimmer_stays_small_and_fades_where_pixels_are_coarse() {
        let mut largest_rad: f32 = 0.0;
        for (place, clock_s) in get_samples() {
            let turn: [f32; 2] = get_boundary_shimmer(place, clock_s, FINE_FOOTPRINT_M);
            assert!(
                turn.iter().all(|x| x.abs() <= BOUNDARY_SHIMMER_RAD),
                "{turn:?}"
            );
            largest_rad = largest_rad.max(turn[0].abs()).max(turn[1].abs());
            assert_eq!(
                get_boundary_shimmer(place, clock_s, SHORTEST_WAVE_M),
                [0.0; 2]
            );
        }
        assert!(largest_rad > 0.5 * BOUNDARY_SHIMMER_RAD, "{largest_rad}");
    }

    #[test]
    fn the_shimmer_is_smooth() {
        let largest_slope_rad_per_m: f32 =
            BOUNDARY_SHIMMER_RAD * std::f32::consts::TAU * 2.0 * WAVES_PER_M[3];
        for ([x, y], clock_s) in get_samples() {
            let here: [f32; 2] = get_boundary_shimmer([x, y], clock_s, FINE_FOOTPRINT_M);
            let next: [f32; 2] =
                get_boundary_shimmer([x + SMOOTH_STEP_M, y], clock_s, FINE_FOOTPRINT_M);
            for i in 0..2 {
                assert!((here[i] - next[i]).abs() <= largest_slope_rad_per_m * SMOOTH_STEP_M);
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_shader_matches_the_cpu() {
        use std::ptr::NonNull;

        use objc2_foundation::NSString;
        use objc2_metal::{
            MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
            MTLComputeCommandEncoder, MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary,
            MTLResourceOptions, MTLSize,
        };

        const KERNEL: &str = "kernel void shimmer_test(device float4 *samples [[buffer(0)]], \
            uint i [[thread_position_in_grid]]) { float4 s = samples[i]; \
            samples[i].xy = get_boundary_shimmer(s.xy, s.z, s.w); }";
        const SHADER_TOLERANCE_RAD: f32 = 1e-6;
        let samples: Vec<[f32; 4]> = get_samples()
            .into_iter()
            .enumerate()
            .map(|(i, ([x, y], clock_s))| [x, y, clock_s, (i % 5) as f32 * 0.06])
            .collect();
        let device = MTLCreateSystemDefaultDevice().expect("a Metal GPU");
        let source: String = format!("{BOUNDARY_PLANES_METAL}\n{KERNEL}");
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(&source), None)
            .expect("boundary_planes.metal compiles");
        let function = library
            .newFunctionWithName(&NSString::from_str("shimmer_test"))
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
        let size: MTLSize = MTLSize {
            width: samples.len(),
            ..one
        };
        encoder.dispatchThreads_threadsPerThreadgroup(size, one);
        encoder.endEncoding();
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE SAMPLES WILL HAPPEN
        let shown: &[[f32; 4]] =
            unsafe { std::slice::from_raw_parts(buffer.contents().as_ptr().cast(), samples.len()) };
        for (sample, gpu) in samples.iter().zip(shown) {
            let cpu: [f32; 2] = get_boundary_shimmer([sample[0], sample[1]], sample[2], sample[3]);
            for i in 0..2 {
                assert!(
                    (gpu[i] - cpu[i]).abs() < SHADER_TOLERANCE_RAD,
                    "{sample:?}: shader {gpu:?}, CPU {cpu:?}"
                );
            }
        }
    }
}
