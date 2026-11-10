//! `settings.rs` reads the game's two settings files, so players can tune it without rebuilding:
//! `server_settings.yaml`, the numbers that shape play for everyone (movement, sprint energy, both
//! guns, consoles, respawning, how a hit slows, the flashlight, thrown wormholes, how curved space
//! is, the black hole), which the game server sends to every player who joins; and
//! `player_settings.yaml`, each player's own choices (keys, mouse speed, head bob, motion blur, the
//! Milky Way's brightness, how a hit looks).
//!
//! Interface: `ServerSettings` (`Builtin`, `Load`, `FromJson`, `to_json`), `PerPlayerSettings`
//! (`Builtin`, `Load`) and their sections.
//!
//! Example: `let settings = ServerSettings::Load(Path::new("fast.yaml"))?;`
//!
//! Must know:
//! - Every key is required and unknown keys fail, so a typo can't silently keep a default. The only
//!   exceptions are named: player keys removed since saves began (`REMOVED_PLAYER_KEYS`) are
//!   dropped, and those added since (`ADDED_PLAYER_KEYS`) take their built-in values when missing.
//! - The built-in settings are the two files themselves, compiled in.
//! - `PerPlayerSettings` exists only on macOS, where the game's window runs (its keys are SDL's);
//!   the game server also builds for Linux.

#[cfg(target_os = "macos")]
use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(target_os = "macos")]
use crate::key_bindings::{self, KeyBindings};
pub use crate::play_settings::{
    AimSettings, BeamSpeckleSettings, BlackHoleSettings, ConsoleSettings, FlashlightSettings,
    GravitySettings, MapEdgeSettings, MovementSettings, RespawnSettings, RifleSettings,
    ShootingSettings, SniperSettings, SpaceSettings, SprintEnergySettings, WhenHitSettings,
    WormholeSettings,
};
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
pub use crate::play_settings::{HeadBobSettings, MinimapCorner};
#[cfg(target_os = "macos")]
pub use crate::play_settings::{HitEffectSettings, MinimapSettings, ViewSettings};
#[cfg(target_os = "macos")]
use warped_shared::visuals::VisualsSettings;

const BUILTIN_SERVER_YAML: &str = include_str!("../server_settings.yaml");
#[cfg(target_os = "macos")]
const BUILTIN_PLAYER_YAML: &str = include_str!("../player_settings.yaml");
pub const MIN_CURVATURE_PER_MAP_CURVATURE: f64 = 1.0;
pub const MAX_CURVATURE_PER_MAP_CURVATURE: f64 = 2.0;
const SERVER_ZERO_ALLOWED_KEYS: [&str; 33] = [
    "wormholes.sniper_hits_to_destroy",
    "wormholes.rifle_hits_to_destroy",
    "glass_lights.low_brightness_frac",
    "gravity.black_hole_acceleration_frac",
    "gravity.black_hole_min_radius_m",
    "gravity.black_hole_darkness_frac",
    "gravity.acceleration_m_per_s_per_s",
    "gravity.brake_m_per_s_per_s",
    "glass.max_bounce_count",
    "glass.reflectivity_frac",
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
    "respawn.start_s",
    "respawn.step_s",
    "shooting.drop_m_per_s_per_s",
    "wormholes.throw_up_rad",
    "black_hole.time_dilation_per_general_relativity",
    "black_hole.max_lens_rad",
];
const SPECKLE_KINDS_SUM_TOLERANCE_FRAC: f64 = 1e-9;
#[cfg(target_os = "macos")]
const PLAYER_ZERO_ALLOWED_KEYS: [&str; 13] = [
    "view.motion_blur_s",
    "view.edge_stretch_frac",
    "view.wormhole_tint_frac",
    "view.milky_way_brightness_frac",
    "view.head_bob.walk_depth_m",
    "view.head_bob.sprint_depth_m",
    "view.head_bob.gun_walk_sway_m",
    "view.head_bob.gun_sprint_sway_m",
    "view.head_bob.rifle_walk_swing_rad",
    "view.head_bob.rifle_sprint_swing_rad",
    "hit_effects.red_frac",
    "hit_effects.shake_px",
    "hit_effects.blur_px",
];
const REMOVED_PLAYER_KEYS: [&str; 1] = ["view.wormhole_tint_s"];
const ADDED_PLAYER_KEYS: [&str; 7] = [
    "visuals",
    "keys.switch_view",
    "view.milky_way_brightness_frac",
    "view.head_bob.gun_walk_sway_m",
    "view.head_bob.gun_sprint_sway_m",
    "view.head_bob.rifle_walk_swing_rad",
    "view.head_bob.rifle_sprint_swing_rad",
];

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `PlayerFile` is `player_settings.yaml` as written: the keys by name, the view and how a hit looks.
#[cfg(target_os = "macos")]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayerFile {
    keys: BTreeMap<String, Vec<String>>,
    view: ViewSettings,
    hit_effects: HitEffectSettings,
    minimap: MinimapSettings,
    visuals: VisualsSettings,
}

