//! `hud` is what the `Session` draws over the frame: the banner, menus and lobby text, the HUD's
//! bars and crosshair, the minimap and map picture, and the wormhole tint.
//!
//! Interface: `Session`'s methods, `OverlayText`, `Picture`, `get_text_positions`, `make_text_mask`,
//! `get_blueshift_frac`, `get_crosshair_rects`.
//!
//! Example: `let text: Option<OverlayText> = session.get_overlay_text();`

use std::net::SocketAddr;

use glam::{DMat4, DVec4};
use sdl3::pixels::{Color, PixelFormat};

use crate::geometry::AXIS_W;
use crate::key_bindings::Action;

use super::{Gun, Multiplayer, Session, VoxelState, get_aim_zoom, get_points_and_teams};
use crate::{
    BANNER_HEIGHT_FRAC, BYTES_PER_PIXEL, DEATH_MASK_PX, DISABLE_BAR_MIN_FRAC, DISABLE_BAR_RGB,
    DISABLE_BAR_SIZE_FRAC, ENERGY_SEGMENT_COUNT, MENU_HEIGHT_FRAC, MENU_MASK_PX, Result,
    exit_prompt, hud_bars, menu, minimap, net, render_metal, render_voxel, voxel_world,
};

pub(super) const MAX_BLUESHIFT_FRAC: f64 = 1.25;
const HEAT_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.9];
const HEAT_BAR_RGB: [f32; 3] = [1.0, 0.6, 0.2];
const HEALTH_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.87];
const HEALTH_BAR_RGB: [f32; 3] = [0.35, 0.95, 0.45];
const HEALTH_PULSE_RGB: [f32; 3] = [1.0, 0.45, 0.2];
const CROSSHAIR_GAP_FRAC: f32 = 0.006;
const CROSSHAIR_ARM_FRAC: f32 = 0.012;
const CROSSHAIR_THICKNESS_FRAC: f32 = 0.0025;
const CROSSHAIR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.85];
const HEAT_BAR_OVERHEATED_RGB: [f32; 3] = [1.0, 0.15, 0.1];
const TEXT_TOP_PX: f32 = 2.0;
const TEXT_LINE_PX: f32 = 12.0;
pub(super) const MENU_LEFT_PX: f32 = 4.0;
pub(super) const DEBUG_GLYPH_PX: f32 = 8.0;
pub(super) const DEATH_DIM_FRAC: f32 = 0.55;
pub(super) const MENU_DIM_FRAC: f32 = 0.8;
const MAP_AREA_MIN_MASK_PX: [i32; 2] = [8, 28];
const MAP_AREA_MAX_EXCL_MASK_PX: [i32; 2] = [504, 306];
const PICTURE_PX_PER_MASK_PX: i32 = 2;
const DEATH_TITLE_MASK: u8 = 255;
const DEATH_COUNTDOWN_MASK: u8 = 128;
const ENERGY_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.93];
const ENERGY_BAR_RGB: [f32; 3] = [0.5, 0.82, 1.0];
const ENERGY_PULSE_RGB: [f32; 3] = [0.8, 0.94, 1.0];
const PERCENT_PER_FRAC: f64 = 100.0;

/// `get_blueshift_frac` gives how far the scene blueshifts (the overlay's `blueshift_frac`: 1 for
/// walking straight ahead at a mouth's throat, negative for a redshift, 0 for none): `strength_frac`
/// times how deep the eye is in a mouth (`VoxelWorld::get_mouth_depth_frac`) times the speed
/// along the view in walking speeds, held within ±`MAX_BLUESHIFT_FRAC`.
///
/// Example: `let blueshift_frac = get_blueshift_frac(0.5, -10.0, 10.0, 1.0);`
pub(crate) fn get_blueshift_frac(
    depth_frac: f64,
    view_m_per_s: f64,
    walk_m_per_s: f64,
    strength_frac: f64,
) -> f32 {
    (strength_frac * depth_frac * view_m_per_s / walk_m_per_s)
        .clamp(-MAX_BLUESHIFT_FRAC, MAX_BLUESHIFT_FRAC) as f32
}

