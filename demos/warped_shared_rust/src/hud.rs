//! `hud.rs` is the heads-up display every level draws alike (W-126, same play in every level),
//! Lunar Surface's, as `OverlayRect`s for each level's renderer and text lines for its banner: the
//! crosshair; the bars in the bottom left (health, one segment per rifle body hit; the gun in
//! hand's ammo, the sniper's one block or the rifle's shots left before it overheats; the sprint
//! energy) with their pulses (`HudPulses`); the bar below the middle while holding `E` on a
//! console; the hit effect's look (`make_hit_effect`); and the banner's lobby and "YOU DIED"
//! lines.
//!
//! Interface: `get_crosshair_rects`, `HudBars`, `HudPulses`, `make_bar_rects`, `make_hit_effect`,
//! `get_lobby_lines`, `get_death_lines`, `describe_gun`, `ENERGY_SEGMENT_COUNT`.
//!
//! Example: `let rects: Vec<OverlayRect> = get_crosshair_rects((2560, 1440));`

use std::net::SocketAddr;

use crate::hud_bars::{self, BarPulses, PULSE_TUNING};
use crate::overlay::{FLAT_RECT_STYLE, HitEffect, NO_RECT_PULSE, OverlayRect};
use crate::play_settings::{HitEffectSettings, RifleSettings};
use crate::weapons::{Gun, Weapons};

pub const CROSSHAIR_GAP_FRAC: f32 = 0.006;
pub const CROSSHAIR_ARM_FRAC: f32 = 0.012;
pub const CROSSHAIR_THICKNESS_FRAC: f32 = 0.0025;
pub const CROSSHAIR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.85];
pub const HEAT_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.9];
pub const HEAT_BAR_RGB: [f32; 3] = [1.0, 0.6, 0.2];
pub const HEAT_BAR_OVERHEATED_RGB: [f32; 3] = [1.0, 0.15, 0.1];
pub const HEALTH_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.87];
pub const HEALTH_BAR_RGB: [f32; 3] = [0.35, 0.95, 0.45];
pub const HEALTH_PULSE_RGB: [f32; 3] = [1.0, 0.45, 0.2];
pub const ENERGY_SEGMENT_COUNT: u32 = 10;
pub const ENERGY_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.93];
pub const ENERGY_BAR_RGB: [f32; 3] = [0.5, 0.82, 1.0];
pub const ENERGY_PULSE_RGB: [f32; 3] = [0.8, 0.94, 1.0];
pub const DISABLE_BAR_MIN_FRAC: [f32; 2] = [0.4, 0.62];
pub const DISABLE_BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.02];
pub const DISABLE_BAR_RGB: [f32; 3] = [1.0, 0.85, 0.3];
pub const HIT_SHAKE_TURN_RAD: f64 = 2.399_963_229_728_653;
const PERCENT_PER_FRAC: f64 = 100.0;

/// `HudBars` is what the HUD's bars show: this player's health (`None` without a game server),
/// the sprint energy (`None` where nobody sprints, as on the house) and how far turning off a
/// console has gone (`None` while not holding `E` on one), each from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudBars {
    pub health_frac: Option<f64>,
    pub energy_frac: Option<f64>,
    pub console_hold_frac: Option<f64>,
}

/// `HudPulses` is the pulses running along the health and energy bars while they change.
pub struct HudPulses {
    pub health: BarPulses,
    pub energy: BarPulses,
}

impl HudPulses {
    /// `New` is both bars full, without pulses.
    ///
    /// Example: `let pulses = HudPulses::New();`
    #[allow(non_snake_case)]
    pub fn New() -> HudPulses {
        HudPulses {
            health: BarPulses::New(1.0),
            energy: BarPulses::New(1.0),
        }
    }