/// `PerPlayerSettings` is everything in `player_settings.yaml`, with the keys read into controls.
#[cfg(target_os = "macos")]
pub struct PerPlayerSettings {
    pub keys: KeyBindings,
    pub view: ViewSettings,
    pub hit_effects: HitEffectSettings,
    pub minimap: MinimapSettings,
    pub visuals: VisualsSettings,
}

/// `GlassSettings` is how shots meet City Sphere's glass from the glass level: how many times a
/// shot may reflect off it, how far a shot goes in all (metres, every leg counted), and the share
/// of the range left that each reflection keeps (`reflectivity_frac`, W-158, per-map game
/// settings; 1 when a file leaves it out, a perfect mirror).
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct GlassSettings {
    pub max_bounce_count: u32,
    pub range_m: f64,
    #[serde(default = "get_perfect_reflectivity_frac")]
    pub reflectivity_frac: f64,
}

/// `get_perfect_reflectivity_frac` is `GlassSettings::reflectivity_frac` for a file without it:
/// a perfect mirror, as before the setting.
fn get_perfect_reflectivity_frac() -> f64 {
    1.0
}

/// `MAX_DOME_CIRCUMFERENCE_FRAC` is the widest a team dome may be: half the way round, where the
/// two teams' domes together would cover the whole glass level.
pub const MAX_DOME_CIRCUMFERENCE_FRAC: f64 = 0.5;
const BUILTIN_DOME_CIRCUMFERENCE_FRAC: f64 = 0.2;

/// `TeamDomesSettings` is how big City Sphere's team domes are (W-148, team boundary planes in
/// every level): the share of the way round the glass level that each team's dome over its pylon
/// spans (`circumference_frac`; the user: "start it at around 0.2 of circumference (i.e. together
/// they take up 0.4 of circumference"). A file without the section takes the built-in 0.2.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct TeamDomesSettings {
    pub circumference_frac: f64,
}

impl Default for TeamDomesSettings {
    /// `default` is the built-in size: each dome spans 0.2 of the circumference.
    ///
    /// Example: `let domes: TeamDomesSettings = TeamDomesSettings::default();`
    fn default() -> TeamDomesSettings {
        TeamDomesSettings {
            circumference_frac: BUILTIN_DOME_CIRCUMFERENCE_FRAC,
        }
    }
}

impl TeamDomesSettings {
    /// `check` fails, naming the key, unless `circumference_frac` is above 0 and at most
    /// `MAX_DOME_CIRCUMFERENCE_FRAC`.
    ///
    /// Example: `settings.team_domes.check()?;`
    pub fn check(&self) -> Result<()> {
        if !(self.circumference_frac > 0.0
            && self.circumference_frac <= MAX_DOME_CIRCUMFERENCE_FRAC)
        {
            return Err(format!(
                "team_domes.circumference_frac must be above 0 and at most {MAX_DOME_CIRCUMFERENCE_FRAC}, not {}",
                self.circumference_frac
            )
            .into());
        }
        Ok(())
    }
}

/// `GlassLightsSettings` is how the lights on City Sphere's glass shell (its glowing ribs) pulse
/// (W-203, dimmer, pulsing lights on City Sphere's glass): between `low_brightness_frac` and
/// `high_brightness_frac` of their full brightness, in a triangle wave of `period_s` that spends
/// `rising_frac` of each period rising and the rest falling (the user: "from around 30%
/// brightness of actual to around 70% brightness of actual on around a 16 second period", "10 are
/// spent on the upwards part of the triangle and 6 are spent on the downwards part"). A file
/// without the section takes those values.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct GlassLightsSettings {
    pub low_brightness_frac: f64,
    pub high_brightness_frac: f64,
    pub period_s: f64,
    pub rising_frac: f64,
}

const BUILTIN_GLASS_LIGHTS: GlassLightsSettings = GlassLightsSettings {
    low_brightness_frac: 0.3,
    high_brightness_frac: 0.7,
    period_s: 16.0,
    rising_frac: 0.625,
};

impl Default for GlassLightsSettings {
    /// `default` is the built-in pulse: 0.3 to 0.7 over 16 s, 10 s rising and 6 s falling.
    ///
    /// Example: `let lights: GlassLightsSettings = GlassLightsSettings::default();`
    fn default() -> GlassLightsSettings {
        BUILTIN_GLASS_LIGHTS
    }
}

impl GlassLightsSettings {
    /// `check` fails, naming the keys, unless the low brightness is from 0 up to the high one,
    /// the period is above 0 and the rising share is strictly between 0 and 1.
    ///
    /// Example: `settings.glass_lights.check()?;`
    pub fn check(&self) -> Result<()> {
        let is_valid: bool = self.low_brightness_frac >= 0.0
            && self.low_brightness_frac <= self.high_brightness_frac
            && self.period_s > 0.0
            && self.rising_frac > 0.0
            && self.rising_frac < 1.0;
        if !is_valid {
            return Err(format!(
                "glass_lights needs 0 <= low_brightness_frac <= high_brightness_frac, period_s above 0 and rising_frac between 0 and 1, not {}, {}, {} and {}",
                self.low_brightness_frac, self.high_brightness_frac, self.period_s, self.rising_frac
            )
            .into());
        }
        Ok(())
    }

