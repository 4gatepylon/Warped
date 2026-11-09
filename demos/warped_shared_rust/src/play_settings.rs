//! `play_settings.rs` is the sections of the settings files that every level reads alike: in
//! `server_settings.yaml`, movement, sprint energy, both guns and their beams, consoles,
//! respawning, how a hit slows, the flashlight, aiming, thrown wormholes, the map's edge, space and
//! the black hole; in `player_settings.yaml`, the view with its head bob, how a hit looks and the
//! minimap. Each level's `settings.rs` composes them into its `ServerSettings` and
//! `PerPlayerSettings` (City Sphere adds its own `glass`) and reads and checks the files.
//!
//! Interface: `MovementSettings`, `SprintEnergySettings`, `HeadBobSettings`, `SniperSettings`,
//! `RifleSettings`, `BeamSpeckleSettings`, `ConsoleSettings`, `RespawnSettings`, `WhenHitSettings`,
//! `HitEffectSettings`, `FlashlightSettings`, `AimSettings`, `WormholeSettings`, `MapEdgeSettings`,
//! `SpaceSettings`, `BlackHoleSettings`, `ViewSettings`, `MinimapCorner`, `MinimapSettings`.
//!
//! Example: `let rifle: &RifleSettings = &settings.rifle;`
//!
//! Must know:
//! - Gotchas: each section's fields are its keys in the files (`deny_unknown_fields`), so renaming
//!   a field changes the files' format (the `compatible-changes` skill); `respawn` alone also
//!   reads the old `delay_s` (`RespawnSettings`).

use serde::{Deserialize, Serialize};

const OLD_FILE_PERIOD_S: f64 = 300.0;

/// `MovementSettings` is walking and sprinting speed as multiples of the map's, a jump's height and
/// time in the air as a fraction of the map's jump (gravity follows from the two), and crouching:
/// its speed and eye height as fractions of walking's, and how long it takes to crouch or stand;
/// and how long after the last movement key a player can shoot.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct MovementSettings {
    pub walk_speed_per_map_speed: f64,
    pub sprint_speed_per_map_speed: f64,
    pub jump_height_m: f64,
    pub jump_time_per_map_jump_time: f64,
    pub crouch_speed_frac: f64,
    pub crouch_eye_height_frac: f64,
    pub crouch_s: f64,
    pub shoot_after_moving_s: f64,
}

/// `SprintEnergySettings` is how long a full sprint bar lasts, and how long an empty one takes to
/// refill while crouched and while walking or standing upright.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SprintEnergySettings {
    pub drain_s: f64,
    pub refill_crouching_s: f64,
    pub refill_walking_s: f64,
}

/// `HeadBobSettings` is how deep the eye dips at each step, how many steps a second, how long the
/// bob takes to start or stop, and how the gun in hand sways with the steps while walking and
/// while sprinting: how far it slides left and right, and how far a gun as long as the bolt-action
/// rifle swings (its muzzle lagging the slide; shorter guns swing in proportion).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct HeadBobSettings {
    pub walk_depth_m: f64,
    pub sprint_depth_m: f64,
    pub walk_steps_per_s: f64,
    pub sprint_steps_per_s: f64,
    pub fade_s: f64,
    pub gun_walk_sway_m: f64,
    pub gun_sprint_sway_m: f64,
    pub rifle_walk_swing_rad: f64,
    pub rifle_sprint_swing_rad: f64,
}

/// `SniperSettings` is gun 1, the laser wand: how long a click's shot hits, how long it then takes
/// to reload, how many hits eliminate a player, how its beam flies (speed, and how long each
/// point stays lit), how far a shot strays from the crosshair (the mean of an exponential
/// distribution, and a cap; `shot_noise.rs`), and the share of its beam's pixels that speckle each
/// frame (the same: a mean and a cap).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SniperSettings {
    pub shot_s: f64,
    pub reload_s: f64,
    pub hits_to_eliminate: u8,
    pub beam_speed_m_per_s: f64,
    pub beam_trail_s: f64,
    pub noise_mean_rad: f64,
    pub noise_max_rad: f64,
    pub beam_speckle_mean_frac: f64,
    pub beam_speckle_max_frac: f64,
}

