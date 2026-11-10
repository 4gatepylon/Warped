//! `hud.rs` is the heads-up display every level draws alike (W-126, same play in every level),
//! Lunar Surface's, as `OverlayRect`s for each level's renderer and text lines for its banner: the
//! crosshair; the bars in the bottom left (health, one segment per rifle body hit; the gun in
//! hand's ammo, the sniper's one block or the rifle's shots left before it overheats; the sprint
//! energy) with their pulses (`HudPulses`); the bar below the middle while holding `E` on a
//! console; the row of squares at the top centre, one per console (pylon), dark once turned off,
//! with a box in this player's team colour around their team's (`make_pylon_square_rects`; W-146,
//! pylon squares and own-team box); the hit effect's look (`make_hit_effect`); and the banner's
//! lobby and "YOU DIED" lines, with the respawn bar under them that fills over the wait.
//!
//! Interface: `get_crosshair_rects`, `HudBars`, `HudPulses`, `AmmoBar`, `make_bar_rects`,
//! `make_ammo_bar_rects`, `PylonSquare`, `make_pylon_square_rects`, `make_team_box_rgba` (a menu's
//! box around this player's team), `TEAM_RGB`, `make_hit_effect`, `get_lobby_lines`,
//! `get_death_lines`, `get_respawn_filled_frac`, `make_respawn_bar_rects`, `describe_gun`,
//! `ENERGY_SEGMENT_COUNT`.
//!
//! Example: `let rects: Vec<OverlayRect> = get_crosshair_rects((2560, 1440));`

use std::net::SocketAddr;

use crate::hud_bars::{self, BarPulses, PULSE_TUNING};
use crate::overlay::{FLAT_RECT_STYLE, HitEffect, NO_RECT_PULSE, OverlayRect, RectStyle};
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
pub const RESPAWN_BAR_MIN_FRAC: [f32; 2] = [0.4, 0.63];
pub const RESPAWN_BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.02];
pub const RESPAWN_BAR_RGB: [f32; 3] = HEALTH_BAR_RGB;
pub const HIT_SHAKE_TURN_RAD: f64 = 2.399_963_229_728_653;
pub const TEAM_RGB: [[f32; 3]; 2] = [[0.35, 0.6, 1.0], [1.0, 0.35, 0.3]];
pub const PYLON_SQUARE_FRAC: f32 = 0.028;
pub const PYLON_SQUARE_TOP_FRAC: f32 = 0.03;
pub const PYLON_SQUARE_GAP_FRAC: f32 = 0.01;
pub const PYLON_TEAM_GAP_FRAC: f32 = 0.045;
pub const OWN_TEAM_BOX_PAD_FRAC: f32 = 0.008;
const OWN_TEAM_BOX_OUTLINE_FRAC: f32 = 0.003;
const OWN_TEAM_BOX_CORNER_FRAC: f32 = 0.005;
const OWN_TEAM_BOX_FILL_OPACITY: f32 = 0.18;
const TEAM_BOX_FILL_ALPHA: u8 = 70;
const TEAM_BOX_BORDER_PX: u32 = 1;
const MAX_CHANNEL: f32 = 255.0;
const BYTES_PER_RGBA_PIXEL: usize = 4;
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

/// `AmmoBar` is what the HUD's ammo bar shows: how full it is (0 to 1), in how many segments, and
/// whether it is drawn in the overheated colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmmoBar {
    pub frac: f64,
    pub segment_count: u32,
    pub is_overheated: bool,
}

impl AmmoBar {
    /// `FromWeapons` is the ammo bar of the gun in hand: the sniper's one block, full when loaded
    /// and empty while it reloads, or the rifle's shots left before it overheats, red once
    /// overheated.
    ///
    /// Example: `let ammo = AmmoBar::FromWeapons(&self.weapons, &self.settings.rifle);`
    #[allow(non_snake_case)]
    pub fn FromWeapons(weapons: &Weapons, rifle: &RifleSettings) -> AmmoBar {
        match weapons.gun {
            Gun::Laser => AmmoBar {
                frac: if weapons.sniper_reload_s <= 0.0 {
                    1.0
                } else {
                    0.0
                },
                segment_count: 1,
                is_overheated: false,
            },
            Gun::Rifle => AmmoBar {
                frac: 1.0 - weapons.rifle_heat_frac.min(1.0),
                segment_count: rifle.shots_until_overheat,
                is_overheated: weapons.is_overheated,
            },
        }
    }
}