    /// `update` moves the pulses on by `dt_s` (`hud_bars.rs`), driven by how fast the health and
    /// the energy in `bars` change; a bar that `bars` leaves out keeps its pulses still. The
    /// health bar has one segment per rifle body hit (`rifle.body_hits_to_eliminate`).
    ///
    /// Example: `self.hud_pulses.update(dt_s, &bars, &self.settings.rifle);`
    pub fn update(&mut self, dt_s: f64, bars: &HudBars, rifle: &RifleSettings) {
        if let Some(health_frac) = bars.health_frac {
            let segment_count: u32 = u32::from(rifle.body_hits_to_eliminate);
            self.health
                .update(dt_s, health_frac, segment_count, &PULSE_TUNING);
        }
        if let Some(energy_frac) = bars.energy_frac {
            self.energy
                .update(dt_s, energy_frac, ENERGY_SEGMENT_COUNT, &PULSE_TUNING);
        }
    }
}

/// `make_bar_rects` gives the HUD's bars as segmented bars in the HUD's look (`hud_bars.rs`): in
/// the bottom left this player's health (if in `bars`; one segment per rifle body hit), the gun in
/// hand's ammo (the rifle's shots left before it overheats, red once overheated, or the sniper's
/// one block, empty while it reloads; no pulses), below the middle the console being turned off
/// (if any), and the sprint energy (if in `bars`); health and energy carry `pulses`.
///
/// Example: `rects.extend(make_bar_rects(&bars, &self.weapons, &self.settings.rifle, &self.hud_pulses));`
pub fn make_bar_rects(
    bars: &HudBars,
    weapons: &Weapons,
    rifle: &RifleSettings,
    pulses: &HudPulses,
) -> Vec<OverlayRect> {
    let mut rects: Vec<OverlayRect> = Vec::new();
    if let Some(health_frac) = bars.health_frac {
        let segment_count: u32 = u32::from(rifle.body_hits_to_eliminate);
        rects.extend(hud_bars::make_segmented_bar(
            HEALTH_BAR_MIN_FRAC,
            segment_count,
            health_frac,
            HEALTH_BAR_RGB,
            pulses.health.make_rect_pulse(
                HEALTH_BAR_MIN_FRAC,
                segment_count,
                health_frac,
                HEALTH_PULSE_RGB,
                HEALTH_BAR_RGB,
                &PULSE_TUNING,
            ),
        ));
    }
    rects.extend(match weapons.gun {
        Gun::Laser => hud_bars::make_segmented_bar(
            HEAT_BAR_MIN_FRAC,
            1,
            if weapons.sniper_reload_s <= 0.0 {
                1.0
            } else {
                0.0
            },
            HEAT_BAR_RGB,
            NO_RECT_PULSE,
        ),
        Gun::Rifle => hud_bars::make_segmented_bar(
            HEAT_BAR_MIN_FRAC,
            rifle.shots_until_overheat,
            1.0 - weapons.rifle_heat_frac.min(1.0),
            if weapons.is_overheated {
                HEAT_BAR_OVERHEATED_RGB
            } else {
                HEAT_BAR_RGB
            },
            NO_RECT_PULSE,
        ),
    });
    if let Some(hold_frac) = bars.console_hold_frac {
        rects.extend(hud_bars::make_progress_bar(
            DISABLE_BAR_MIN_FRAC,
            DISABLE_BAR_SIZE_FRAC,
            hold_frac.min(1.0) as f32,
            DISABLE_BAR_RGB,
        ));
    }
    if let Some(energy_frac) = bars.energy_frac {
        rects.extend(hud_bars::make_segmented_bar(
            ENERGY_BAR_MIN_FRAC,
            ENERGY_SEGMENT_COUNT,
            energy_frac,
            ENERGY_BAR_RGB,
            pulses.energy.make_rect_pulse(
                ENERGY_BAR_MIN_FRAC,
                ENERGY_SEGMENT_COUNT,
                energy_frac,
                ENERGY_PULSE_RGB,
                ENERGY_BAR_RGB,
                &PULSE_TUNING,
            ),
        ));
    }
    rects
}

