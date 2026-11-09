//! `visuals.rs` is a player's basic visuals (W-51, basic visuals): the brightness, contrast,
//! saturation and gamma of the finished picture, which each level applies in screen space in its
//! last full-screen pass, after tone mapping, so the game needn't look as dark as it is drawn. It
//! holds the `visuals` section of `player_settings.yaml`, the settings page's rows for it, the
//! uniform the shaders read, and the colour maths, once for every level: on the GPU in
//! `visuals.metal` (`PLAYER_VISUALS_METAL`, which each level prepends to its shader source) and on
//! the CPU in `apply_player_visuals`, which the tests compare with the shader.
//!
//! Interface: `VisualsSettings` (`IDENTITY`, `check`, `get`, `get_mut`, `to_gpu`), `VisualsKnob`,
//! `VISUALS_ROWS`, `GpuPlayerVisuals` (`IDENTITY`), `apply_player_visuals`,
//! `PLAYER_VISUALS_METAL`.
//!
//! Example: `frame.set_visuals(mine.visuals.to_gpu());`
//!
//! Must know:
//! - Gotchas: every setting at 1 is the identity, for which the shader and `apply_player_visuals`
//!   return the colour untouched, so a player who never changes them sees the same pixels.
//! - Gotchas: the fields are the file's keys (`deny_unknown_fields`); saves from before W-51 lack
//!   the section, so each level lists `visuals` in its `ADDED_PLAYER_KEYS`.

use serde::{Deserialize, Serialize};

/// `PLAYER_VISUALS_METAL` is `visuals.metal`: the `PlayerVisuals` struct and
/// `apply_player_visuals`, for a level to prepend to its shader source.
pub const PLAYER_VISUALS_METAL: &str = include_str!("visuals.metal");
/// `VISUALS_ROWS` is each setting with its key in the file (also its label on the settings page),
/// the step of one press of Left or Right, and its lowest and highest values.
pub const VISUALS_ROWS: [(VisualsKnob, &str, f64, f64, f64); 4] = [
    (VisualsKnob::Brightness, "brightness", 0.05, 0.5, 2.0),
    (VisualsKnob::Contrast, "contrast", 0.05, 0.5, 2.0),
    (VisualsKnob::Saturation, "saturation", 0.05, 0.0, 2.0),
    (VisualsKnob::Gamma, "gamma", 0.05, 0.5, 2.0),
];
/// `STEPS_PER_UNIT` is how finely `to_gpu` rounds each setting (the settings page shows four
/// decimals), so steps that add back up to 1 give exactly the identity.
const STEPS_PER_UNIT: f64 = 10_000.0;
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];
const MID_GREY: f32 = 0.5;

/// `VisualsKnob` is one of the four visuals settings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VisualsKnob {
    Brightness,
    Contrast,
    Saturation,
    Gamma,
}

/// `VisualsSettings` is the `visuals` section of `player_settings.yaml`: how the finished picture
/// is adjusted on this player's screen, each 1 for none. `brightness` scales the picture (above 1
/// brighter), `contrast` spreads it from mid grey, `saturation` moves each colour from its grey (0
/// for black and white), and `gamma` above 1 lifts the dark and middle tones.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct VisualsSettings {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub gamma: f64,
}

/// `GpuPlayerVisuals` matches `PlayerVisuals` in `visuals.metal`: the settings as the shader reads
/// them.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GpuPlayerVisuals {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
}

impl GpuPlayerVisuals {
    /// `IDENTITY` leaves every pixel as it is.
    pub const IDENTITY: GpuPlayerVisuals = GpuPlayerVisuals {
        brightness: 1.0,
        contrast: 1.0,
        saturation: 1.0,
        gamma: 1.0,
    };
}

impl VisualsSettings {
    /// `IDENTITY` is the built-in visuals, which leave the picture as each level draws it.
    pub const IDENTITY: VisualsSettings = VisualsSettings {
        brightness: 1.0,
        contrast: 1.0,
        saturation: 1.0,
        gamma: 1.0,
    };

    /// `get` gives one setting's value.
    ///
    /// Example: `let brightness: f64 = mine.visuals.get(VisualsKnob::Brightness);`
    pub fn get(&self, knob: VisualsKnob) -> f64 {
        match knob {
            VisualsKnob::Brightness => self.brightness,
            VisualsKnob::Contrast => self.contrast,
            VisualsKnob::Saturation => self.saturation,
            VisualsKnob::Gamma => self.gamma,
        }
    }