    /// `get_brightness_frac` gives the lights' brightness at `time_s`, as a share of full: with
    /// `p` the share of the period gone by, `p / rising_frac` of the way from low to high while
    /// `p < rising_frac`, and `(1 - p) / (1 - rising_frac)` of the way after.
    ///
    /// Example: `let brightness_frac: f64 = settings.glass_lights.get_brightness_frac(now_s);`
    pub fn get_brightness_frac(&self, time_s: f64) -> f64 {
        let phase_frac: f64 = time_s.rem_euclid(self.period_s) / self.period_s;
        let wave_frac: f64 = if phase_frac < self.rising_frac {
            phase_frac / self.rising_frac
        } else {
            (1.0 - phase_frac) / (1.0 - self.rising_frac)
        };
        self.low_brightness_frac
            + (self.high_brightness_frac - self.low_brightness_frac) * wave_frac
    }
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
    #[serde(default)]
    pub shooting: ShootingSettings,
    pub when_hit: WhenHitSettings,
    pub flashlight: FlashlightSettings,
    pub map_edge: MapEdgeSettings,
    pub aim: AimSettings,
    pub wormholes: WormholeSettings,
    pub space: SpaceSettings,
    pub black_hole: BlackHoleSettings,
    #[serde(default)]
    pub gravity: GravitySettings,
    pub glass: GlassSettings,
    #[serde(default)]
    pub team_domes: TeamDomesSettings,
    #[serde(default)]
    pub glass_lights: GlassLightsSettings,
}

/// `check_numbers` fails, naming the key and `source`, on the first number in `value` that is
/// negative or not finite, or zero outside `zero_allowed`.
///
/// Example: `check_numbers(&serde_json::to_value(&settings)?, &SERVER_ZERO_ALLOWED_KEYS, "fast.yaml")?;`
fn check_numbers(value: &Value, zero_allowed: &[&str], source: &str) -> Result<()> {
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
fn read_text(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)
        .map_err(|error| format!("settings {}: {error}", path.display()))?)
}

/// `drop_removed_keys` deletes from a settings file's YAML each dotted key of `removed` (such as
/// `view.wormhole_tint_s`) that it has, so a file an older version saved still loads; every other
/// unknown key still fails.
///
/// Example: `drop_removed_keys(&mut yaml, &REMOVED_PLAYER_KEYS);`
fn drop_removed_keys(yaml: &mut serde_yaml::Value, removed: &[&str]) {
    for key in removed {
        let mut parts: Vec<&str> = key.split('.').collect();
        let name: &str = parts.pop().expect("a key has at least one part");
        let parent: Option<&mut serde_yaml::Value> = parts
            .iter()
            .try_fold(&mut *yaml, |node, part| node.get_mut(*part));
        if let Some(serde_yaml::Value::Mapping(fields)) = parent {
            fields.remove(name);
        }
    }
}

/// `add_missing_keys` gives a settings file's YAML each dotted key of `added` (such as
/// `keys.switch_view`, added after players began saving their settings) that it lacks, with its
/// value in `defaults`, so a file an older version saved still loads; every other missing key
/// still fails. A key of `added` that `defaults` lacks is a bug, and panics.
///
/// Example: `add_missing_keys(&mut yaml, &defaults, &ADDED_PLAYER_KEYS);`
fn add_missing_keys(yaml: &mut serde_yaml::Value, defaults: &serde_yaml::Value, added: &[&str]) {
    for key in added {
        let mut parts: Vec<&str> = key.split('.').collect();
        let default: &serde_yaml::Value = parts
            .iter()
            .try_fold(defaults, |node, part| node.get(*part))
            .unwrap_or_else(|| panic!("add_missing_keys: the defaults have no {key}"));
        let name: &str = parts.pop().expect("a key has at least one part");
        let parent: Option<&mut serde_yaml::Value> = parts
            .iter()
            .try_fold(&mut *yaml, |node, part| node.get_mut(*part));
        if let Some(serde_yaml::Value::Mapping(fields)) = parent
            && !fields.contains_key(name)
        {
            fields.insert(name.into(), default.clone());
        }
    }
}