/// `make_bar_rects` gives the HUD's bars as segmented bars in the HUD's look (`hud_bars.rs`): in
/// the bottom left this player's health (if in `bars`; one segment per rifle body hit), the gun in
/// hand's ammo (`AmmoBar::FromWeapons`; no pulses), below the middle the console being turned off
/// (if any), and the sprint energy (if in `bars`); health and energy carry `pulses`.
///
/// Example: `rects.extend(make_bar_rects(&bars, &self.weapons, &self.settings.rifle, &self.hud_pulses));`
pub fn make_bar_rects(
    bars: &HudBars,
    weapons: &Weapons,
    rifle: &RifleSettings,
    pulses: &HudPulses,
) -> Vec<OverlayRect> {
    make_ammo_bar_rects(bars, &AmmoBar::FromWeapons(weapons, rifle), rifle, pulses)
}

/// `make_ammo_bar_rects` is `make_bar_rects` with the ammo bar given (`ammo`), for a level whose
/// guns aren't a `Weapons`.
///
/// Example: `rects.extend(make_ammo_bar_rects(&bars, &ammo, &settings.rifle, &pulses));`
pub fn make_ammo_bar_rects(
    bars: &HudBars,
    ammo: &AmmoBar,
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
    rects.extend(hud_bars::make_segmented_bar(
        HEAT_BAR_MIN_FRAC,
        ammo.segment_count,
        ammo.frac,
        if ammo.is_overheated {
            HEAT_BAR_OVERHEATED_RGB
        } else {
            HEAT_BAR_RGB
        },
        NO_RECT_PULSE,
    ));
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

/// `get_respawn_filled_frac` gives how full the respawn bar is with `respawn_in_s` left of a wait
/// of `respawn_wait_s` (the server's, `RespawnSettings::get_wait_s`): empty when the wait starts,
/// full when it ends, and full for a wait of 0.
///
/// Example: `let frac: f64 = get_respawn_filled_frac(15.0, 20.0); // 0.25`
pub fn get_respawn_filled_frac(respawn_in_s: f64, respawn_wait_s: f64) -> f64 {
    if respawn_wait_s <= 0.0 {
        return 1.0;
    }
    (1.0 - respawn_in_s / respawn_wait_s).clamp(0.0, 1.0)
}

/// `make_respawn_bar_rects` gives the respawn bar under "YOU DIED" (`get_death_lines`), in the
/// HUD's look (`hud_bars::make_progress_bar`): it fills from empty to full over the wait, as
/// `get_respawn_filled_frac` says.
///
/// Example: `rects.extend(make_respawn_bar_rects(15.0, 20.0));`
pub fn make_respawn_bar_rects(respawn_in_s: f64, respawn_wait_s: f64) -> Vec<OverlayRect> {
    hud_bars::make_progress_bar(
        RESPAWN_BAR_MIN_FRAC,
        RESPAWN_BAR_SIZE_FRAC,
        get_respawn_filled_frac(respawn_in_s, respawn_wait_s) as f32,
        RESPAWN_BAR_RGB,
    )
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

/// `PylonSquare` is one console (pylon) in the HUD's row of squares: the team that defends it
/// (an index into `TEAM_RGB`) and whether it has been turned off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PylonSquare {
    pub team: u32,
    pub is_off: bool,
}

/// `make_pylon_square_rects` gives the row of squares at the top centre of a window of
/// `window_px` pixels, one per console in `pylons`: each team's consoles side by side in their
/// order, blue's group left of red's, a lit square in the team's colour while on and an empty
/// slot outlined in it once off (the bars' look, `hud_bars.rs`, unslanted). Around `own_team`'s
/// group (if any) goes a box in its colour, drawn first so the squares lie over it.
///
/// Example: `rects.extend(make_pylon_square_rects(&pylons, Some(0), (2560, 1440)));`
pub fn make_pylon_square_rects(
    pylons: &[PylonSquare],
    own_team: Option<u32>,
    window_px: (u32, u32),
) -> Vec<OverlayRect> {
    let team_count: u32 = TEAM_RGB.len() as u32;
    assert!(
        pylons.iter().all(|pylon| pylon.team < team_count)
            && own_team.is_none_or(|team| team < team_count),
        "make_pylon_square_rects: teams {pylons:?} and own team {own_team:?}, expected each below {team_count}"
    );
    let x_per_y: f32 = window_px.1 as f32 / window_px.0.max(1) as f32;
    let groups: Vec<(u32, Vec<bool>)> = (0..team_count)
        .map(|team| {
            let offs: Vec<bool> = pylons
                .iter()
                .filter(|pylon| pylon.team == team)
                .map(|pylon| pylon.is_off)
                .collect();
            (team, offs)
        })
        .filter(|(_, offs)| !offs.is_empty())
        .collect();
    let get_width = |count: usize| {
        count as f32 * PYLON_SQUARE_FRAC + (count - 1) as f32 * PYLON_SQUARE_GAP_FRAC
    };
    let total_width: f32 = groups
        .iter()
        .map(|(_, offs)| get_width(offs.len()))
        .sum::<f32>()
        + groups.len().saturating_sub(1) as f32 * PYLON_TEAM_GAP_FRAC;
    let mut left: f32 = -total_width / 2.0;
    let (top, bottom) = (
        PYLON_SQUARE_TOP_FRAC,
        PYLON_SQUARE_TOP_FRAC + PYLON_SQUARE_FRAC,
    );
    let mut boxes: Vec<OverlayRect> = Vec::new();
    let mut squares: Vec<OverlayRect> = Vec::new();
    for (team, offs) in &groups {
        let rgb: [f32; 3] = TEAM_RGB[*team as usize];
        if Some(*team) == own_team {
            let (pad, right) = (OWN_TEAM_BOX_PAD_FRAC, left + get_width(offs.len()));
            boxes.push(OverlayRect {
                min_frac: [0.5 + (left - pad) * x_per_y, top - pad],
                max_frac: [0.5 + (right + pad) * x_per_y, bottom + pad],
                color: [rgb[0], rgb[1], rgb[2], OWN_TEAM_BOX_FILL_OPACITY],
                style: RectStyle {
                    corner_frac: OWN_TEAM_BOX_CORNER_FRAC,
                    outline_frac: OWN_TEAM_BOX_OUTLINE_FRAC,
                    outline_color: [rgb[0], rgb[1], rgb[2], 1.0],
                    ..FLAT_RECT_STYLE
                },
                pulse: NO_RECT_PULSE,
            });
        }
        for is_off in offs {
            let mut square: OverlayRect = hud_bars::make_glass_rect(
                [0.5 + left * x_per_y, top],
                [0.5 + (left + PYLON_SQUARE_FRAC) * x_per_y, bottom],
                rgb,
                !is_off,
                NO_RECT_PULSE,
            );
            square.style.slant_x_per_y = 0.0;
            squares.push(square);
            left += PYLON_SQUARE_FRAC + PYLON_SQUARE_GAP_FRAC;
        }
        left += PYLON_TEAM_GAP_FRAC - PYLON_SQUARE_GAP_FRAC;
    }
    boxes.extend(squares);
    boxes
}

/// `make_team_box_rgba` gives an RGBA picture (rows top first) of `size_px`: a box in `team`'s
/// colour, a `TEAM_BOX_BORDER_PX` border with a faint fill of it, for a menu to draw under the
/// lines of this player's team.
///
/// Example: `let pixels = make_team_box_rgba((496, 50), 1);`
pub fn make_team_box_rgba(size_px: (u32, u32), team: u32) -> Vec<u8> {
    assert!(
        (team as usize) < TEAM_RGB.len()
            && size_px.0 > 2 * TEAM_BOX_BORDER_PX
            && size_px.1 > 2 * TEAM_BOX_BORDER_PX,
        "make_team_box_rgba: team {team} and size {size_px:?}, expected a team below {} and a size larger than two borders",
        TEAM_RGB.len()
    );
    let [r, g, b] = TEAM_RGB[team as usize].map(|c| (c * MAX_CHANNEL).round() as u8);
    let (width, height) = size_px;
    let mut pixels: Vec<u8> =
        Vec::with_capacity(width as usize * height as usize * BYTES_PER_RGBA_PIXEL);
    for y in 0..height {
        for x in 0..width {
            let is_border: bool = x < TEAM_BOX_BORDER_PX
                || y < TEAM_BOX_BORDER_PX
                || x >= width - TEAM_BOX_BORDER_PX
                || y >= height - TEAM_BOX_BORDER_PX;
            let alpha: u8 = if is_border {
                u8::MAX
            } else {
                TEAM_BOX_FILL_ALPHA
            };
            pixels.extend([r, g, b, alpha]);
        }
    }
    pixels
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
    //! | Respawn time left | a fraction of a second (1), between whole seconds (rounded up), three digits | `the_death_banner_counts_whole_seconds` |
    //! | Respawn bar's time left | the whole wait (empty), half (half full), none (full), below 0 or above the wait (clamped) | `the_respawn_bar_fills_over_the_wait` |
    //! | Respawn wait | above 0, 0 (full at once) | `the_respawn_bar_fills_over_the_wait` |
    //! | Server answered | not yet (connecting), yes (counts and keys) | `the_lobby_says_whether_connected` |
    //! | Consoles per team | each level's: 3 (Lunar Surface, Dust), 2 (Snow Planet, Dark Forest), 1 (City Sphere) | `each_levels_squares_make_a_centred_row` |
    //! | Teams with consoles | both, one | `each_levels_squares_make_a_centred_row`, `a_lone_teams_squares_are_centred` |
    //! | Window shape | 16:9, 21:9 (squares stay square) | `each_levels_squares_make_a_centred_row` |
    //! | Own team | none (lobby), blue, red | `the_box_goes_around_the_own_teams_squares` |
    //! | Console | on (lit), off (an empty slot) | `turned_off_consoles_look_different` |
    //! | Team box picture | border, inside | `the_team_box_picture_has_a_border` |

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
        assert_eq!(get_death_lines(119.5), ["YOU DIED", "Respawning in 120"]);
    }

    #[test]
    fn the_respawn_bar_fills_over_the_wait() {
        const WAIT_S: f64 = 20.0;
        assert_eq!(get_respawn_filled_frac(WAIT_S, WAIT_S), 0.0);
        assert_eq!(
            get_respawn_filled_frac(WAIT_S * HALF_FRAC, WAIT_S),
            HALF_FRAC
        );
        assert_eq!(get_respawn_filled_frac(0.0, WAIT_S), 1.0);
        assert_eq!(get_respawn_filled_frac(-1.0, WAIT_S), 1.0);
        assert_eq!(get_respawn_filled_frac(WAIT_S + 1.0, WAIT_S), 0.0);
        assert_eq!(get_respawn_filled_frac(0.0, 0.0), 1.0);
        let empty: Vec<OverlayRect> = make_respawn_bar_rects(WAIT_S, WAIT_S);
        assert_eq!(empty.len(), 1);
        let half: Vec<OverlayRect> = make_respawn_bar_rects(WAIT_S * HALF_FRAC, WAIT_S);
        assert_eq!(half.len(), 2);
        let slot_w_frac: f32 = empty[0].max_frac[0] - empty[0].min_frac[0];
        let filled_w_frac: f32 = half[1].max_frac[0] - half[1].min_frac[0];
        assert!((filled_w_frac - slot_w_frac * HALF_FRAC as f32).abs() < 1e-6);
        assert_eq!(half[1].min_frac, RESPAWN_BAR_MIN_FRAC);
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

    /// `WINDOWS_PX` are a 16:9 and a 21:9 window.
    const WINDOWS_PX: [(u32, u32); 2] = [(2560, 1440), (3440, 1440)];
    /// `SQUARE_TOLERANCE_PX` is how far a square's sides may differ, in pixels.
    const SQUARE_TOLERANCE_PX: f32 = 0.01;
    /// `CENTER_TOLERANCE_FRAC` is how far from the middle a row's centre may be.
    const CENTER_TOLERANCE_FRAC: f32 = 1e-5;

    /// `make_pylons` gives `per_team` consoles for each team, blue's first, with the ones in `offs`
    /// (indices into the list) turned off.
    fn make_pylons(per_team: usize, offs: &[usize]) -> Vec<PylonSquare> {
        (0..2 * per_team)
            .map(|i| PylonSquare {
                team: (i / per_team) as u32,
                is_off: offs.contains(&i),
            })
            .collect()
    }

    /// `get_size_px` gives `rect`'s width and height in pixels in a window of `window_px`.
    fn get_size_px(rect: &OverlayRect, window_px: (u32, u32)) -> [f32; 2] {
        [
            (rect.max_frac[0] - rect.min_frac[0]) * window_px.0 as f32,
            (rect.max_frac[1] - rect.min_frac[1]) * window_px.1 as f32,
        ]
    }

    #[test]
    fn each_levels_squares_make_a_centred_row() {
        for per_team in [3, 2, 1] {
            for window_px in WINDOWS_PX {
                let rects: Vec<OverlayRect> =
                    make_pylon_square_rects(&make_pylons(per_team, &[]), None, window_px);
                assert_eq!(rects.len(), 2 * per_team);
                for rect in &rects {
                    let [width_px, height_px] = get_size_px(rect, window_px);
                    assert!(
                        (width_px - height_px).abs() < SQUARE_TOLERANCE_PX,
                        "{rect:?}"
                    );
                    assert_eq!(rect.style.slant_x_per_y, 0.0);
                }
                let (blue, red) = rects.split_at(per_team);
                assert!(blue.iter().all(|rect| rect.color[..3] == TEAM_RGB[0]));
                assert!(red.iter().all(|rect| rect.color[..3] == TEAM_RGB[1]));
                let blue_right_frac: f32 = blue[per_team - 1].max_frac[0];
                assert!(blue_right_frac < 0.5 && red[0].min_frac[0] > 0.5);
                let center_frac: f32 =
                    (rects[0].min_frac[0] + rects[2 * per_team - 1].max_frac[0]) / 2.0;
                assert!(
                    (center_frac - 0.5).abs() < CENTER_TOLERANCE_FRAC,
                    "{center_frac}"
                );
                let team_gap_px: f32 = (red[0].min_frac[0] - blue_right_frac) * window_px.0 as f32;
                let expected_gap_px: f32 = PYLON_TEAM_GAP_FRAC * window_px.1 as f32;
                assert!((team_gap_px - expected_gap_px).abs() < SQUARE_TOLERANCE_PX);
            }
        }
    }

    #[test]
    fn a_lone_teams_squares_are_centred() {
        let red_only: Vec<PylonSquare> = make_pylons(2, &[])[2..].to_vec();
        let rects: Vec<OverlayRect> = make_pylon_square_rects(&red_only, None, WINDOWS_PX[0]);
        assert_eq!(rects.len(), 2);
        let center_frac: f32 = (rects[0].min_frac[0] + rects[1].max_frac[0]) / 2.0;
        assert!(
            (center_frac - 0.5).abs() < CENTER_TOLERANCE_FRAC,
            "{center_frac}"
        );
    }

    #[test]
    fn the_box_goes_around_the_own_teams_squares() {
        let pylons: Vec<PylonSquare> = make_pylons(3, &[]);
        let window_px: (u32, u32) = WINDOWS_PX[0];
        let unboxed: Vec<OverlayRect> = make_pylon_square_rects(&pylons, None, window_px);
        assert_eq!(unboxed.len(), 6);
        for own_team in [0, 1] {
            let rects: Vec<OverlayRect> =
                make_pylon_square_rects(&pylons, Some(own_team), window_px);
            assert_eq!(rects.len(), 7);
            let (team_box, squares) = (rects[0], &rects[1..]);
            assert_eq!(squares, &unboxed[..]);
            let rgb: [f32; 3] = TEAM_RGB[own_team as usize];
            assert_eq!(team_box.style.outline_color, [rgb[0], rgb[1], rgb[2], 1.0]);
            assert_eq!(team_box.color[..3], rgb);
            let is_inside = |rect: &OverlayRect| {
                (0..2).all(|a| {
                    rect.min_frac[a] > team_box.min_frac[a]
                        && rect.max_frac[a] < team_box.max_frac[a]
                })
            };
            let own: usize = own_team as usize * 3;
            assert!(squares[own..own + 3].iter().all(is_inside));
            let other: usize = (1 - own_team) as usize * 3;
            assert!(!squares[other..other + 3].iter().any(is_inside));
        }
    }

    #[test]
    fn turned_off_consoles_look_different() {
        let rects: Vec<OverlayRect> =
            make_pylon_square_rects(&make_pylons(2, &[1, 2]), None, WINDOWS_PX[0]);
        let (on, off) = (rects[0], rects[1]);
        assert_eq!(on.color[..3], TEAM_RGB[0]);
        assert_ne!(off.color, on.color);
        assert!(
            off.color[..3].iter().all(|&c| c < TEAM_RGB[0][2] / 2.0),
            "{off:?}"
        );
        assert_eq!(off.style.outline_color[..3], TEAM_RGB[0]);
        assert_eq!(rects[2].style.outline_color[..3], TEAM_RGB[1]);
        assert_eq!(rects[3].color[..3], TEAM_RGB[1]);
    }

    #[test]
    fn the_team_box_picture_has_a_border() {
        let size_px: (u32, u32) = (6, 4);
        let pixels: Vec<u8> = make_team_box_rgba(size_px, 1);
        assert_eq!(pixels.len(), 6 * 4 * BYTES_PER_RGBA_PIXEL);
        let [r, g, b] = TEAM_RGB[1].map(|c| (c * MAX_CHANNEL).round() as u8);
        let get_pixel = |x: usize, y: usize| {
            let at: usize = (y * size_px.0 as usize + x) * BYTES_PER_RGBA_PIXEL;
            [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
        };
        assert_eq!(get_pixel(0, 0), [r, g, b, u8::MAX]);
        assert_eq!(get_pixel(5, 3), [r, g, b, u8::MAX]);
        assert_eq!(get_pixel(2, 1), [r, g, b, TEAM_BOX_FILL_ALPHA]);
    }
}