    /// `get_mut` lends the field holding one setting, for the settings page to step.
    ///
    /// Example: `*mine.visuals.get_mut(VisualsKnob::Gamma) = 1.2;`
    pub fn get_mut(&mut self, knob: VisualsKnob) -> &mut f64 {
        match knob {
            VisualsKnob::Brightness => &mut self.brightness,
            VisualsKnob::Contrast => &mut self.contrast,
            VisualsKnob::Saturation => &mut self.saturation,
            VisualsKnob::Gamma => &mut self.gamma,
        }
    }

    /// `check` fails, naming `source` and the key, on the first setting outside its range in
    /// `VISUALS_ROWS` (or not a number).
    ///
    /// Example: `file.visuals.check("mine.yaml")?;`
    pub fn check(&self, source: &str) -> Result<(), String> {
        for (knob, key, _, low, high) in VISUALS_ROWS {
            let value: f64 = self.get(knob);
            if !(low..=high).contains(&value) {
                return Err(format!(
                    "settings {source}: visuals.{key} must be from {low} to {high} (1 for none), not {value}"
                ));
            }
        }
        Ok(())
    }

    /// `to_gpu` gives the settings as the shader reads them, each rounded to `STEPS_PER_UNIT`.
    ///
    /// Example: `frame.set_visuals(mine.visuals.to_gpu());`
    pub fn to_gpu(&self) -> GpuPlayerVisuals {
        let round = |x: f64| ((x * STEPS_PER_UNIT).round() / STEPS_PER_UNIT) as f32;
        GpuPlayerVisuals {
            brightness: round(self.brightness),
            contrast: round(self.contrast),
            saturation: round(self.saturation),
            gamma: round(self.gamma),
        }
    }
}

/// `apply_player_visuals` is `visuals.metal`'s function of the same name on the CPU: an
/// sRGB-encoded colour (each channel 0 to 1) as `visuals` show it, the colour untouched for the
/// identity.
///
/// Example: `let shown: [f32; 3] = apply_player_visuals([0.2, 0.3, 0.4], &mine.visuals.to_gpu());`
pub fn apply_player_visuals(encoded: [f32; 3], visuals: &GpuPlayerVisuals) -> [f32; 3] {
    if *visuals == GpuPlayerVisuals::IDENTITY {
        return encoded;
    }
    let c: [f32; 3] =
        encoded.map(|x| (x * visuals.brightness - MID_GREY) * visuals.contrast + MID_GREY);
    let luma: f32 = c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2];
    c.map(|x| {
        (luma + (x - luma) * visuals.saturation)
            .clamp(0.0, 1.0)
            .powf(1.0 / visuals.gamma)
    })
}

#[cfg(test)]
mod tests {
    //! Tests of the visuals' settings and colour maths.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Settings | the identity (built in), the identity reached by steps up and down, one setting changed | `the_identity_changes_no_colour`, `steps_back_to_1_are_the_identity` |
    //! | Colour | black, white, mid grey, primaries, a dark and a light mixed colour | `the_identity_changes_no_colour`, `each_setting_moves_colours_its_way` |
    //! | Setting changed | brightness, contrast, saturation (0 and above 1), gamma; each up and down | `each_setting_moves_colours_its_way` |
    //! | Range | each setting's lowest and highest together (every corner), on every colour | `extremes_stay_in_0_to_1` |
    //! | Checking | lowest, highest, built in; below lowest, above highest, not a number | `check_takes_each_range_and_names_the_key` |
    //! | Rows | each step divides its range and lands on 1 | `rows_step_through_1` |
    //! | Shader against CPU | the identity (bit for bit), every setting changed (within float rounding) | `the_shader_matches_the_cpu` (macOS) |

    use super::*;

