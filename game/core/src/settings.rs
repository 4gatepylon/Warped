//! `settings.rs` reads `server_settings.yaml`, the numbers that shape play for everyone (movement,
//! sprint energy, both guns, consoles, respawning, how a hit slows, the flashlight, thrown wormholes,
//! how curved space is, the black hole), which the game server sends to every player who joins, so
//! they can be tuned without rebuilding. It also holds the sections of each player's own
//! `player_settings.yaml`, which the client's `player_settings.rs` reads, and both files as built in.
//!
//! Interface: `ServerSettings` (`Builtin`, `Load`, `FromYaml`, `FromJson`, `to_json`), the
//! sections of both files, `ViewSettings::Builtin`, `BUILTIN_SERVER_YAML`, `BUILTIN_PLAYER_YAML`,
//! `check_numbers`, `read_text`.
//!
//! Example: `let settings = ServerSettings::Load(Path::new("fast.yaml"))?;`
//!
//! Must know:
//! - Every key is required and unknown keys fail, so a typo can't silently keep a default.
//! - The built-in settings are the two files themselves, compiled in.

use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BUILTIN_SERVER_YAML: &str = include_str!("../server_settings.yaml");
pub const BUILTIN_PLAYER_YAML: &str = include_str!("../player_settings.yaml");
pub const MIN_CURVATURE_PER_MAP_CURVATURE: f64 = 1.0;
pub const MAX_CURVATURE_PER_MAP_CURVATURE: f64 = 2.0;
const SERVER_ZERO_ALLOWED_KEYS: [&str; 21] = [
    "movement.shoot_after_moving_s",
    "map_edge.start_wall_frac",
    "sniper.reload_s",
    "sniper.noise_mean_rad",
    "sniper.noise_max_rad",
    "sniper.beam_speckle_mean_frac",
    "sniper.beam_speckle_max_frac",
    "rifle.noise_start_rad",
    "rifle.noise_max_rad",
    "rifle.beam_speckle_start_frac",
    "rifle.beam_speckle_max_frac",
    "beam_speckle.black_frac",
    "beam_speckle.white_frac",
    "beam_speckle.dropout_frac",
    "rifle.recoil_up_rad",
    "rifle.recoil_sideways_rad",
    "rifle.recoil_settle_s",
    "respawn.delay_s",
    "wormholes.throw_up_rad",
    "black_hole.time_dilation_per_general_relativity",
    "black_hole.max_lens_rad",
];
const SPECKLE_KINDS_SUM_TOLERANCE_FRAC: f64 = 1e-9;
/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

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

/// `RespawnSettings` is how long a player who was hit waits before coming back.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RespawnSettings {
    pub delay_s: f64,
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

impl ViewSettings {
    /// `Builtin` gives the `view` section of the built-in `player_settings.yaml`, for a world
    /// that no player's own settings have reached yet (a server's, a benchmark's).
    ///
    /// Example: `let head_bob = ViewSettings::Builtin().head_bob;`
    #[allow(non_snake_case)]
    pub fn Builtin() -> ViewSettings {
        let file: serde_yaml::Value = serde_yaml::from_str(BUILTIN_PLAYER_YAML)
            .expect("the built-in player settings are YAML");
        serde_yaml::from_value(file["view"].clone()).expect("the built-in view settings are valid")
    }
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

/// `ServerSettings` is everything in `server_settings.yaml`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    pub movement: MovementSettings,
    pub sprint_energy: SprintEnergySettings,
    pub sniper: SniperSettings,
    pub rifle: RifleSettings,
    pub beam_speckle: BeamSpeckleSettings,
    pub consoles: ConsoleSettings,
    pub respawn: RespawnSettings,
    pub when_hit: WhenHitSettings,
    pub flashlight: FlashlightSettings,
    pub map_edge: MapEdgeSettings,
    pub aim: AimSettings,
    pub wormholes: WormholeSettings,
    pub space: SpaceSettings,
    pub black_hole: BlackHoleSettings,
}

/// `check_numbers` fails, naming the key and `source`, on the first number in `value` that is
/// negative or not finite, or zero outside `zero_allowed`.
///
/// Example: `check_numbers(&serde_json::to_value(&settings)?, &SERVER_ZERO_ALLOWED_KEYS, "fast.yaml")?;`
pub fn check_numbers(value: &Value, zero_allowed: &[&str], source: &str) -> Result<()> {
    match find_bad_number(value, "", zero_allowed) {
        Some(key) => Err(format!(
            "settings {source}: {key} must be a number above 0 (0 is allowed only for {})",
            zero_allowed.join(", ")
        )
        .into()),
        None => Ok(()),
    }
}

/// `find_bad_number` gives the dotted key of the first number in `value` that is negative or not
/// finite, or zero outside `zero_allowed`, or `None` if every number is fine.
///
/// Example: `let bad = find_bad_number(&serde_json::to_value(&settings)?, "", &SERVER_ZERO_ALLOWED_KEYS);`
fn find_bad_number(value: &Value, key: &str, zero_allowed: &[&str]) -> Option<String> {
    match value {
        Value::Object(fields) => fields.iter().find_map(|(name, field)| {
            let child: String = if key.is_empty() {
                name.clone()
            } else {
                format!("{key}.{name}")
            };
            find_bad_number(field, &child, zero_allowed)
        }),
        Value::Number(number) => {
            let x: f64 = number.as_f64().expect("a JSON number reads as f64");
            let is_bad: bool =
                !x.is_finite() || x < 0.0 || (x == 0.0 && !zero_allowed.contains(&key));
            is_bad.then(|| key.to_string())
        }
        _ => None,
    }
}