impl ServerSettings {
    /// `FromYaml` reads server settings from YAML text, naming `source` in errors, and checks every
    /// number (`check_numbers`) and the numbers that bound each other or have a range.
    ///
    /// Example: `let settings = ServerSettings::FromYaml(&text, "fast.yaml")?;`
    #[allow(non_snake_case)]
    fn FromYaml(text: &str, source: &str) -> Result<ServerSettings> {
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
        settings
            .gravity
            .check()
            .map_err(|error| format!("settings {source}: {error}"))?;
        settings
            .team_domes
            .check()
            .map_err(|error| format!("settings {source}: {error}"))?;
        settings
            .glass_lights
            .check()
            .map_err(|error| format!("settings {source}: {error}"))?;
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

#[cfg(target_os = "macos")]
impl PerPlayerSettings {
    /// `FromYaml` reads a player's settings from YAML text, naming `source` in errors: the keys
    /// (`KeyBindings::FromNames`), and every number checked as `check_numbers` does, with
    /// `view.wormhole_tint_frac` and `view.milky_way_brightness_frac` at most 1, and `visuals` in its
    /// ranges (`VisualsSettings::check`). The keys in
    /// `REMOVED_PLAYER_KEYS`, which older versions saved, are dropped (`drop_removed_keys`), and those in `ADDED_PLAYER_KEYS`, which
    /// older versions didn't save, take their built-in values when missing (`add_missing_keys`).
    /// Keys saved before W-140 (hold Tab for the map, Esc for the menu) are upgraded
    /// (`key_bindings::upgrade_saved_names`).
    ///
    /// Example: `let mine = PerPlayerSettings::FromYaml(&text, "mine.yaml")?;`
    #[allow(non_snake_case)]
    fn FromYaml(text: &str, source: &str) -> Result<PerPlayerSettings> {
        let mut yaml: serde_yaml::Value =
            serde_yaml::from_str(text).map_err(|error| format!("settings {source}: {error}"))?;
        drop_removed_keys(&mut yaml, &REMOVED_PLAYER_KEYS);
        let defaults: serde_yaml::Value = serde_yaml::from_str(BUILTIN_PLAYER_YAML)
            .expect("the built-in player_settings.yaml is YAML (tested)");
        add_missing_keys(&mut yaml, &defaults, &ADDED_PLAYER_KEYS);
        let mut file: PlayerFile =
            serde_yaml::from_value(yaml).map_err(|error| format!("settings {source}: {error}"))?;
        key_bindings::upgrade_saved_names(&mut file.keys);
        let numbers: Value = serde_json::json!({"view": file.view, "hit_effects": file.hit_effects, "minimap": file.minimap});
        check_numbers(&numbers, &PLAYER_ZERO_ALLOWED_KEYS, source)?;
        for (key, value_frac) in [
            ("view.wormhole_tint_frac", file.view.wormhole_tint_frac),
            (
                "view.milky_way_brightness_frac",
                file.view.milky_way_brightness_frac,
            ),
        ] {
            if value_frac > 1.0 {
                return Err(format!(
                    "settings {source}: {key} must be at most 1, not {value_frac}"
                )
                .into());
            }
        }
        file.visuals.check(source)?;
        Ok(PerPlayerSettings {
            keys: KeyBindings::FromNames(&file.keys, source)?,
            view: file.view,
            hit_effects: file.hit_effects,
            minimap: file.minimap,
            visuals: file.visuals,
        })
    }

    /// `Builtin` gives the settings in `player_settings.yaml` as this program was built with it.
    ///
    /// Example: `let mine = PerPlayerSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> PerPlayerSettings {
        PerPlayerSettings::FromYaml(BUILTIN_PLAYER_YAML, "player_settings.yaml (built in)")
            .expect("the built-in player_settings.yaml is valid (tested)")
    }

    /// `to_yaml` gives the settings as YAML text that `Load` reads back (without the notes).
    ///
    /// Example: `std::fs::write(path, mine.to_yaml())?;`
    pub fn to_yaml(&self) -> String {
        let file: PlayerFile = PlayerFile {
            keys: self.keys.to_names(),
            view: self.view.clone(),
            hit_effects: self.hit_effects.clone(),
            minimap: self.minimap.clone(),
            visuals: self.visuals,
        };
        serde_yaml::to_string(&file).expect("player settings serialize to YAML")
    }

    /// `Load` reads a player's settings from a YAML file like `player_settings.yaml`.
    ///
    /// Example: `let mine = PerPlayerSettings::Load(Path::new("mine.yaml"))?;`
    #[allow(non_snake_case)]
    pub fn Load(path: &Path) -> Result<PerPlayerSettings> {
        PerPlayerSettings::FromYaml(&read_text(path)?, &path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    //! Tests of reading and checking both settings files.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Source | the built-in files, server settings sent as JSON, player settings the menu saved | `builtin_settings_read_and_survive_json`, `saved_player_settings_load_back` |
    //! | Keys | all present, one unknown, one missing | `builtin_settings_read_and_survive_json`, `unknown_and_missing_keys_fail` |
    //! | Numbers | above 0, 0 where allowed, 0 elsewhere, negative; in either file | `builtin_settings_read_and_survive_json`, `bad_numbers_name_their_key` |
    //! | Numbers that bound each other | in order (built in), out of order (a zoom below 1, a map edge that stops before it starts, a rifle's noise or beam speckle that shrinks with heat, a speckle share above 1, speckle kinds that don't add up to 1) | `builtin_settings_read_and_survive_json`, `bad_numbers_name_their_key` |
    //! | Wormhole tint strength | 0, between 0 and 1, 1 (built in), above 1 | `wormhole_tint_takes_0_to_1`, `builtin_settings_read_and_survive_json` |
    //! | Removed key (`view.wormhole_tint_s`) | absent (built in), present in an old save (dropped, loads), a similar key never removed (fails) | `builtin_settings_read_and_survive_json`, `removed_keys_are_dropped_and_others_still_fail` |
    //! | Curvature per map curvature | below 1, 1, between 1 and 2 (1.5, built in), 2, above 2 | `curvature_takes_1_to_2`, `builtin_settings_read_and_survive_json` |
    //! | Glass lights (W-203) | the built-in pulse at its low, half way up, its high, half way down, a period later, a time before 0; a file without the section; low 0, low above high, a shorter period, a rising share of 1 and of a half | `glass_lights_follow_an_uneven_triangle` |
    //! | Team dome size (W-148) | 0.2 (built in), a file without the section (0.2), small, a half, 0, above a half | `dome_size_takes_above_0_to_a_half` |
    //! | Added key (`keys.switch_view`, `view.milky_way_brightness_frac`, the four `view.head_bob` sway and swing keys) | present (built in), missing in an old save (gets its built-in value), another key missing (fails) | `builtin_settings_read_and_survive_json`, `added_keys_take_their_defaults_and_others_still_fail` |
    //! | Menu and map keys (W-140, hold Tab for the map, Esc for the menu) | current (built in), saved before (`release_mouse`, `open_menu` on `Tab`: loads with `Escape` for the menu and `Tab` for the map), `release_mouse` beside `show_map` (fails) | `builtin_settings_read_and_survive_json`, `saves_from_before_the_held_map_open_the_menu_with_escape` |
    //! | Visuals (W-51, basic visuals) | the identity (built in), a save without the section (gets the identity), a value out of its range (fails, naming it) | `saves_without_visuals_load_with_the_identity` |
    //! | Milky Way brightness | 0, between 0 and 1, 1 (built in), above 1 | `milky_way_brightness_takes_0_to_1`, `builtin_settings_read_and_survive_json` |
    //! | Slowest time rate near the black hole | 0, between 0 and 1 (0.1, built in), 1, above 1 | `min_time_rate_takes_above_0_to_1`, `builtin_settings_read_and_survive_json` |
    //! | Time dilation per general relativity | 0 (allowed: none), 15 (built in) | `min_time_rate_takes_above_0_to_1`, `builtin_settings_read_and_survive_json` |

    use super::*;
    use crate::key_bindings::{Action, Control};
    use sdl3::keyboard::Scancode;

    #[test]
    fn builtin_settings_read_and_survive_json() {
        let settings: ServerSettings = ServerSettings::Builtin();
        assert_eq!(
            ServerSettings::FromJson(&settings.to_json()).expect("valid JSON"),
            settings
        );
        assert!(PerPlayerSettings::Builtin().view.look_rad_per_px > 0.0);
    }

    #[test]
    fn saved_player_settings_load_back() {
        let mut mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        mine.view.motion_blur_s = 0.0;
        mine.keys
            .set_controls(Action::Jump, vec![Control::Key(Scancode::J)])
            .expect("J can jump");
        let saved: String = mine.to_yaml();
        let loaded: PerPlayerSettings =
            PerPlayerSettings::FromYaml(&saved, "test").expect("saved settings load");
        assert_eq!(loaded.to_yaml(), saved);
        assert_eq!(loaded.keys.get_names(Action::Jump), "J");
    }

    #[test]
    fn unknown_and_missing_keys_fail() {
        let unknown: String = BUILTIN_SERVER_YAML.replace("drain_s:", "drain_seconds:");
        assert!(ServerSettings::FromYaml(&unknown, "test").is_err());
        let missing: String = BUILTIN_SERVER_YAML.replace("  refill_walking_s: 15.0", "");
        assert!(ServerSettings::FromYaml(&missing, "test").is_err());
        let unknown_view: String = BUILTIN_PLAYER_YAML.replace("motion_blur_s:", "blur_s:");
        assert!(PerPlayerSettings::FromYaml(&unknown_view, "test").is_err());
    }

    #[test]
    fn bad_numbers_name_their_key() {
        let zero_blur: String =
            BUILTIN_PLAYER_YAML.replace("motion_blur_s: 0.01", "motion_blur_s: 0");
        assert!(PerPlayerSettings::FromYaml(&zero_blur, "test").is_ok());
        let no_reload: String = BUILTIN_SERVER_YAML.replace("reload_s: 2.0", "reload_s: 0");
        assert!(ServerSettings::FromYaml(&no_reload, "test").is_ok());
        for (old, new, key) in [
            ("drain_s: 7.5", "drain_s: 0", "sprint_energy.drain_s"),
            ("shots_per_s: 8.0", "shots_per_s: -1", "rifle.shots_per_s"),
        ] {
            let error: String =
                ServerSettings::FromYaml(&BUILTIN_SERVER_YAML.replace(old, new), "test")
                    .expect_err("a bad number fails")
                    .to_string();
            assert!(error.contains(key), "{error}");
        }
        let shrinking_aim: String =
            BUILTIN_SERVER_YAML.replace("rifle_zoom: 1.5", "rifle_zoom: 0.5");
        let error: String = ServerSettings::FromYaml(&shrinking_aim, "test")
            .expect_err("a zoom below 1 fails")
            .to_string();
        assert!(error.contains("aim"), "{error}");
        let shrinking_noise: String =
            BUILTIN_SERVER_YAML.replace("noise_start_rad: 0.002", "noise_start_rad: 0.5");
        let error: String = ServerSettings::FromYaml(&shrinking_noise, "test")
            .expect_err("a rifle's noise that shrinks with heat fails")
            .to_string();
        assert!(error.contains("noise_start_rad"), "{error}");
        for bad in [
            BUILTIN_SERVER_YAML.replace(
                "beam_speckle_start_frac: 0.0",
                "beam_speckle_start_frac: 0.7",
            ),
            BUILTIN_SERVER_YAML.replace("beam_speckle_max_frac: 0.5", "beam_speckle_max_frac: 1.5"),
            BUILTIN_SERVER_YAML
                .replace("beam_speckle_mean_frac: 0.2", "beam_speckle_mean_frac: 1.5"),
            BUILTIN_SERVER_YAML.replace("dropout_frac: 0.8", "dropout_frac: 0.7"),
        ] {
            let error: String = ServerSettings::FromYaml(&bad, "test")
                .expect_err("a bad beam speckle fails")
                .to_string();
            assert!(error.contains("beam speckles"), "{error}");
        }
        let all_dropout: String = BUILTIN_SERVER_YAML
            .replace("black_frac: 0.1", "black_frac: 0")
            .replace("white_frac: 0.1", "white_frac: 0")
            .replace("dropout_frac: 0.8", "dropout_frac: 1.0");
        assert!(ServerSettings::FromYaml(&all_dropout, "test").is_ok());
        let backward_edge: String =
            BUILTIN_SERVER_YAML.replace("start_wall_frac: 0.0", "start_wall_frac: 0.6");
        let error: String = ServerSettings::FromYaml(&backward_edge, "test")
            .expect_err("a stop below the start fails")
            .to_string();
        assert!(error.contains("map_edge"), "{error}");
        let still_view: String =
            BUILTIN_PLAYER_YAML.replace("look_rad_per_px: 0.0025", "look_rad_per_px: 0");
        let error: String = PerPlayerSettings::FromYaml(&still_view, "test")
            .err()
            .expect("a bad number fails")
            .to_string();
        assert!(error.contains("view.look_rad_per_px"), "{error}");
    }

    #[test]
    fn wormhole_tint_takes_0_to_1() {
        let none: String =
            BUILTIN_PLAYER_YAML.replace("wormhole_tint_frac: 1.0", "wormhole_tint_frac: 0");
        assert!(PerPlayerSettings::FromYaml(&none, "test").is_ok());
        let half: String =
            BUILTIN_PLAYER_YAML.replace("wormhole_tint_frac: 1.0", "wormhole_tint_frac: 0.5");
        assert!(PerPlayerSettings::FromYaml(&half, "test").is_ok());
        let too_strong: String =
            BUILTIN_PLAYER_YAML.replace("wormhole_tint_frac: 1.0", "wormhole_tint_frac: 1.5");
        let error: String = PerPlayerSettings::FromYaml(&too_strong, "test")
            .err()
            .expect("a tint above 1 fails")
            .to_string();
        assert!(error.contains("view.wormhole_tint_frac"), "{error}");
    }

    #[test]
    fn milky_way_brightness_takes_0_to_1() {
        const BUILTIN_LINE: &str = "milky_way_brightness_frac: 1.0";
        let with_brightness = |value: &str| -> String {
            BUILTIN_PLAYER_YAML
                .replace(BUILTIN_LINE, &format!("milky_way_brightness_frac: {value}"))
        };
        assert!(BUILTIN_PLAYER_YAML.contains(BUILTIN_LINE));
        for value in ["0", "0.5"] {
            let loaded: PerPlayerSettings =
                PerPlayerSettings::FromYaml(&with_brightness(value), "test").expect("0 to 1 loads");
            assert_eq!(
                loaded.view.milky_way_brightness_frac.to_string(),
                value.to_string()
            );
        }
        let error: String = PerPlayerSettings::FromYaml(&with_brightness("1.5"), "test")
            .err()
            .expect("a brightness above 1 fails")
            .to_string();
        assert!(error.contains("view.milky_way_brightness_frac"), "{error}");
    }

    #[test]
    fn removed_keys_are_dropped_and_others_still_fail() {
        const OLD_TINT_LINE: &str = "\n  wormhole_tint_s: 1.5  # removed by W-82";
        let with_tint_line = |line: &str| -> String {
            let tint_frac_line: &str = "\n  wormhole_tint_frac: 1.0";
            BUILTIN_PLAYER_YAML.replacen(tint_frac_line, &format!("{tint_frac_line}{line}"), 1)
        };
        let old_save: String = with_tint_line(OLD_TINT_LINE);
        assert_ne!(old_save, BUILTIN_PLAYER_YAML);
        let loaded: PerPlayerSettings =
            PerPlayerSettings::FromYaml(&old_save, "test").expect("an old save loads");
        assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        let typo: String = with_tint_line("\n  wormhole_tint_seconds: 1.5");
        let error: String = PerPlayerSettings::FromYaml(&typo, "test")
            .err()
            .expect("a key that was never removed fails")
            .to_string();
        assert!(error.contains("wormhole_tint_seconds"), "{error}");
    }

    #[test]
    fn curvature_takes_1_to_2() {
        const BUILTIN_CURVATURE_PER_MAP_CURVATURE: f64 = 1.5;
        let builtin: f64 = ServerSettings::Builtin().space.curvature_per_map_curvature;
        assert_eq!(builtin, BUILTIN_CURVATURE_PER_MAP_CURVATURE);
        for (factor, is_valid) in [("1.0", true), ("2.0", true), ("0.5", false), ("2.5", false)] {
            let yaml: String = BUILTIN_SERVER_YAML.replace(
                "curvature_per_map_curvature: 1.5",
                &format!("curvature_per_map_curvature: {factor}"),
            );
            let result: Result<ServerSettings> = ServerSettings::FromYaml(&yaml, "test");
            assert_eq!(result.is_ok(), is_valid, "{factor}");
            if let Err(error) = result {
                assert!(
                    error
                        .to_string()
                        .contains("space.curvature_per_map_curvature"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn glass_lights_follow_an_uneven_triangle() {
        let lights: GlassLightsSettings = ServerSettings::Builtin().glass_lights;
        assert_eq!(lights, BUILTIN_GLASS_LIGHTS);
        for (time_s, expected_frac) in [
            (0.0, 0.3),
            (5.0, 0.5),
            (10.0, 0.7),
            (13.0, 0.5),
            (16.0, 0.3),
            (26.0, 0.7),
            (-3.0, 0.5),
        ] {
            let brightness_frac: f64 = lights.get_brightness_frac(time_s);
            assert!(
                (brightness_frac - expected_frac).abs() < 1e-12,
                "{time_s}: {brightness_frac}"
            );
        }
        let section_at: usize = BUILTIN_SERVER_YAML
            .find("glass_lights:")
            .expect("the section");
        let without: ServerSettings =
            ServerSettings::FromYaml(&BUILTIN_SERVER_YAML[..section_at], "test")
                .expect("no section");
        assert_eq!(without.glass_lights, BUILTIN_GLASS_LIGHTS);
        for (old, new, is_valid) in [
            ("low_brightness_frac: 0.3", "low_brightness_frac: 0.0", true),
            (
                "low_brightness_frac: 0.3",
                "low_brightness_frac: 0.8",
                false,
            ),
            ("period_s: 16.0", "period_s: 4.0", true),
            ("rising_frac: 0.625", "rising_frac: 1.0", false),
            ("rising_frac: 0.625", "rising_frac: 0.5", true),
        ] {
            assert!(BUILTIN_SERVER_YAML.contains(old), "{old}");
            let result: Result<ServerSettings> =
                ServerSettings::FromYaml(&BUILTIN_SERVER_YAML.replace(old, new), "test");
            assert_eq!(result.is_ok(), is_valid, "{new}");
            if let Err(error) = result {
                assert!(error.to_string().contains("glass_lights"), "{error}");
            }
        }
    }

    #[test]
    fn dome_size_takes_above_0_to_a_half() {
        let builtin: TeamDomesSettings = ServerSettings::Builtin().team_domes;
        assert_eq!(builtin.circumference_frac, BUILTIN_DOME_CIRCUMFERENCE_FRAC);
        let section_at: usize = BUILTIN_SERVER_YAML
            .find("team_domes:")
            .expect("the section");
        let without: ServerSettings =
            ServerSettings::FromYaml(&BUILTIN_SERVER_YAML[..section_at], "test")
                .expect("no section");
        assert_eq!(without.team_domes, TeamDomesSettings::default());
        for (frac, is_valid) in [
            ("0.05", true),
            ("0.5", true),
            ("0.0", false),
            ("0.6", false),
        ] {
            let yaml: String = BUILTIN_SERVER_YAML.replace(
                "circumference_frac: 0.2",
                &format!("circumference_frac: {frac}"),
            );
            let result: Result<ServerSettings> = ServerSettings::FromYaml(&yaml, "test");
            assert_eq!(result.is_ok(), is_valid, "{frac}");
            if let Err(error) = result {
                assert!(
                    error.to_string().contains("team_domes.circumference_frac"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn min_time_rate_takes_above_0_to_1() {
        let with_values = |rate_frac: &str, dilation: &str| -> Result<ServerSettings> {
            let yaml: String = BUILTIN_SERVER_YAML
                .replace(
                    "min_time_rate_frac: 0.1",
                    &format!("min_time_rate_frac: {rate_frac}"),
                )
                .replace(
                    "time_dilation_per_general_relativity: 15.0",
                    &format!("time_dilation_per_general_relativity: {dilation}"),
                );
            ServerSettings::FromYaml(&yaml, "test")
        };
        for (rate_frac, is_valid) in [("0.0", false), ("1.0", true), ("1.5", false)] {
            let result: Result<ServerSettings> = with_values(rate_frac, "15.0");
            assert_eq!(result.is_ok(), is_valid, "{rate_frac}");
            if let Err(error) = result {
                assert!(
                    error.to_string().contains("black_hole.min_time_rate_frac"),
                    "{error}"
                );
            }
        }
        assert!(with_values("0.1", "0.0").is_ok());
    }

    #[test]
    fn added_keys_take_their_defaults_and_others_still_fail() {
        let without_key = |name: &str| -> String {
            let prefix: String = format!("{name}:");
            let lines: Vec<&str> = BUILTIN_PLAYER_YAML
                .lines()
                .filter(|line| !line.trim_start().starts_with(&prefix))
                .collect();
            lines.join("\n")
        };
        for name in [
            "switch_view",
            "milky_way_brightness_frac",
            "gun_walk_sway_m",
            "gun_sprint_sway_m",
            "rifle_walk_swing_rad",
            "rifle_sprint_swing_rad",
        ] {
            let old_save: String = without_key(name);
            assert!(!old_save.contains(name));
            let loaded: PerPlayerSettings =
                PerPlayerSettings::FromYaml(&old_save, "test").expect("an old save loads");
            assert!(
                loaded
                    .keys
                    .matches(Action::SwitchView, Control::Key(Scancode::V))
            );
            assert_eq!(loaded.view.milky_way_brightness_frac, 1.0);
            assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        }
        let error: String = PerPlayerSettings::FromYaml(&without_key("flashlight"), "test")
            .err()
            .expect("a key that wasn't added later still fails when missing")
            .to_string();
        assert!(error.contains("flashlight"), "{error}");
    }

    #[test]
    fn saves_from_before_the_held_map_open_the_menu_with_escape() {
        const SHOW_MAP_LINE: &str = "  show_map: [Tab]";
        const OPEN_MENU_LINE: &str = "  open_menu: [Escape]";
        const OLD_RELEASE_MOUSE_LINE: &str = "  release_mouse: [Escape]";
        let old_save: String = BUILTIN_PLAYER_YAML
            .replacen(SHOW_MAP_LINE, OLD_RELEASE_MOUSE_LINE, 1)
            .replacen(OPEN_MENU_LINE, "  open_menu: [Tab]", 1);
        assert!(!old_save.contains("show_map") && old_save.contains("open_menu: [Tab]"));
        let loaded: PerPlayerSettings =
            PerPlayerSettings::FromYaml(&old_save, "test").expect("an old save loads");
        assert!(
            loaded
                .keys
                .matches(Action::OpenMenu, Control::Key(Scancode::Escape))
        );
        assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        let both: String = BUILTIN_PLAYER_YAML.replacen(
            SHOW_MAP_LINE,
            &format!("{SHOW_MAP_LINE}\n{OLD_RELEASE_MOUSE_LINE}"),
            1,
        );
        let error: String = PerPlayerSettings::FromYaml(&both, "test")
            .err()
            .expect("release_mouse beside show_map still fails")
            .to_string();
        assert!(error.contains("release_mouse"), "{error}");
    }

    #[test]
    fn saves_without_visuals_load_with_the_identity() {
        assert_eq!(
            PerPlayerSettings::Builtin().visuals,
            VisualsSettings::IDENTITY
        );
        let mut is_in_visuals: bool = false;
        let old_save: String = BUILTIN_PLAYER_YAML
            .lines()
            .filter(|line| {
                if !line.is_empty() && !line.starts_with(' ') {
                    is_in_visuals = line.starts_with("visuals:");
                }
                !is_in_visuals
            })
            .collect::<Vec<&str>>()
            .join("\n");
        assert!(!old_save.contains("brightness:") && old_save.contains("keys:"));
        let loaded: PerPlayerSettings =
            PerPlayerSettings::FromYaml(&old_save, "test").expect("a save without visuals loads");
        assert_eq!(loaded.visuals, VisualsSettings::IDENTITY);
        assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        let too_bright: String =
            BUILTIN_PLAYER_YAML.replacen("  brightness: 1.0", "  brightness: 9.0", 1);
        let error: String = PerPlayerSettings::FromYaml(&too_bright, "test")
            .err()
            .expect("a brightness of 9 fails")
            .to_string();
        assert!(error.contains("visuals.brightness"), "{error}");
    }
}