    const COLORS: [[f32; 3]; 8] = [
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
        [0.5, 0.5, 0.5],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.05, 0.08, 0.12],
        [0.9, 0.7, 0.3],
    ];
    const CHANGED: VisualsSettings = VisualsSettings {
        brightness: 1.3,
        contrast: 1.2,
        saturation: 1.4,
        gamma: 1.25,
    };

    /// `with` gives the identity with one setting changed to `value`.
    ///
    /// Example: `let brighter = with(VisualsKnob::Brightness, 1.5);`
    fn with(knob: VisualsKnob, value: f64) -> GpuPlayerVisuals {
        let mut visuals: VisualsSettings = VisualsSettings::IDENTITY;
        *visuals.get_mut(knob) = value;
        visuals.to_gpu()
    }

    #[test]
    fn the_identity_changes_no_colour() {
        let identity: GpuPlayerVisuals = VisualsSettings::IDENTITY.to_gpu();
        assert_eq!(identity, GpuPlayerVisuals::IDENTITY);
        for color in COLORS {
            for x in [color, color.map(|c| c * 0.37 + 0.001)] {
                let shown: [f32; 3] = apply_player_visuals(x, &identity);
                assert_eq!(shown.map(f32::to_bits), x.map(f32::to_bits));
            }
        }
    }

    #[test]
    fn steps_back_to_1_are_the_identity() {
        for (knob, _, step, _, _) in VISUALS_ROWS {
            let mut visuals: VisualsSettings = VisualsSettings::IDENTITY;
            for _ in 0..7 {
                *visuals.get_mut(knob) += step;
            }
            for _ in 0..7 {
                *visuals.get_mut(knob) -= step;
            }
            assert_eq!(visuals.to_gpu(), GpuPlayerVisuals::IDENTITY, "{knob:?}");
        }
    }

    #[test]
    fn each_setting_moves_colours_its_way() {
        let dark: [f32; 3] = [0.2, 0.25, 0.3];
        let brighter: [f32; 3] = apply_player_visuals(dark, &with(VisualsKnob::Brightness, 1.5));
        assert!((0..3).all(|i| brighter[i] > dark[i]), "{brighter:?}");
        let darker: [f32; 3] = apply_player_visuals(dark, &with(VisualsKnob::Brightness, 0.5));
        assert!((0..3).all(|i| darker[i] < dark[i]), "{darker:?}");
        let more_contrast = with(VisualsKnob::Contrast, 1.5);
        assert!(apply_player_visuals(dark, &more_contrast)[0] < dark[0]);
        assert!(apply_player_visuals([0.8; 3], &more_contrast)[0] > 0.8);
        assert_eq!(apply_player_visuals([0.5; 3], &more_contrast), [0.5; 3]);
        let less_contrast: [f32; 3] = apply_player_visuals(dark, &with(VisualsKnob::Contrast, 0.5));
        assert!(less_contrast[0] > dark[0]);
        let grey: [f32; 3] =
            apply_player_visuals([0.9, 0.2, 0.1], &with(VisualsKnob::Saturation, 0.0));
        assert!((grey[0] - grey[1]).abs() < 1e-6 && (grey[1] - grey[2]).abs() < 1e-6);
        let vivid: [f32; 3] =
            apply_player_visuals([0.6, 0.4, 0.3], &with(VisualsKnob::Saturation, 1.5));
        assert!(vivid[0] - vivid[2] > 0.6 - 0.3, "{vivid:?}");
        let lifted: [f32; 3] = apply_player_visuals(dark, &with(VisualsKnob::Gamma, 1.5));
        assert!((0..3).all(|i| lifted[i] > dark[i]), "{lifted:?}");
        let sunk: [f32; 3] = apply_player_visuals(dark, &with(VisualsKnob::Gamma, 0.5));
        assert!((0..3).all(|i| sunk[i] < dark[i]), "{sunk:?}");
        for gamma in [0.5, 2.0] {
            let ends = with(VisualsKnob::Gamma, gamma);
            assert_eq!(apply_player_visuals([0.0; 3], &ends), [0.0; 3]);
            assert_eq!(apply_player_visuals([1.0; 3], &ends), [1.0; 3]);
        }
    }

    #[test]
    fn extremes_stay_in_0_to_1() {
        for corner in 0..1 << VISUALS_ROWS.len() {
            let mut visuals: VisualsSettings = VisualsSettings::IDENTITY;
            for (bit, (knob, _, _, low, high)) in VISUALS_ROWS.into_iter().enumerate() {
                *visuals.get_mut(knob) = if corner >> bit & 1 == 0 { low } else { high };
            }
            assert_eq!(visuals.check("test"), Ok(()));
            for color in COLORS {
                let shown: [f32; 3] = apply_player_visuals(color, &visuals.to_gpu());
                assert!(
                    shown.iter().all(|x| (0.0..=1.0).contains(x)),
                    "{visuals:?} {color:?} -> {shown:?}"
                );
            }
        }
    }

    #[test]
    fn check_takes_each_range_and_names_the_key() {
        assert_eq!(VisualsSettings::IDENTITY.check("test"), Ok(()));
        for (knob, key, _, low, high) in VISUALS_ROWS {
            for value in [low, high] {
                let mut visuals: VisualsSettings = VisualsSettings::IDENTITY;
                *visuals.get_mut(knob) = value;
                assert_eq!(visuals.check("test"), Ok(()));
            }
            for value in [low - 0.01, high + 0.01, f64::NAN] {
                let mut visuals: VisualsSettings = VisualsSettings::IDENTITY;
                *visuals.get_mut(knob) = value;
                let error: String = visuals.check("mine.yaml").expect_err("out of range");
                assert!(
                    error.contains(&format!("visuals.{key}")) && error.contains("mine.yaml"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn rows_step_through_1() {
        for (knob, _, step, low, high) in VISUALS_ROWS {
            for bound in [low, high] {
                let steps: f64 = (1.0 - bound) / step;
                assert!((steps - steps.round()).abs() < 1e-9, "{knob:?} {bound}");
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

        const KERNEL: &str = "kernel void apply_visuals_test(device float4 *colors [[buffer(0)]], \
            constant PlayerVisuals &visuals [[buffer(1)]], uint i [[thread_position_in_grid]]) { \
            colors[i].rgb = apply_player_visuals(colors[i].rgb, visuals); }";
        const SHADER_TOLERANCE: f32 = 1e-5;
        let device = MTLCreateSystemDefaultDevice().expect("a Metal GPU");
        let source: String = format!("{PLAYER_VISUALS_METAL}\n{KERNEL}");
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(&source), None)
            .expect("visuals.metal compiles");
        let function = library
            .newFunctionWithName(&NSString::from_str("apply_visuals_test"))
            .expect("the test kernel");
        let pipeline = device
            .newComputePipelineStateWithFunction_error(&function)
            .expect("a pipeline");
        let queue = device.newCommandQueue().expect("a command queue");
        let colors: Vec<[f32; 4]> = COLORS
            .iter()
            .chain(&COLORS.map(|c| c.map(|x| x * 0.37 + 0.001)))
            .map(|c| [c[0], c[1], c[2], 0.0])
            .collect();
        for visuals in [VisualsSettings::IDENTITY, CHANGED] {
            let gpu: GpuPlayerVisuals = visuals.to_gpu();
            let bytes_count: usize = std::mem::size_of_val(colors.as_slice());
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY PAST THE COLOURS WILL HAPPEN
            let buffer = unsafe {
                device.newBufferWithBytes_length_options(
                    NonNull::new(colors.as_ptr().cast_mut().cast()).expect("a slice isn't null"),
                    bytes_count,
                    MTLResourceOptions::StorageModeShared,
                )
            }
            .expect("a buffer");
            let command_buffer = queue.commandBuffer().expect("a command buffer");
            let encoder = command_buffer
                .computeCommandEncoder()
                .expect("a compute encoder");
            encoder.setComputePipelineState(&pipeline);
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE KERNEL READING THE WRONG BUFFERS WILL HAPPEN
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&buffer), 0, 0);
                encoder.setBytes_length_atIndex(
                    NonNull::from(&gpu).cast(),
                    std::mem::size_of::<GpuPlayerVisuals>(),
                    1,
                );
            }
            let size: MTLSize = MTLSize {
                width: colors.len(),
                height: 1,
                depth: 1,
            };
            encoder.dispatchThreads_threadsPerThreadgroup(size, size);
            encoder.endEncoding();
            command_buffer.commit();
            command_buffer.waitUntilCompleted();
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE COLOURS WILL HAPPEN
            let shown: &[[f32; 4]] = unsafe {
                std::slice::from_raw_parts(buffer.contents().as_ptr().cast(), colors.len())
            };
            for (color, gpu_shown) in colors.iter().zip(shown) {
                let rgb: [f32; 3] = [color[0], color[1], color[2]];
                let cpu_shown: [f32; 3] = apply_player_visuals(rgb, &gpu);
                for i in 0..3 {
                    match visuals == VisualsSettings::IDENTITY {
                        true => assert_eq!(gpu_shown[i].to_bits(), rgb[i].to_bits()),
                        false => assert!(
                            (gpu_shown[i] - cpu_shown[i]).abs() < SHADER_TOLERANCE,
                            "{rgb:?}: shader {gpu_shown:?}, CPU {cpu_shown:?}"
                        ),
                    }
                }
            }
        }
    }
}