/// `RifleSettings` is gun 2: its fire rate, how long a shot hits, how its beam flies, overheating
/// and cooling, the hits that eliminate a player, each shot's recoil, the most a shot strays
/// from the crosshair, cold and at full heat, and the share of its beam's pixels that speckle each
/// frame, cold and at full heat (`shot_noise.rs`).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RifleSettings {
    pub shots_per_s: f64,
    pub pulse_s: f64,
    pub beam_speed_m_per_s: f64,
    pub beam_trail_s: f64,
    pub shots_until_overheat: u32,
    pub cool_down_s: f64,
    pub head_hits_to_eliminate: u8,
    pub body_hits_to_eliminate: u8,
    pub recoil_up_rad: f64,
    pub recoil_sideways_rad: f64,
    pub recoil_settle_s: f64,
    pub noise_start_rad: f64,
    pub noise_max_rad: f64,
    pub beam_speckle_start_frac: f64,
    pub beam_speckle_max_frac: f64,
}

/// `BeamSpeckleSettings` is how a laser beam's speckled pixels look, with both guns: the shares of
/// them painted black, turned white and dropping out (showing what is behind), which add up to 1.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct BeamSpeckleSettings {
    pub black_frac: f64,
    pub white_frac: f64,
    pub dropout_frac: f64,
}

/// `ConsoleSettings` is how long holding `E` takes to turn off a console, and from how far.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ConsoleSettings {
    pub hold_to_disable_s: f64,
    pub reach_m: f64,
}

/// `RespawnSettings` is how long a player who was hit waits before coming back, a wait that grows
/// the longer the level has gone on (the user's rule, W-142 (growing respawn time)): `start_s` at
/// first, `step_s` more every `period_s` (`get_wait_s`). A file from before the rule, with only
/// `delay_s`, still loads as the flat wait it asked for: `start_s` = `delay_s`, `step_s` = 0
/// (`RespawnFile`).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(try_from = "RespawnFile")]
pub struct RespawnSettings {
    pub start_s: f64,
    pub step_s: f64,
    pub period_s: f64,
}

/// `RespawnFile` is the `respawn` section as a file may write it: the rule's three keys, or only
/// the flat `delay_s` of files from before the rule.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RespawnFile {
    delay_s: Option<f64>,
    start_s: Option<f64>,
    step_s: Option<f64>,
    period_s: Option<f64>,
}

impl TryFrom<RespawnFile> for RespawnSettings {
    type Error = String;

    /// `try_from` reads the section's rule, or an old file's flat `delay_s`, and fails on a mix of
    /// the two, a missing key, or a `period_s` that isn't above 0.
    ///
    /// Example: `let respawn = RespawnSettings::try_from(file)?;`
    fn try_from(file: RespawnFile) -> Result<RespawnSettings, String> {
        let settings: RespawnSettings = match file {
            RespawnFile {
                delay_s: Some(delay_s),
                start_s: None,
                step_s: None,
                period_s: None,
            } => RespawnSettings {
                start_s: delay_s,
                step_s: 0.0,
                period_s: OLD_FILE_PERIOD_S,
            },
            RespawnFile {
                delay_s: None,
                start_s: Some(start_s),
                step_s: Some(step_s),
                period_s: Some(period_s),
            } => RespawnSettings {
                start_s,
                step_s,
                period_s,
            },
            _ => {
                return Err(
                    "respawn needs start_s, step_s and period_s (or, in a file from before them, \
                     only delay_s)"
                        .to_string(),
                );
            }
        };
        if !(settings.period_s > 0.0) {
            return Err(format!(
                "respawn.period_s must be above 0, not {}",
                settings.period_s
            ));
        }
        Ok(settings)
    }
}

impl RespawnSettings {
    /// `get_wait_s` gives how long a player hit `level_s` seconds after the level (its round)
    /// started waits before coming back: `start_s + step_s × floor(level_s / period_s)`.
    ///
    /// Example: `let wait_s: f64 = settings.respawn.get_wait_s(301.0); // 30 s by default`
    pub fn get_wait_s(&self, level_s: f64) -> f64 {
        assert!(
            level_s >= 0.0,
            "get_wait_s: the level's time must be 0 or more, not {level_s}"
        );
        self.start_s + self.step_s * (level_s / self.period_s).floor()
    }
}

/// `WhenHitSettings` is how a hit slows the player who takes it: to a fraction of their speed, for
/// a while.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WhenHitSettings {
    pub slow_speed_frac: f64,
    pub slow_s: f64,
}