/// `read_text` reads a settings file, naming it in errors.
///
/// Example: `let text = read_text(Path::new("fast.yaml"))?;`
pub fn read_text(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)
        .map_err(|error| format!("settings {}: {error}", path.display()))?)
}

impl ServerSettings {
    /// `FromYaml` reads server settings from YAML text, naming `source` in errors, and checks every
    /// number (`check_numbers`) and the numbers that bound each other or have a range.
    ///
    /// Example: `let settings = ServerSettings::FromYaml(&text, "fast.yaml")?;`
    #[allow(non_snake_case)]
    pub fn FromYaml(text: &str, source: &str) -> Result<ServerSettings> {
        let settings: ServerSettings =
            serde_yaml::from_str(text).map_err(|error| format!("settings {source}: {error}"))?;
        check_numbers(
            &serde_json::to_value(&settings)?,
            &SERVER_ZERO_ALLOWED_KEYS,
            source,
        )?;
        let aim: &AimSettings = &settings.aim;
        if aim.sniper_zoom < 1.0 || aim.rifle_zoom < 1.0 {
            return Err(format!(
                "settings {source}: aim zooms must be at least 1 (1 for none), not {} and {}",
                aim.sniper_zoom, aim.rifle_zoom
            )
            .into());
        }
        let rifle: &RifleSettings = &settings.rifle;
        if rifle.noise_start_rad > rifle.noise_max_rad {
            return Err(format!(
                "settings {source}: rifle needs noise_start_rad <= noise_max_rad, not {} and {}",
                rifle.noise_start_rad, rifle.noise_max_rad
            )
            .into());
        }
        let speckle: &BeamSpeckleSettings = &settings.beam_speckle;
        let speckles_frac: [f64; 7] = [
            settings.sniper.beam_speckle_mean_frac,
            settings.sniper.beam_speckle_max_frac,
            rifle.beam_speckle_start_frac,
            rifle.beam_speckle_max_frac,
            speckle.black_frac,
            speckle.white_frac,
            speckle.dropout_frac,
        ];
        let kinds_sum_frac: f64 = speckle.black_frac + speckle.white_frac + speckle.dropout_frac;
        if speckles_frac.iter().any(|&x| x > 1.0)
            || rifle.beam_speckle_start_frac > rifle.beam_speckle_max_frac
            || (kinds_sum_frac - 1.0).abs() > SPECKLE_KINDS_SUM_TOLERANCE_FRAC
        {
            return Err(format!(
                "settings {source}: beam speckles are shares from 0 to 1, with the rifle's \
                 beam_speckle_start_frac <= beam_speckle_max_frac and beam_speckle's three \
                 shares adding up to 1, not {speckles_frac:?}"
            )
            .into());
        }
        let edge: &MapEdgeSettings = &settings.map_edge;
        if edge.start_wall_frac >= edge.stop_wall_frac || edge.stop_wall_frac > 1.0 {
            return Err(format!(
                "settings {source}: map_edge needs start_wall_frac < stop_wall_frac <= 1, not {} and {}",
                edge.start_wall_frac, edge.stop_wall_frac
            )
            .into());
        }
        let curvature: f64 = settings.space.curvature_per_map_curvature;
        if !(MIN_CURVATURE_PER_MAP_CURVATURE..=MAX_CURVATURE_PER_MAP_CURVATURE).contains(&curvature)
        {
            return Err(format!(
                "settings {source}: space.curvature_per_map_curvature must be from {MIN_CURVATURE_PER_MAP_CURVATURE} (the map's own curvature) to {MAX_CURVATURE_PER_MAP_CURVATURE} (twice it), not {curvature}"
            )
            .into());
        }
        let min_time_rate_frac: f64 = settings.black_hole.min_time_rate_frac;
        if min_time_rate_frac > 1.0 {
            return Err(format!(
                "settings {source}: black_hole.min_time_rate_frac must be above 0 and at most 1, not {min_time_rate_frac}"
            )
            .into());
        }
        Ok(settings)
    }

    /// `Builtin` gives the settings in `server_settings.yaml` as this program was built with it.
    ///
    /// Example: `let settings = ServerSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> ServerSettings {
        ServerSettings::FromYaml(BUILTIN_SERVER_YAML, "server_settings.yaml (built in)")
            .expect("the built-in server_settings.yaml is valid (tested)")
    }

    /// `Load` reads server settings from a YAML file like `server_settings.yaml`.
    ///
    /// Example: `let settings = ServerSettings::Load(Path::new("fast.yaml"))?;`
    #[allow(non_snake_case)]
    pub fn Load(path: &Path) -> Result<ServerSettings> {
        ServerSettings::FromYaml(&read_text(path)?, &path.display().to_string())
    }

    /// `FromJson` reads settings sent by a game server (`to_json`), checking them as `Load` does.
    ///
    /// Example: `let settings = ServerSettings::FromJson(&bytes)?;`
    #[allow(non_snake_case)]
    pub fn FromJson(bytes: &[u8]) -> Result<ServerSettings> {
        let text: &str = std::str::from_utf8(bytes)?;
        ServerSettings::FromYaml(text, "from the game server")
    }

    /// `to_json` gives the settings as compact JSON, to send to players.
    ///
    /// Example: `let bytes = settings.to_json();`
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("settings serialize to JSON")
    }
}