/// `get_crosshair_rects` gives the crosshair in the middle of a window of `window_px` pixels: four
/// arms `CROSSHAIR_ARM_FRAC` long, `CROSSHAIR_GAP_FRAC` from the middle, in fractions of the window's
/// height so they look the same at any size.
///
/// Example: `let rects = get_crosshair_rects((2560, 1600));`
fn get_crosshair_rects(window_px: (u32, u32)) -> Vec<render_metal::OverlayRect> {
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
        .map(|([x0, y0], [x1, y1])| render_metal::OverlayRect {
            min_frac: [0.5 + x0, 0.5 + y0],
            max_frac: [0.5 + x1, 0.5 + y1],
            color: CROSSHAIR_COLOR,
            style: render_metal::FLAT_RECT_STYLE,
            pulse: render_metal::NO_RECT_PULSE,
        })
        .collect()
}

/// `OverlayText` is text shown over the dimmed view: its lines, each with whether it is
/// highlighted, the size of the mask it is drawn into (`make_text_mask`), how much of the window's
/// height that mask may take, whether its lines are centred (a banner) or start at the left (the
/// menu), how much the view behind it is darkened, and a picture under it.
pub(crate) struct OverlayText {
    pub(crate) lines: Vec<(String, bool)>,
    pub(crate) mask_px: (u32, u32),
    pub(crate) height_frac: f32,
    pub(crate) is_centered: bool,
    pub(crate) dim_frac: f32,
    pub(crate) picture: Option<Picture>,
}

/// `Picture` is an RGBA picture (rows top first) of `size_px` under overlay text, stretched
/// between two corners in the text mask's pixels.
pub(crate) struct Picture {
    pub(crate) pixels: Vec<u8>,
    pub(crate) size_px: (u32, u32),
    pub(crate) min_mask_px: [i32; 2],
    pub(crate) max_excl_mask_px: [i32; 2],
}

/// `get_map_picture_max_px` gives the largest map picture the map page holds, in pixels.
///
/// Example: `let layout = MapLayout::FromMap(&map, get_map_picture_max_px())?;`
pub(crate) fn get_map_picture_max_px() -> (u32, u32) {
    let [width, height] = [0, 1].map(|a| {
        ((MAP_AREA_MAX_EXCL_MASK_PX[a] - MAP_AREA_MIN_MASK_PX[a]) * PICTURE_PX_PER_MASK_PX) as u32
    });
    (width, height)
}

/// `get_lobby_lines` gives the lobby banner's two lines: until the server at `server` first
/// answers, that this window is connecting to it; once it has (`lobby_counts`, each team's head
/// count), that it is connected, the counts and `join_keys_line`.
///
/// Example: `let lines = get_lobby_lines(Some([1, 2]), server, "1 joins blue, 2 joins red".to_string());`
pub(crate) fn get_lobby_lines(
    lobby_counts: Option<[u32; net::TEAM_NAMES.len()]>,
    server: SocketAddr,
    join_keys_line: String,
) -> [String; 2] {
    match lobby_counts {
        Some([blue, red]) => [format!("CONNECTED  BLUE {blue}  RED {red}"), join_keys_line],
        None => ["CONNECTING".to_string(), format!("server {server}")],
    }
}