/// `HitEffectSettings` is how a hit looks to the player who takes it: the screen's edges turning
/// red, shaking and blurring inward, all fading over `effect_s`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct HitEffectSettings {
    pub effect_s: f64,
    pub red_frac: f64,
    pub shake_px: f64,
    pub blur_px: f64,
}

/// `FlashlightSettings` is the flashlight's cone (half its opening angle and the width of its soft
/// edge), the distance at which its light falls to half, and its brightness against the sun's.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashlightSettings {
    pub cone_half_angle_rad: f64,
    pub soft_edge_rad: f64,
    pub range_m: f64,
    pub brightness_frac: f64,
}

/// `AimSettings` is aiming (holding the aim control): how much closer things look with each gun at
/// full zoom, and how long zooming fully in or out takes. The mouse turns the view more slowly by
/// the same factor.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct AimSettings {
    pub sniper_zoom: f64,
    pub rifle_zoom: f64,
    pub zoom_s: f64,
}

/// `WormholeSettings` is how a thrown wormhole mouth flies (its speed, how far above the aim it
/// leaves, its gravity and how long it may fly before it fizzles), on voxel maps its size
/// (Schwarzschild radius and reach; the house's mouths all share the house's size), how many shots
/// of each gun destroy an enemy's pair, and how long a hit pair flashes red.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WormholeSettings {
    pub throw_speed_m_per_s: f64,
    pub throw_up_rad: f64,
    pub gravity_m_per_s_per_s: f64,
    pub max_flight_s: f64,
    pub schwarzschild_radius_m: f64,
    pub reach_m: f64,
    pub sniper_hits_to_destroy: u8,
    pub rifle_hits_to_destroy: u8,
    pub hit_flash_s: f64,
}

/// `MapEdgeSettings` is the map's soft edge on a map with a crater: how far up the crater's wall
/// (0 at its foot, 1 at the rim) players start to slow down, and how far up their speed would
/// reach 0, which they never get to (each step outward is cut by the share of the way they have
/// come).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct MapEdgeSettings {
    pub start_wall_frac: f64,
    pub stop_wall_frac: f64,
}

/// `SpaceSettings` is how curved space is: its curvature (1 over the curvature radius) as a multiple
/// of the map's own, from `MIN_CURVATURE_PER_MAP_CURVATURE` to `MAX_CURVATURE_PER_MAP_CURVATURE`
/// (`Map::scale_curvature`).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SpaceSettings {
    pub curvature_per_map_curvature: f64,
}

/// `BlackHoleSettings` is the black hole that grows as consoles turn off (W-68) and slows time
/// near it (W-107): how long it eases to a new size, its Schwarzschild radius and reach at full
/// strength, its wide lens at full strength (the most it turns a ray, and its core radius as a
/// share of the reach), how many times stronger than in general relativity time slows near it (0
/// for not at all), and the slowest its time runs (a share of the time far away, above 0 so that
/// nobody freezes).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct BlackHoleSettings {
    pub transition_s: f64,
    pub max_schwarzschild_radius_m: f64,
    pub max_reach_m: f64,
    pub max_lens_rad: f64,
    pub lens_core_per_reach: f64,
    pub time_dilation_per_general_relativity: f64,
    pub min_time_rate_frac: f64,
}

/// `ViewSettings` is how a player's view moves: how far it turns for each pixel the mouse moves, how
/// long motion smears, how strongly the view stretches near the map's edge, how strongly the scene
/// shifts colour (light cyan forward, dark red backward) while moving through a wormhole's throat
/// (0 to 1), how bright the Milky Way is (0 to 1), and the head bob.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ViewSettings {
    pub look_rad_per_px: f64,
    pub motion_blur_s: f64,
    pub edge_stretch_frac: f64,
    pub wormhole_tint_frac: f64,
    pub milky_way_brightness_frac: f64,
    pub head_bob: HeadBobSettings,
}

/// `MinimapCorner` is the corner of the window the minimap sits in.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum MinimapCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// `MinimapSettings` is the minimap: whether it shows, the corner it sits in, its radius as a
/// fraction of the window's height, and how far from the player it reaches (also the circle on
/// the menu's map page).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct MinimapSettings {
    pub is_shown: bool,
    pub corner: MinimapCorner,
    pub radius_frac: f64,
    pub range_m: f64,
}

