//! `settings.rs` reads the game's two settings files, so players can tune it without rebuilding:
//! `server_settings.yaml`, the numbers that shape play for everyone (movement, sprint energy, both
//! guns, consoles, respawning, how a hit slows, the flashlight, thrown wormholes, how curved space
//! is, the reality bend), which the game server sends to every player who joins; and
//! `player_settings.yaml`, each player's own choices (keys, mouse speed, head bob, motion blur, how
//! a hit looks).
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

use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::key_bindings::KeyBindings;

const BUILTIN_SERVER_YAML: &str = include_str!("../server_settings.yaml");
const BUILTIN_PLAYER_YAML: &str = include_str!("../player_settings.yaml");
pub const MIN_CURVATURE_PER_MAP_CURVATURE: f64 = 1.0;
pub const MAX_CURVATURE_PER_MAP_CURVATURE: f64 = 2.0;
const SERVER_ZERO_ALLOWED_KEYS: [&str; 18] = [
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
];
const SPECKLE_KINDS_SUM_TOLERANCE_FRAC: f64 = 1e-9;
const PLAYER_ZERO_ALLOWED_KEYS: [&str; 8] = [
    "view.motion_blur_s",
    "view.edge_stretch_frac",
    "view.wormhole_tint_frac",
    "view.head_bob.walk_depth_m",
    "view.head_bob.sprint_depth_m",
    "hit_effects.red_frac",
    "hit_effects.shake_px",
    "hit_effects.blur_px",
];
const REMOVED_PLAYER_KEYS: [&str; 1] = ["view.wormhole_tint_s"];
const ADDED_PLAYER_KEYS: [&str; 1] = ["keys.switch_view"];

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `MovementSettings` is walking and sprinting speed as multiples of the map's, a jump's height and
/// time in the air as a fraction of the map's jump (gravity follows from the two), and crouching:
/// its speed and eye height as fractions of walking's, and how long it takes to crouch or stand.
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

/// `HeadBobSettings` is how deep the eye dips at each step, how many steps a second, and how long
/// the bob takes to start or stop.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct HeadBobSettings {
    pub walk_depth_m: f64,
    pub sprint_depth_m: f64,
    pub walk_steps_per_s: f64,
    pub sprint_steps_per_s: f64,
    pub fade_s: f64,
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

/// `RealityBendSettings` is the reality bend (W-68): the edge radius beyond which the bent map is
/// cut away, how long the bend eases to a new strength, and the black hole's Schwarzschild radius
/// and reach at full strength.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RealityBendSettings {
    pub edge_radius_m: f64,
    pub transition_s: f64,
    pub max_schwarzschild_radius_m: f64,
    pub max_reach_m: f64,
}

/// `ViewSettings` is how a player's view moves: how far it turns for each pixel the mouse moves, how
/// long motion smears, how strongly the view stretches near the map's edge, how strongly the scene
/// shifts colour (light cyan forward, dark red backward) while moving through a wormhole's throat
/// (0 to 1), and the head bob.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ViewSettings {
    pub look_rad_per_px: f64,
    pub motion_blur_s: f64,
    pub edge_stretch_frac: f64,
    pub wormhole_tint_frac: f64,
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

/// `PlayerFile` is `player_settings.yaml` as written: the keys by name, the view and how a hit looks.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayerFile {
    keys: BTreeMap<String, Vec<String>>,
    view: ViewSettings,
    hit_effects: HitEffectSettings,
    minimap: MinimapSettings,
}

/// `PerPlayerSettings` is everything in `player_settings.yaml`, with the keys read into controls.
pub struct PerPlayerSettings {
    pub keys: KeyBindings,
    pub view: ViewSettings,
    pub hit_effects: HitEffectSettings,
    pub minimap: MinimapSettings,
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
    pub reality_bend: RealityBendSettings,
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

impl PerPlayerSettings {
    /// `FromYaml` reads a player's settings from YAML text, naming `source` in errors: the keys
    /// (`KeyBindings::FromNames`), and every number checked as `check_numbers` does, with
    /// `view.wormhole_tint_frac` at most 1. The keys in `REMOVED_PLAYER_KEYS`, which older
    /// versions saved, are dropped (`drop_removed_keys`), and those in `ADDED_PLAYER_KEYS`, which
    /// older versions didn't save, take their built-in values when missing (`add_missing_keys`).
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
        let file: PlayerFile =
            serde_yaml::from_value(yaml).map_err(|error| format!("settings {source}: {error}"))?;
        let numbers: Value = serde_json::json!({"view": file.view, "hit_effects": file.hit_effects, "minimap": file.minimap});
        check_numbers(&numbers, &PLAYER_ZERO_ALLOWED_KEYS, source)?;
        if file.view.wormhole_tint_frac > 1.0 {
            return Err(format!(
                "settings {source}: view.wormhole_tint_frac must be at most 1, not {}",
                file.view.wormhole_tint_frac
            )
            .into());
        }
        Ok(PerPlayerSettings {
            keys: KeyBindings::FromNames(&file.keys, source)?,
            view: file.view,
            hit_effects: file.hit_effects,
            minimap: file.minimap,
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
    //! | Added key (`keys.switch_view`) | present (built in), missing in an old save (gets the built-in `V`), another key missing (fails) | `builtin_settings_read_and_survive_json`, `added_keys_take_their_defaults_and_others_still_fail` |

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
    fn added_keys_take_their_defaults_and_others_still_fail() {
        let without_key = |name: &str| -> String {
            let prefix: String = format!("{name}:");
            let lines: Vec<&str> = BUILTIN_PLAYER_YAML
                .lines()
                .filter(|line| !line.trim_start().starts_with(&prefix))
                .collect();
            lines.join("\n")
        };
        let old_save: String = without_key("switch_view");
        assert!(!old_save.contains("switch_view"));
        let loaded: PerPlayerSettings =
            PerPlayerSettings::FromYaml(&old_save, "test").expect("an old save loads");
        assert!(
            loaded
                .keys
                .matches(Action::SwitchView, Control::Key(Scancode::V))
        );
        assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        let error: String = PerPlayerSettings::FromYaml(&without_key("flashlight"), "test")
            .err()
            .expect("a key that wasn't added later still fails when missing")
            .to_string();
        assert!(error.contains("flashlight"), "{error}");
    }
}