/// `get_text_positions` places each of `text`'s lines in its mask: the line cut to the mask's
/// width, and where its top left goes.
///
/// Example: `for (line, (x, y), is_highlighted) in get_text_positions(&text) { ... }`
pub(crate) fn get_text_positions(text: &OverlayText) -> Vec<(String, (f32, f32), bool)> {
    let (width, height) = text.mask_px;
    let bottom_px: f32 =
        TEXT_TOP_PX + TEXT_LINE_PX * text.lines.len().saturating_sub(1) as f32 + DEBUG_GLYPH_PX;
    assert!(
        bottom_px <= height as f32,
        "get_text_positions: {} lines don't fit a mask {height} px high",
        text.lines.len()
    );
    let max_char_count: usize = (width as f32 / DEBUG_GLYPH_PX) as usize;
    text.lines
        .iter()
        .enumerate()
        .map(|(i, (line, is_highlighted))| {
            let shown: String = line.chars().take(max_char_count).collect();
            let x: f32 = if text.is_centered {
                (width as f32 - DEBUG_GLYPH_PX * shown.chars().count() as f32) / 2.0
            } else {
                MENU_LEFT_PX
            };
            (
                shown,
                (x, TEXT_TOP_PX + TEXT_LINE_PX * i as f32),
                *is_highlighted,
            )
        })
        .collect()
}

/// `make_text_mask` draws `text` with SDL's built-in text into a mask of `text.mask_px`, one byte
/// a pixel: `DEATH_TITLE_MASK` on highlighted lines' text, `DEATH_COUNTDOWN_MASK` on the others',
/// 0 elsewhere, for the Metal overlay.
///
/// Example: `let mask = make_text_mask(&text)?;`
pub(crate) fn make_text_mask(text: &OverlayText) -> Result<Vec<u8>> {
    let (width, height) = text.mask_px;
    let surface = sdl3::surface::Surface::new(width, height, PixelFormat::RGBA32)?;
    let mut canvas = surface.into_canvas()?;
    for (line, position, is_highlighted) in get_text_positions(text) {
        canvas.set_draw_color(if is_highlighted {
            Color::RGB(255, 0, 0)
        } else {
            Color::RGB(0, 255, 0)
        });
        canvas.draw_debug_text(line.as_str(), position)?;
    }
    canvas.present();
    let surface = canvas.into_surface();
    let pitch: usize = surface.pitch() as usize;
    Ok(surface.with_lock(|pixels: &[u8]| {
        (0..height as usize)
            .flat_map(|y| (0..width as usize).map(move |x| y * pitch + x * BYTES_PER_PIXEL))
            .map(|offset| match (pixels[offset], pixels[offset + 1]) {
                (0, 0) => 0,
                (0, _) => DEATH_COUNTDOWN_MASK,
                _ => DEATH_TITLE_MASK,
            })
            .collect()
    }))
}

impl Session {
    /// `get_banner_lines` gives the two lines shown over the view: while the exit prompt shows,
    /// its question and keys; when this window can't play on its server
    /// (`get_connection_problem`), what is wrong; in the lobby, whether it is connected to the
    /// server yet (`get_lobby_lines`); after asking to join, until welcomed, that
    /// it is joining and the server's address; once a team has won, the winner and how to start a
    /// new game; while this player is out after a hit, "YOU DIED" and the countdown to respawning;
    /// or `None`.
    ///
    /// Example: `if let Some(lines) = session.get_banner_lines() { ... }`
    fn get_banner_lines(&self) -> Option<[String; 2]> {
        if self.exit_prompt.is_shown() {
            return Some(exit_prompt::get_lines());
        }
        let multiplayer: &Multiplayer = self.multiplayer.as_ref()?;
        let client: &net::Client = &multiplayer.client;
        if let Some(problem) = self.get_connection_problem() {
            return Some(problem.get_lines(client.get_server()));
        }
        if multiplayer.client.is_in_lobby() {
            let keys = &self.player_settings.keys;
            return Some(get_lobby_lines(
                client.lobby_counts,
                client.get_server(),
                format!(
                    "{} joins blue, {} joins red",
                    keys.get_label(Action::JoinBlue),
                    keys.get_label(Action::JoinRed)
                ),
            ));
        }
        if client.is_waiting_for_welcome() && !multiplayer.is_eliminated {
            return Some([
                "JOINING THE GAME".to_string(),
                format!("server {}", client.get_server()),
            ]);
        }
        if let Some(winner) = multiplayer.client.game.winner {
            return Some([
                format!(
                    "{} TEAM WINS",
                    net::TEAM_NAMES[winner as usize].to_uppercase()
                ),
                "New game: Enter on the server".to_string(),
            ]);
        }
        if !multiplayer.is_eliminated {
            return None;
        }
        Some([
            "YOU DIED".to_string(),
            format!(
                "Respawning in {:.0}",
                multiplayer.respawn_in_s.ceil().max(1.0)
            ),
        ])
    }