#[cfg(test)]
mod tests {
    //! Tests of the respawn rule and of reading its section.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Level time | 0, just before 5 minutes, at 5, just before 10, at 10, much later | `the_wait_grows_every_period` |
    //! | Step | above 0 (grows), 0 (flat) | `the_wait_grows_every_period`, `an_old_delay_reads_as_a_flat_wait` |
    //! | Keys written | the rule's three, only the old `delay_s`, both, some missing, an unknown one | `the_wait_grows_every_period`, `an_old_delay_reads_as_a_flat_wait`, `mixed_missing_and_unknown_keys_fail` |
    //! | Period | above 0, 0 | `the_wait_grows_every_period`, `mixed_missing_and_unknown_keys_fail` |
    //! | Sent to players | the rule written back out and read again | `the_rule_survives_writing` |

    use super::*;

    const LUNAR_SERVER_YAML: &str =
        include_str!("../../hyperbolic_wormholes_rust/server_settings.yaml");
    const JUST_BEFORE_S: f64 = 1e-6;
    const FIVE_MINUTES_S: f64 = 300.0;
    const TEN_MINUTES_S: f64 = 600.0;
    const HOUR_S: f64 = 3600.0;
    const OLD_DELAY_S: f64 = 3.0;

    /// `LunarSettings` is the respawn section of Lunar Surface's `server_settings.yaml`.
    #[derive(Deserialize)]
    struct LunarSettings {
        respawn: RespawnSettings,
    }

    /// `read_respawn` reads a `respawn` section from YAML text.
    fn read_respawn(yaml: &str) -> Result<RespawnSettings, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    #[test]
    fn the_wait_grows_every_period() {
        let settings: LunarSettings =
            serde_yaml::from_str(LUNAR_SERVER_YAML).expect("Lunar Surface's server settings");
        let respawn: RespawnSettings = settings.respawn;
        assert_eq!(
            respawn,
            RespawnSettings {
                start_s: 20.0,
                step_s: 10.0,
                period_s: FIVE_MINUTES_S,
            }
        );
        assert_eq!(respawn.get_wait_s(0.0), 20.0);
        assert_eq!(respawn.get_wait_s(FIVE_MINUTES_S - JUST_BEFORE_S), 20.0);
        assert_eq!(respawn.get_wait_s(FIVE_MINUTES_S), 30.0);
        assert_eq!(respawn.get_wait_s(TEN_MINUTES_S - JUST_BEFORE_S), 30.0);
        assert_eq!(respawn.get_wait_s(TEN_MINUTES_S), 40.0);
        assert_eq!(respawn.get_wait_s(HOUR_S), 140.0);
    }

    #[test]
    fn an_old_delay_reads_as_a_flat_wait() {
        let respawn: RespawnSettings =
            read_respawn(&format!("delay_s: {OLD_DELAY_S}")).expect("an old file's respawn");
        assert_eq!((respawn.start_s, respawn.step_s), (OLD_DELAY_S, 0.0));
        assert_eq!(respawn.get_wait_s(0.0), OLD_DELAY_S);
        assert_eq!(respawn.get_wait_s(HOUR_S), OLD_DELAY_S);
    }

    #[test]
    fn mixed_missing_and_unknown_keys_fail() {
        for yaml in [
            "{delay_s: 3, start_s: 20, step_s: 10, period_s: 300}",
            "{start_s: 20, step_s: 10}",
            "{}",
            "{start_s: 20, step_s: 10, period_s: 300, every_s: 1}",
            "{start_s: 20, step_s: 10, period_s: 0}",
        ] {
            assert!(read_respawn(yaml).is_err(), "{yaml}");
        }
        assert!(read_respawn("{start_s: 0, step_s: 0, period_s: 1}").is_ok());
    }

    #[test]
    fn the_rule_survives_writing() {
        let respawn: RespawnSettings =
            read_respawn("{start_s: 2, step_s: 1, period_s: 20}").expect("a respawn section");
        let text: String = serde_yaml::to_string(&respawn).expect("respawn serializes");
        assert!(!text.contains("delay_s"), "{text}");
        assert_eq!(read_respawn(&text).expect("it reads back"), respawn);
    }
}