/// `make_hit_effect` gives what the last hit on this player still does to the scene at
/// `effect_frac` (1 just after the hit, fading to 0: `health::fade_effect_frac`): red edges, a
/// blur from the edges inward, and a shake in a new direction each frame (`frame_count`, the
/// golden angle apart), all by the player settings' `hit_effects`.
///
/// Example: `let hit: HitEffect = make_hit_effect(0.5, 120, &self.player_settings.hit_effects);`
pub fn make_hit_effect(
    effect_frac: f64,
    frame_count: u64,
    hit_effects: &HitEffectSettings,
) -> HitEffect {
    let shake_rad: f64 = frame_count as f64 * HIT_SHAKE_TURN_RAD;
    let shake_px: f64 = hit_effects.shake_px * effect_frac;
    HitEffect {
        strength_frac: effect_frac as f32,
        red_frac: hit_effects.red_frac as f32,
        blur_px: hit_effects.blur_px as f32,
        shake_px: [shake_rad.cos(), shake_rad.sin()].map(|c| (c * shake_px) as f32),
    }
}

/// `get_lobby_lines` gives the lobby banner's two lines: until the server at `server` first
/// answers, that this window is connecting to it; once it has (`lobby_counts`, each team's head
/// count, blue then red), that it is connected, the counts and `join_keys_line`.
///
/// Example: `let lines = get_lobby_lines(Some([1, 2]), server, "1 joins blue, 2 joins red".to_string());`
pub fn get_lobby_lines(
    lobby_counts: Option<[u32; 2]>,
    server: SocketAddr,
    join_keys_line: String,
) -> [String; 2] {
    match lobby_counts {
        Some([blue, red]) => [format!("CONNECTED  BLUE {blue}  RED {red}"), join_keys_line],
        None => ["CONNECTING".to_string(), format!("server {server}")],
    }
}

/// `get_death_lines` gives the banner of a player out after a hit: "YOU DIED" and the whole
/// seconds until they respawn (`respawn_in_s`, rounded up, at least 1).
///
/// Example: `let [title, countdown] = get_death_lines(2.4);`
pub fn get_death_lines(respawn_in_s: f64) -> [String; 2] {
    [
        "YOU DIED".to_string(),
        format!("Respawning in {:.0}", respawn_in_s.ceil().max(1.0)),
    ]
}

/// `describe_gun` gives the HUD line's part about the gun in hand: gun 1, the laser, or gun 2, the
/// rifle with its heat, or overheated.
///
/// Example: `let gun: String = describe_gun(&self.weapons);`
pub fn describe_gun(weapons: &Weapons) -> String {
    match (weapons.gun, weapons.is_overheated) {
        (Gun::Laser, _) => ", gun 1 (laser)".to_string(),
        (Gun::Rifle, true) => ", gun 2 (rifle) overheated".to_string(),
        (Gun::Rifle, false) => format!(
            ", gun 2 (rifle) heat {:.0}%",
            weapons.rifle_heat_frac * PERCENT_PER_FRAC
        ),
    }
}