    /// `get_zoom` gives how much closer the world looks now: the gun in hand's full zoom
    /// (`aim` in the server settings), eased in by how far this player has aimed (`get_aim_zoom`).
    ///
    /// Example: `let zoom = self.get_zoom();`
    pub(crate) fn get_zoom(&self) -> f64 {
        let full_zoom: f64 = match self.gun {
            Gun::Laser => self.settings.aim.sniper_zoom,
            Gun::Rifle => self.settings.aim.rifle_zoom,
        };
        get_aim_zoom(self.aim_frac, full_zoom)
    }

    /// `get_lit_beams` gives the lit part of each laser shot now (`Tracer::get_lit_m`), with the
    /// shooter's team and gun and the shot's speckle share, on a map of curvature radius `r`. While the trail of one of this player's own shots still reaches the
    /// muzzle, its beam starts at the gun's tip as drawn now (`get_muzzle`), so it stays on the tip
    /// as the gun moves.
    ///
    /// Example: `let beams = self.get_lit_beams(voxel.world.r);`
    pub(crate) fn get_lit_beams(&self, r: f64) -> Vec<render_voxel::LaserBeam> {
        let now_s: f64 = self.start.elapsed().as_secs_f64();
        let muzzle: Option<glam::DVec4> = self.get_muzzle(r);
        self.tracers
            .iter()
            .map(|tracer| {
                let (tail_m, head_m) = tracer.get_lit_m(now_s);
                let mut lit: Vec<voxel_world::BeamSegment> =
                    voxel_world::clip_beam(&tracer.segments, r, tail_m, head_m);
                if tracer.is_own
                    && tail_m <= 0.0
                    && let (Some(muzzle), Some(first)) = (muzzle, lit.first_mut())
                    && let Some(joined) = voxel_world::BeamSegment::Between(
                        muzzle,
                        voxel_world::geodesic_point(first.a, first.b, first.length),
                    )
                {
                    *first = joined;
                }
                render_voxel::LaserBeam {
                    segments: lit,
                    team: tracer.team,
                    is_rifle: tracer.is_rifle,
                    speckle_frac: tracer.speckle_frac,
                }
            })
            .collect()
    }

    /// `get_muzzle` gives where the tip of the gun in hand shows in the world, on a map of
    /// curvature radius `r`: the wand's tip in camera space, moved
    /// toward the view's centre by the aim zoom (which magnifies the scene but not the gun), or
    /// `None` with no gun in view.
    ///
    /// Example: `let muzzle = self.get_muzzle(voxel.world.r);`
    fn get_muzzle(&self, r: f64) -> Option<glam::DVec4> {
        let tip_m: glam::DVec3 = self.get_wand()?.get_tip_m().as_dvec3();
        let zoom: f64 = self.get_zoom();
        Some(
            self.get_eye_frame()
                * voxel_world::map_exp(
                    glam::DVec3::new(tip_m.x / zoom, tip_m.y / zoom, tip_m.z),
                    r,
                ),
        )
    }

    /// `make_overlay_minimap` gives the minimap for a window of `window_px` (`minimap.rs`): around
    /// this player, with the other players and the shots, or `None` when the player settings hide
    /// it or text covers the view.
    ///
    /// Example: `let minimap = session.make_overlay_minimap((2560, 1600));`
    pub(crate) fn make_overlay_minimap(
        &self,
        window_px: (u32, u32),
    ) -> Option<render_metal::OverlayMinimap<'_>> {
        if !self.player_settings.minimap.is_shown || self.is_view_covered() {
            return None;
        }
        let (feet, beams): (DMat4, Vec<(Vec<voxel_world::BeamSegment>, u32)>) = (
            self.voxel.player.get_feet_frame(&self.voxel.world),
            self.get_lit_beams(self.voxel.world.r)
                .into_iter()
                .map(|beam| (beam.segments, beam.team))
                .collect(),
        );
        let players: Vec<(DVec4, u32)> = match &self.multiplayer {
            Some(multiplayer) => multiplayer
                .client
                .players
                .iter()
                .map(|player| (player.frame.col(AXIS_W), player.team))
                .collect(),
            None => Vec::new(),
        };
        let size_px: (u32, u32) = self.map_layout.get_size_px();
        Some(render_metal::OverlayMinimap {
            map_pixels: self.map_layout.get_plain_pixels(),
            map_size_px: (size_px.0 as usize, size_px.1 as usize),
            minimap: minimap::make_minimap(
                &self.map_layout,
                &self.player_settings.minimap,
                feet,
                &players,
                &beams,
                window_px,
            ),
        })
    }

    /// `get_edge_stretch_frac` gives how far this player's view stretches at the map's soft edge:
    /// the square of how far into it they stand (`VoxelWorld::get_edge_frac`), so it starts gently
    /// and builds near the stop, times their `view.edge_stretch_frac`; 0 off a voxel map.
    ///
    /// Example: `let stretch_frac = session.get_edge_stretch_frac();`
    pub(crate) fn get_edge_stretch_frac(&self) -> f32 {
        let feet: DVec4 = self
            .voxel
            .player
            .get_feet_frame(&self.voxel.world)
            .col(AXIS_W);
        (self.voxel.world.get_edge_frac(feet).powi(2) * self.player_settings.view.edge_stretch_frac)
            as f32
    }

    /// `is_view_covered` says whether text covers the view: the menu or a banner.
    ///
    /// Example: `if !self.is_view_covered() { ... }`
    pub(crate) fn is_view_covered(&self) -> bool {
        self.menu.is_some() || self.get_banner_lines().is_some()
    }

    /// `get_overlay_text` gives the text shown over the dimmed view: the menu while it is open,
    /// else the banner lines (`get_banner_lines`), or `None`.
    ///
    /// Example: `if let Some(text) = session.get_overlay_text() { ... }`
    pub(crate) fn get_overlay_text(&self) -> Option<OverlayText> {
        if let Some(menu) = &self.menu {
            let save_path: String = match &self.player_settings_path {
                Some(path) => path.display().to_string(),
                None => "nowhere: HOME isn't set, pass --player-settings".to_string(),
            };
            let scoreboard: menu::Scoreboard = match &self.multiplayer {
                None => menu::Scoreboard::NoServer,
                Some(Multiplayer { client, .. }) => match &client.scores {
                    None => menu::Scoreboard::Waiting,
                    Some(scores) => menu::Scoreboard::Scores {
                        scores,
                        own_id: client.welcomed_id,
                    },
                },
            };
            return Some(OverlayText {
                lines: menu.get_lines(&self.player_settings, &save_path, scoreboard),
                mask_px: MENU_MASK_PX,
                height_frac: MENU_HEIGHT_FRAC,
                is_centered: false,
                dim_frac: MENU_DIM_FRAC,
                picture: (menu.get_page() == menu::Page::Map).then(|| self.make_map_picture()),
            });
        }
        let [title, detail] = self.get_banner_lines()?;
        Some(OverlayText {
            lines: vec![(title, true), (detail, false)],
            mask_px: DEATH_MASK_PX,
            height_frac: BANNER_HEIGHT_FRAC,
            is_centered: true,
            dim_frac: DEATH_DIM_FRAC,
            picture: None,
        })
    }

    /// `make_map_picture` gives the map page's picture: the whole map from above
    /// (`map_layout.rs`) with this player on it and, in a game on a server, every other player in
    /// the game, centred in the page's area for it.
    ///
    /// Example: `let picture = self.make_map_picture();`
    fn make_map_picture(&self) -> Picture {
        let feet: glam::DMat4 = self.voxel.player.get_feet_frame(&self.voxel.world);
        let others: Vec<(DVec4, u32)> = match &self.multiplayer {
            Some(multiplayer) => get_points_and_teams(&multiplayer.client.players),
            None => Vec::new(),
        };
        let size_px: (u32, u32) = self.map_layout.get_size_px();
        let size_mask_px: [i32; 2] =
            [size_px.0 as i32, size_px.1 as i32].map(|n| n / PICTURE_PX_PER_MASK_PX);
        let min_mask_px: [i32; 2] = [0, 1].map(|a| {
            MAP_AREA_MIN_MASK_PX[a]
                + (MAP_AREA_MAX_EXCL_MASK_PX[a] - MAP_AREA_MIN_MASK_PX[a] - size_mask_px[a]) / 2
        });
        Picture {
            pixels: self.map_layout.draw_players(
                feet,
                self.get_team(),
                self.player_settings.minimap.range_m,
                &others,
            ),
            size_px,
            min_mask_px,
            max_excl_mask_px: [0, 1].map(|a| min_mask_px[a] + size_mask_px[a]),
        }
    }

    /// `get_overlay_rects` gives the HUD's rectangles drawn over the view: the crosshair (unless
    /// text covers the view) and, in the bottom left, as segmented bars in the HUD's look
    /// (`hud_bars.rs`): this player's health in a game on a server (one segment per rifle body
    /// hit), the gun in hand's ammo (the rifle's shots left before it overheats, or the sniper's
    /// one block, empty while it reloads; no pulses), and on a voxel map the sprint energy. Health
    /// and energy carry pulses that run over their filled part while they change
    /// (`update_bar_pulses`). Below the middle, the progress of turning off a console (while
    /// holding `E` on one).
    ///
    /// Example: `let rects = session.get_overlay_rects();`
    pub(crate) fn get_overlay_rects(&self) -> Vec<render_metal::OverlayRect> {
        let mut rects: Vec<render_metal::OverlayRect> = Vec::new();
        if !self.is_view_covered() {
            rects.extend(get_crosshair_rects(self.last_output_px));
        }
        if let Some(multiplayer) = &self.multiplayer {
            let segment_count: u32 = u32::from(self.settings.rifle.body_hits_to_eliminate);
            let health_frac: f64 = multiplayer.client.health_frac;
            rects.extend(hud_bars::make_segmented_bar(
                HEALTH_BAR_MIN_FRAC,
                segment_count,
                health_frac,
                HEALTH_BAR_RGB,
                self.health_pulses.make_rect_pulse(
                    HEALTH_BAR_MIN_FRAC,
                    segment_count,
                    health_frac,
                    HEALTH_PULSE_RGB,
                    HEALTH_BAR_RGB,
                    &hud_bars::PULSE_TUNING,
                ),
            ));
        }
        if self.gun == Gun::Laser {
            let is_loaded: bool = self.sniper_reload_s <= 0.0;
            rects.extend(hud_bars::make_segmented_bar(
                HEAT_BAR_MIN_FRAC,
                1,
                if is_loaded { 1.0 } else { 0.0 },
                HEAT_BAR_RGB,
                render_metal::NO_RECT_PULSE,
            ));
        }
        if self.gun == Gun::Rifle {
            rects.extend(hud_bars::make_segmented_bar(
                HEAT_BAR_MIN_FRAC,
                self.settings.rifle.shots_until_overheat,
                1.0 - self.rifle_heat_frac.min(1.0),
                if self.is_overheated {
                    HEAT_BAR_OVERHEATED_RGB
                } else {
                    HEAT_BAR_RGB
                },
                render_metal::NO_RECT_PULSE,
            ));
        }
        if let Some((_, held_s)) = self.multiplayer.as_ref().and_then(|m| m.disabling) {
            let fill_frac: f32 =
                (held_s / self.settings.consoles.hold_to_disable_s).min(1.0) as f32;
            rects.extend(hud_bars::make_progress_bar(
                DISABLE_BAR_MIN_FRAC,
                DISABLE_BAR_SIZE_FRAC,
                fill_frac,
                DISABLE_BAR_RGB,
            ));
        }
        let voxel: &VoxelState = &self.voxel;
        rects.extend(hud_bars::make_segmented_bar(
            ENERGY_BAR_MIN_FRAC,
            ENERGY_SEGMENT_COUNT,
            voxel.player.energy_frac,
            ENERGY_BAR_RGB,
            self.energy_pulses.make_rect_pulse(
                ENERGY_BAR_MIN_FRAC,
                ENERGY_SEGMENT_COUNT,
                voxel.player.energy_frac,
                ENERGY_PULSE_RGB,
                ENERGY_BAR_RGB,
                &hud_bars::PULSE_TUNING,
            ),
        ));
        rects
    }

    /// `get_hud_lines` gives the HUD's lines: speed, movement, laser, the last message and the
    /// controls.
    ///
    /// Example: `let lines = session.get_hud_lines("metal", (1280, 800), (2560, 1600));`
    pub(crate) fn get_hud_lines(
        &self,
        renderer_name: &str,
        render_px: (u32, u32),
        output_px: (u32, u32),
    ) -> Vec<String> {
        let scaling: String = if render_px == output_px {
            String::new()
        } else {
            format!(" upscaled to {}x{}", output_px.0, output_px.1)
        };
        let energy: String = format!(
            ", energy {:.0}%",
            self.voxel.player.energy_frac * PERCENT_PER_FRAC
        );
        let gun: String = match (self.gun, self.is_overheated) {
            (Gun::Laser, _) => ", gun 1 (laser)".to_string(),
            (Gun::Rifle, true) => ", gun 2 (rifle) overheated".to_string(),
            (Gun::Rifle, false) => format!(
                ", gun 2 (rifle) heat {:.0}%",
                self.rifle_heat_frac * PERCENT_PER_FRAC
            ),
        };
        let (is_flying, is_on_ground, height_m) = (
            self.voxel.player.is_flying,
            self.voxel.player.is_on_ground,
            self.voxel.player.h_m - self.voxel.world.player.eye_height_m,
        );
        let movement: &str = if is_flying {
            "flying"
        } else if is_on_ground {
            "walking"
        } else {
            "in the air"
        };
        let mut lines: Vec<String> = vec![
            format!(
                "{:.0} fps, {:.1} ms render{}, {renderer_name} at {}x{}{scaling}",
                self.fps,
                self.render_ms,
                self.gpu_ms
                    .map_or(String::new(), |ms| format!(" ({ms:.1} ms on the GPU)")),
                render_px.0,
                render_px.1
            ),
            format!("{movement}, height {:.1} blocks{energy}{gun}", height_m),
        ];
        if let Some(laser) = &self.laser {
            lines.push(format!("laser {laser}"));
        }
        if self.voxel.world.can_grow_black_hole() {
            let voxel: &VoxelState = &self.voxel;
            let strength_pct: f64 = voxel.black_hole_growth.strength_frac * PERCENT_PER_FRAC;
            let rate_pct: f64 = self.get_time_rate_frac() * PERCENT_PER_FRAC;
            lines.push(format!(
                "BLACK HOLE {strength_pct:.0}%, your time runs at {rate_pct:.0}%"
            ));
        }
        if let Some(multiplayer) = &self.multiplayer {
            lines.push(match multiplayer.client.id {
                Some(id) => format!("player {id}, {} others", multiplayer.client.players.len()),
                None if multiplayer.is_eliminated => format!(
                    "You died; respawning in {:.0} s",
                    multiplayer.respawn_in_s.ceil()
                ),
                None => "joining".to_string(),
            });
        }
        if self.message_time_s > 0.0 {
            lines.push(self.message.clone());
        }
        lines.push(if self.is_captured {
            self.player_settings.keys.describe()
        } else {
            "Click to capture the mouse; Esc to exit".to_string()
        });
        lines
    }
}