/// `get_crosshair_rects` gives the crosshair in the middle of a window of `window_px` pixels: four
/// arms `CROSSHAIR_ARM_FRAC` long, `CROSSHAIR_GAP_FRAC` from the middle, in fractions of the window's
/// height so they look the same at any size.
///
/// Example: `let rects = get_crosshair_rects((2560, 1600));`
pub fn get_crosshair_rects(window_px: (u32, u32)) -> Vec<OverlayRect> {
    let x_per_y: f32 = window_px.1 as f32 / window_px.0.max(1) as f32;
    let (near, far, half) = (
        CROSSHAIR_GAP_FRAC,
        CROSSHAIR_GAP_FRAC + CROSSHAIR_ARM_FRAC,
        CROSSHAIR_THICKNESS_FRAC / 2.0,
    );
    [(near, far), (-far, -near)]
        .into_iter()
        .flat_map(|(lo, hi)| {
            [
                ([lo * x_per_y, -half], [hi * x_per_y, half]),
                ([-half * x_per_y, lo], [half * x_per_y, hi]),
            ]
        })
        .map(|([x0, y0], [x1, y1])| OverlayRect {
            min_frac: [0.5 + x0, 0.5 + y0],
            max_frac: [0.5 + x1, 0.5 + y1],
            color: CROSSHAIR_COLOR,
            style: FLAT_RECT_STYLE,
            pulse: NO_RECT_PULSE,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tests of the HUD's bars and lines.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Bars given | the gun's only (the house without a server), health, energy and a console hold too | `bars_show_what_is_given` |
    //! | Gun in hand | the sniper (one block), the rifle (one segment per shot until overheat) | `bars_show_what_is_given`, `the_gun_line_names_the_gun` |
    //! | Rifle | cold, hot, overheated | `the_gun_line_names_the_gun` |
    //! | Respawn time left | a fraction of a second (1), between whole seconds (rounded up) | `the_death_banner_counts_whole_seconds` |
    //! | Server answered | not yet (connecting), yes (counts and keys) | `the_lobby_says_whether_connected` |

    use super::*;
    use crate::shot_noise::ShotNoise;
    use serde::Deserialize;

    const LUNAR_SERVER_YAML: &str =
        include_str!("../../hyperbolic_wormholes_rust/server_settings.yaml");
    const SEED: u64 = 7;
    const HALF_FRAC: f64 = 0.5;
    const PORT: u16 = 47810;

    /// `ServerSettings` is the rifle's section of Lunar Surface's `server_settings.yaml`.
    #[derive(Deserialize)]
    struct ServerSettings {
        rifle: RifleSettings,
    }

    /// `read_rifle` reads Lunar Surface's built-in rifle settings.
    fn read_rifle() -> RifleSettings {
        let settings: ServerSettings =
            serde_yaml::from_str(LUNAR_SERVER_YAML).expect("Lunar Surface's server settings");
        settings.rifle
    }

    #[test]
    fn bars_show_what_is_given() {
        let rifle: RifleSettings = read_rifle();
        let pulses: HudPulses = HudPulses::New();
        let mut weapons: Weapons = Weapons::New(ShotNoise::FromSeed(SEED));
        let gun_only: HudBars = HudBars {
            health_frac: None,
            energy_frac: None,
            console_hold_frac: None,
        };
        assert_eq!(
            make_bar_rects(&gun_only, &weapons, &rifle, &pulses).len(),
            rifle.shots_until_overheat as usize
        );
        weapons.switch_gun(Gun::Laser);
        assert_eq!(
            make_bar_rects(&gun_only, &weapons, &rifle, &pulses).len(),
            1
        );
        let all: HudBars = HudBars {
            health_frac: Some(HALF_FRAC),
            energy_frac: Some(HALF_FRAC),
            console_hold_frac: Some(HALF_FRAC),
        };
        let progress_count: usize = hud_bars::make_progress_bar(
            DISABLE_BAR_MIN_FRAC,
            DISABLE_BAR_SIZE_FRAC,
            HALF_FRAC as f32,
            DISABLE_BAR_RGB,
        )
        .len();
        assert_eq!(
            make_bar_rects(&all, &weapons, &rifle, &pulses).len(),
            usize::from(rifle.body_hits_to_eliminate)
                + 1
                + progress_count
                + ENERGY_SEGMENT_COUNT as usize
        );
    }

    #[test]
    fn the_gun_line_names_the_gun() {
        let mut weapons: Weapons = Weapons::New(ShotNoise::FromSeed(SEED));
        assert_eq!(describe_gun(&weapons), ", gun 2 (rifle) heat 0%");
        weapons.rifle_heat_frac = HALF_FRAC;
        assert_eq!(describe_gun(&weapons), ", gun 2 (rifle) heat 50%");
        weapons.is_overheated = true;
        assert_eq!(describe_gun(&weapons), ", gun 2 (rifle) overheated");
        weapons.switch_gun(Gun::Laser);
        assert_eq!(describe_gun(&weapons), ", gun 1 (laser)");
    }

    #[test]
    fn the_death_banner_counts_whole_seconds() {
        assert_eq!(get_death_lines(0.2), ["YOU DIED", "Respawning in 1"]);
        assert_eq!(get_death_lines(2.4), ["YOU DIED", "Respawning in 3"]);
    }

    #[test]
    fn the_lobby_says_whether_connected() {
        let server: SocketAddr = SocketAddr::from(([127, 0, 0, 1], PORT));
        assert_eq!(
            get_lobby_lines(None, server, String::new()),
            ["CONNECTING", "server 127.0.0.1:47810"]
        );
        assert_eq!(
            get_lobby_lines(Some([1, 2]), server, "keys".to_string()),
            ["CONNECTED  BLUE 1  RED 2", "keys"]
        );
    }
}
