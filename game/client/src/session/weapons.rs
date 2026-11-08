//! `weapons` is the `Session`'s guns and thrown wormholes: throwing and destroying wormholes, the
//! laser and the rifle, disabling consoles, how much a hit weighs, and the gun model in hand.
//!
//! Interface: `Session`'s methods, `take_hit_weight`, `make_wand`, `get_sway`, `get_aim_zoom`.
//!
//! Example: `let wand: Option<render_metal::Wand> = session.get_wand();`

use glam::{DMat4, EulerRot, Mat3, Vec3};

use crate::geometry::AXIS_W;
use crate::player::Input;
use crate::settings::ServerSettings;
use crate::thrown_wormholes::{HIT_FLASH_COLOR, Marker, MouthPlace, PlayerWormholes, fly_throw};
use crate::voxel_world::VoxelWorld;

use super::{Gun, Multiplayer, Session, VoxelState};
use crate::{
    multiplayer, net, render_metal, settings, shot_noise, third_person, viewmodel, voxel_world,
};

const LONE_MOUTH_HITS_TO_DESTROY: u8 = 1;
const WAND_RECOIL_M: f32 = 0.035;
const WAND_RECOIL_TILT_RAD: f32 = 0.12;
const WAND_LASER_SHAFT_HALF_M: [f32; 4] = [0.011, 0.011, 0.16, 0.0];
const WAND_LASER_SHAFT_COLOR: [f32; 4] = [0.2, 0.17, 0.22, 1.0];
const WAND_LASER_TIP_HALF_M: [f32; 4] = [0.016, 0.016, 0.028, 0.0];
const WAND_RIFLE_SHAFT_HALF_M: [f32; 4] = [0.024, 0.032, 0.17, 0.0];
const WAND_RIFLE_SHAFT_COLOR: [f32; 4] = [0.42, 0.45, 0.5, 1.0];
const WAND_RIFLE_TIP_HALF_M: [f32; 4] = [0.007, 0.007, 0.07, 0.0];
const GUN_POSE_PER_S: f64 = 6.0;
const WAND_JOG_CENTER_M: [f32; 3] = [0.22, -0.22, -0.34];
const WAND_AIM_YAW_RAD: f32 = 0.076;
const WAND_JOG_YAW_RAD: f32 = 0.35;
const WAND_AIM_PITCH_RAD: f32 = 0.064;
const WAND_JOG_PITCH_RAD: f32 = -0.35;
const WAND_JOG_ROLL_RAD: f32 = 0.5;
const WAND_JOG_SWING_RAD: f32 = 0.18;
const WAND_WALK_BOB_M: f32 = 0.006;
const WAND_JOG_BOB_M: f32 = 0.03;
const WAND_BLUE_TIP_COLOR: [f32; 4] = [0.35, 0.68, 1.0, 1.0];
const WAND_RED_TIP_COLOR: [f32; 4] = [1.0, 0.28, 0.14, 1.0];

/// `take_hit_weight` gives how many hits like this one `gun` needs to eliminate someone by
/// `settings` (a rifle hit to the head counts more), or `None` when this rifle shot already hit
/// (`has_rifle_shot_hit`, which it then sets): the laser reports every frame it hits, the rifle
/// once per shot.
///
/// Example: `if let Some(hits_to_kill) = take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false) { ... }`
pub(crate) fn take_hit_weight(
    settings: &ServerSettings,
    gun: Gun,
    has_rifle_shot_hit: &mut bool,
    is_head_hit: bool,
) -> Option<u8> {
    match (gun, is_head_hit) {
        (Gun::Laser, _) => Some(settings.sniper.hits_to_eliminate),
        (Gun::Rifle, _) if *has_rifle_shot_hit => None,
        (Gun::Rifle, is_head) => {
            *has_rifle_shot_hit = true;
            Some(if is_head {
                settings.rifle.head_hits_to_eliminate
            } else {
                settings.rifle.body_hits_to_eliminate
            })
        }
    }
}

/// `make_wand` places the wand in camera space: held on the right, aimed toward the crosshair with
/// the front of its tip at the muzzle (`voxel_world::MUZZLE_M`, where the laser starts), dipping a little at each step (`phase_rad`), and, as `run_pose_frac` goes to 1
/// (eased), lowered, tipped down and rolled, bouncing and swinging with each stride; its tip has
/// `team`'s colour. The laser (`gun`) is a thin dark wand with a short glowing tip; the rifle is a
/// thick grey block with a long, thin glowing barrel. Recoil (`recoil_frac`, 1 just after a shot)
/// pushes it back toward the eye and tips it up, and `sway` (`get_sway`: metres right, radians of
/// muzzle turned left) slides and swings it with the steps.
///
/// Example: `let wand = make_wand(0.0, 1.2, 0.0, 1, Gun::Laser, (0.0, 0.0));`
fn make_wand(
    run_pose_frac: f64,
    phase_rad: f64,
    recoil_frac: f64,
    team: u32,
    gun: Gun,
    sway: (f32, f32),
) -> render_metal::OverlayWand {
    let (sway_m, swing_rad) = sway;
    let jog: f32 = ease_frac(run_pose_frac) as f32;
    let (step, swing) = (phase_rad.sin().abs() as f32, phase_rad.sin() as f32);
    let lerp = |aim: f32, run: f32| aim + (run - aim) * jog;
    let bob_m: f32 = lerp(WAND_WALK_BOB_M, WAND_JOG_BOB_M) * step;
    let (shaft_half_m, tip_half_m) = match gun {
        Gun::Laser => (WAND_LASER_SHAFT_HALF_M, WAND_LASER_TIP_HALF_M),
        Gun::Rifle => (WAND_RIFLE_SHAFT_HALF_M, WAND_RIFLE_TIP_HALF_M),
    };
    let aim_rotation: Mat3 =
        Mat3::from_euler(EulerRot::YXZ, WAND_AIM_YAW_RAD, WAND_AIM_PITCH_RAD, 0.0);
    let aim_center: Vec3 = voxel_world::MUZZLE_M.as_vec3()
        + aim_rotation * (Vec3::Z * (shaft_half_m[2] + 2.0 * tip_half_m[2]));
    let kick: f32 = recoil_frac as f32;
    let center: Vec3 = aim_center.lerp(Vec3::from(WAND_JOG_CENTER_M), jog) - Vec3::Y * bob_m
        + Vec3::Z * WAND_RECOIL_M * kick
        + Vec3::X * sway_m;
    let rotation: Mat3 = Mat3::from_euler(
        EulerRot::YXZ,
        lerp(WAND_AIM_YAW_RAD, WAND_JOG_YAW_RAD) + swing_rad,
        lerp(WAND_AIM_PITCH_RAD, WAND_JOG_PITCH_RAD) + WAND_RECOIL_TILT_RAD * kick,
        (WAND_JOG_ROLL_RAD + WAND_JOG_SWING_RAD * swing) * jog,
    );
    render_metal::OverlayWand {
        center_m: center.extend(0.0).to_array(),
        axes: [rotation.x_axis, rotation.y_axis, rotation.z_axis]
            .map(|axis| axis.extend(0.0).to_array()),
        shaft_half_m,
        shaft_color: match gun {
            Gun::Laser => WAND_LASER_SHAFT_COLOR,
            Gun::Rifle => WAND_RIFLE_SHAFT_COLOR,
        },
        tip_half_m,
        tip_color: if team == 1 {
            WAND_RED_TIP_COLOR
        } else {
            WAND_BLUE_TIP_COLOR
        },
    }
}

/// `ease_frac` eases a fraction from 0 to 1 (smoothstep): slow at both ends.
///
/// Example: `let jog: f64 = ease_frac(0.5);`
fn ease_frac(frac: f64) -> f64 {
    frac * frac * (3.0 - 2.0 * frac)
}

/// `get_sway` gives how the gun in hand sways now with the steps (`phase_rad`, the step phase: a
/// step each half turn), as (metres slid right, radians its muzzle turns left): the hands slide it
/// left on one step and right on the next by `head_bob`'s walking slide, and a longer, heavier gun
/// lags behind them, its muzzle swinging the other way by `head_bob`'s swing for a gun as long as
/// the bolt-action rifle, times `model`'s length over the rifle's. Both grow to their sprinting
/// sizes as the run pose (`run_pose_frac`, eased as `make_wand` eases it) goes to 1, and fade in
/// and out with `step_frac` (0 standing still, 1 stepping).
///
/// Example: `let sway = get_sway(GunModel::Revolver, 1.0, 0.0, 0.5, &settings.view.head_bob);`
pub(crate) fn get_sway(
    model: viewmodel::GunModel,
    step_frac: f64,
    run_pose_frac: f64,
    phase_rad: f64,
    head_bob: &settings::HeadBobSettings,
) -> (f32, f32) {
    let jog: f64 = ease_frac(run_pose_frac);
    let lerp = |walk: f64, sprint: f64| walk + (sprint - walk) * jog;
    let length_frac: f64 =
        f64::from(model.get_modelled_length_m() / viewmodel::BOLT_ACTION_RIFLE_MODELLED_LENGTH_M);
    let side: f64 = step_frac * phase_rad.sin();
    let sway_m: f64 = lerp(head_bob.gun_walk_sway_m, head_bob.gun_sprint_sway_m) * side;
    let swing_rad: f64 = lerp(
        head_bob.rifle_walk_swing_rad,
        head_bob.rifle_sprint_swing_rad,
    ) * length_frac
        * side;
    (sway_m as f32, swing_rad as f32)
}

/// `get_aim_zoom` gives how much closer the world looks at aim fraction `aim_frac` (0 not aiming,
/// 1 fully aimed), eased in and out, toward `full_zoom`.
///
/// Example: `let zoom = get_aim_zoom(0.5, 2.5);`
pub(crate) fn get_aim_zoom(aim_frac: f64, full_zoom: f64) -> f64 {
    let t: f64 = aim_frac.clamp(0.0, 1.0);
    1.0 + (full_zoom - 1.0) * t * t * (3.0 - 2.0 * t)
}

impl Session {
    /// `can_throw` says whether this player may throw a wormhole mouth now: playing, with the
    /// mouse captured, without a throw in the air, and without both mouths down.
    ///
    /// Example: `if self.can_throw() { self.throw_wormhole(); }`
    pub(crate) fn can_throw(&self) -> bool {
        self.is_captured
            && self.route.is_empty()
            && !self.is_in_lobby()
            && !self.multiplayer.as_ref().is_some_and(|m| m.is_eliminated)
            && self.thrown.can_throw()
    }

    /// `throw_wormhole` throws this player's next wormhole mouth from their eye.
    ///
    /// Example: `self.throw_wormhole();`
    pub(crate) fn throw_wormhole(&mut self) {
        let (camera, color) = (self.get_eye_frame(), self.thrown.get_next_color());
        let settings = &self.settings.wormholes;
        let flight = fly_throw(camera, color, settings, self.voxel.world.r, |x, v, s| {
            self.voxel.world.find_hit_s(x, v, s)
        });
        self.thrown
            .throw(flight, self.start.elapsed().as_secs_f64());
    }

    /// `update_wormholes` lands this player's throw once it comes down (where the map allows, clear
    /// of the other mouths down, other players' too), flashes and destroys wormholes by the
    /// server's damage (`update_wormhole_damage`), and puts every linked pair, this player's first,
    /// into the world when they change, flashing red where they were just hit.
    ///
    /// Example: `self.update_wormholes();`
    pub(crate) fn update_wormholes(&mut self) {
        let now_s: f64 = self.start.elapsed().as_secs_f64();
        let mut others: Vec<MouthPlace> = self.thrown.get_kept_places();
        for (_, wormholes) in &self.get_all_wormholes()[1..] {
            others.extend(wormholes.get_places());
        }
        let (eye_m, r) = (self.voxel.world.player.eye_height_m, self.voxel.world.r);
        let world: &VoxelWorld = &self.voxel.world;
        let message: Option<String> = self
            .thrown
            .update(now_s, eye_m, r, |p| world.can_place_mouth(p, &others));
        if let Some(message) = message {
            self.show_message(message);
        }
        self.update_wormhole_damage(now_s);
        let (owners, pairs): (Vec<Option<u32>>, Vec<[MouthPlace; 2]>) = self
            .get_all_wormholes()
            .into_iter()
            .filter_map(|(owner, wormholes)| Some((owner, wormholes.get_pair()?)))
            .unzip();
        let flashing: Vec<bool> = owners.iter().map(|&o| self.is_flashing(o, now_s)).collect();
        if pairs != self.thrown_pairs {
            self.voxel.world.set_thrown_wormholes(&pairs);
            (self.thrown_pairs, self.thrown_pair_owners) = (pairs, owners);
        }
        self.voxel.world.set_flashing_pairs(&flashing)
    }

    /// `update_wormhole_damage` makes the wormholes of every player whose damage (from the server)
    /// rose flash red for the settings' `wormholes.hit_flash_s` from `now_s`, and destroys this
    /// player's once theirs reaches 1.
    ///
    /// Example: `self.update_wormhole_damage(now_s);`
    fn update_wormhole_damage(&mut self, now_s: f64) {
        let all: Vec<(Option<u32>, PlayerWormholes)> = self.get_all_wormholes();
        for &(owner, wormholes) in &all {
            let last_frac: f64 = match self.wormhole_damages.iter().find(|(o, _)| *o == owner) {
                Some(&(_, frac)) => frac,
                None => 0.0,
            };
            if wormholes.damage_frac > last_frac {
                self.wormhole_flashes.retain(|(o, _)| *o != owner);
                let until_s: f64 = now_s + self.settings.wormholes.hit_flash_s;
                self.wormhole_flashes.push((owner, until_s));
            }
        }
        self.wormhole_damages = all.iter().map(|&(o, w)| (o, w.damage_frac)).collect();
        if all[0].1.damage_frac >= 1.0 && !self.thrown.placed.get_places().is_empty() {
            self.thrown.clear();
            self.show_message("Your wormholes were destroyed".to_string());
        }
    }

    /// `is_flashing` says whether `owner`'s wormholes (`None` for this player's) flash red at `now_s`.
    ///
    /// Example: `let is_red = self.is_flashing(Some(3), now_s);`
    fn is_flashing(&self, owner: Option<u32>, now_s: f64) -> bool {
        self.wormhole_flashes
            .iter()
            .any(|&(o, until_s)| o == owner && until_s > now_s)
    }

    /// `get_all_wormholes` gives every player's wormholes with their owner: this player's first
    /// (`None`, with the damage the server counts for them), then the others' by the last snapshot.
    ///
    /// Example: `let all = self.get_all_wormholes();`
    fn get_all_wormholes(&self) -> Vec<(Option<u32>, PlayerWormholes)> {
        let (own_damage_frac, remote) = self.multiplayer.as_ref().map_or((0.0, Vec::new()), |m| {
            let remote = m.client.players.iter().map(|p| (Some(p.id), p.wormholes));
            (m.client.wormhole_damage_frac, remote.collect())
        });
        let own: PlayerWormholes = PlayerWormholes {
            damage_frac: own_damage_frac,
            ..self.thrown.placed
        };
        std::iter::once((None, own)).chain(remote).collect()
    }

    /// `get_wormhole_markers` lists the glowing balls to draw for every player's thrown mouths, with
    /// their owner (as `get_all_wormholes`): throws in the air, and mouths down whose partner isn't
    /// (red while they flash).
    ///
    /// Example: `let markers = self.get_wormhole_markers();`
    pub(crate) fn get_wormhole_markers(&self) -> Vec<(Option<u32>, Marker)> {
        let r: f64 = self.voxel.world.r;
        let now_s: f64 = self.start.elapsed().as_secs_f64();
        let mut markers: Vec<(Option<u32>, Marker)> = Vec::new();
        for (owner, wormholes) in self.get_all_wormholes() {
            for mut marker in wormholes.get_markers(r) {
                if marker.is_mouth && self.is_flashing(owner, now_s) {
                    marker.rgb = HIT_FLASH_COLOR;
                }
                markers.push((owner, marker));
            }
        }
        markers
    }

    /// `report_wormhole_hits` tells the server, once per shot, whose enemy wormholes this player's
    /// shot went into (`entered_pairs`, indices into the world's thrown pairs), which counts toward
    /// destroying them by the gun, or hit (`hit_marker`, an index into `markers`, if it is a lone
    /// mouth's ball), which destroys it at once. A shot at this player's own or a teammate's
    /// wormholes does nothing.
    ///
    /// Example: `self.report_wormhole_hits(&[0], None, &markers);`
    pub(crate) fn report_wormhole_hits(
        &mut self,
        entered_pairs: &[usize],
        hit_marker: Option<usize>,
        markers: &[(Option<u32>, Marker)],
    ) {
        let Some(multiplayer) = &self.multiplayer else {
            return;
        };
        let team: Option<u32> = multiplayer.client.team;
        let is_enemy = |&(id, _): &(u32, u8)| {
            let player = multiplayer.client.players.iter().find(|p| p.id == id);
            player.is_some_and(|p| Some(p.team) != team)
        };
        let gun_hits_to_destroy: u8 = match self.gun {
            Gun::Laser => self.settings.wormholes.sniper_hits_to_destroy,
            Gun::Rifle => self.settings.wormholes.rifle_hits_to_destroy,
        };
        let mut enemy_hits: Vec<(u32, u8)> = entered_pairs
            .iter()
            .filter_map(|&i| self.thrown_pair_owners[i])
            .map(|id| (id, gun_hits_to_destroy))
            .chain(
                hit_marker
                    .filter(|&k| markers[k].1.is_mouth)
                    .and_then(|k| markers[k].0)
                    .map(|id| (id, LONE_MOUTH_HITS_TO_DESTROY)),
            )
            .filter(is_enemy)
            .collect();
        enemy_hits.dedup_by_key(|&mut (id, _)| id);
        if self.has_shot_hit_wormholes || enemy_hits.is_empty() {
            return;
        }
        self.has_shot_hit_wormholes = true;
        let multiplayer: &mut Multiplayer = self.multiplayer.as_mut().expect("checked above");
        for (id, hits_to_destroy) in enemy_hits {
            if let Err(error) = multiplayer.client.send_wormhole_hit(id, hits_to_destroy) {
                self.show_message(format!("Couldn't reach the server: {error}"));
                return;
            }
        }
    }

    /// `is_firing` says whether this player's laser is on: on for a scripted route, or a shot fired
    /// by a click in the last `sniper.shot_s` (the settings), and only while this player can shoot.
    ///
    /// Example: `let is_firing = self.is_firing();`
    pub(crate) fn is_firing(&self) -> bool {
        (self.is_laser_on || self.shot_time_s > 0.0) && self.can_shoot()
    }

    /// `get_laser_aim` gives the frame this player's laser fires from while it is on: `eye` (the
    /// eye's frame, turned toward the crosshair's point in third person: `get_shot_eye`) turned by
    /// this shot's noise (`shot_offset`), or `None` while it is off. The window sends it instead of
    /// the eye's frame while firing, so others trace the same shot.
    ///
    /// Example: `let laser_aim = self.get_laser_aim(self.get_shot_eye());`
    pub(crate) fn get_laser_aim(&self, eye: DMat4) -> Option<DMat4> {
        self.is_firing().then(|| self.shot_offset.aim(eye))
    }

    /// `get_boom_frac` gives how far out along its boom this player's camera is now
    /// (`third_person::ViewSwitch::get_boom_frac`): 0, at the eye, on a scripted route.
    ///
    /// Example: `if self.get_boom_frac() > 0.0 { ... }`
    fn get_boom_frac(&self) -> f64 {
        if !self.route.is_empty() {
            return 0.0;
        }
        self.view_switch.get_boom_frac(self.aim_frac)
    }

    /// `get_third_person_view` gives, while the camera is out of the eye on a voxel map, where it
    /// is (`third_person::place_camera`) and the eye turned toward what its crosshair covers
    /// (`third_person::aim_at_crosshair`), both in the map with the other players' bodies; `None`
    /// while the camera is at the eye.
    ///
    /// Example: `if let Some((placed, shot_eye)) = self.get_third_person_view() { ... }`
    pub(crate) fn get_third_person_view(&self) -> Option<(third_person::ThirdPersonCamera, DMat4)> {
        let boom_frac: f64 = self.get_boom_frac();
        if boom_frac <= 0.0 {
            return None;
        }
        let voxel: &VoxelState = &self.voxel;
        let players: &[net::RemotePlayer] = self
            .multiplayer
            .as_ref()
            .map_or(&[], |m| m.client.players.as_slice());
        let world: VoxelWorld = multiplayer::with_bodies(&voxel.world, players);
        let eye: DMat4 = voxel.player.get_camera_frame(&voxel.world);
        let placed: third_person::ThirdPersonCamera =
            third_person::place_camera(&world, eye, boom_frac);
        Some((
            placed,
            third_person::aim_at_crosshair(&world, eye, placed.frame),
        ))
    }

    /// `get_shot_eye` gives the frame a shot leaves from before its noise: the eye's, turned toward
    /// what the crosshair covers while the camera is out of the eye (`get_third_person_view`).
    ///
    /// Example: `let laser_aim = self.get_laser_aim(self.get_shot_eye());`
    pub(crate) fn get_shot_eye(&self) -> DMat4 {
        self.get_third_person_view()
            .map_or_else(|| self.get_eye_frame(), |(_, shot_eye)| shot_eye)
    }

    /// `can_shoot` says whether this player may shoot: once no movement key has been held for the
    /// server settings' `movement.shoot_after_moving_s` (walking crouched doesn't count, and
    /// standing, falling or flying with no key held may shoot), and, in a game on a server, only
    /// once in the game (welcomed, so not in the lobby, while joining or while out after a hit),
    /// not while holding `E` on a console, and not once a team has won.
    ///
    /// Example: `if self.can_shoot() { ... }`
    pub(crate) fn can_shoot(&self) -> bool {
        self.since_movement_key_s >= self.settings.movement.shoot_after_moving_s
            && self.multiplayer.as_ref().is_none_or(|m| {
                m.client.id.is_some() && m.disabling.is_none() && m.client.game.winner.is_none()
            })
    }

    /// `switch_gun` makes `gun` the one in hand, stopping a shot in flight.
    ///
    /// Example: `self.switch_gun(Gun::Rifle);`
    pub(crate) fn switch_gun(&mut self, gun: Gun) {
        self.gun = gun;
        self.shot_time_s = 0.0;
    }

    /// `get_time_rate_frac` gives how fast this player's own time runs where their eye is, as a
    /// share of time far from the black hole (`VoxelWorld::get_time_rate_frac`, W-107): 1 off a
    /// voxel map.
    ///
    /// Example: `let own_dt_s = dt_s * self.get_time_rate_frac();`
    pub(crate) fn get_time_rate_frac(&self) -> f64 {
        let eye: DMat4 = self.voxel.player.get_camera_frame(&self.voxel.world);
        self.voxel.world.get_time_rate_frac(eye.col(AXIS_W))
    }

    /// `update_rifle` fires the rifle while its trigger is held and this player can shoot: a
    /// `RIFLE_PULSE_S` pulse of the beam every `RIFLE_SHOT_PERIOD_S`, each adding
    /// 1 / `RIFLE_OVERHEAT_SHOT_COUNT` of heat. At full heat it overheats; it cools fully in
    /// `cool_down_s` while not firing, and fires again once cool. Each shot strays by noise that
    /// grows with the heat (`shot_noise::ShotNoise::sample_rifle`), and recoils: the view kicks
    /// up by `recoil_up_rad` and sideways by `recoil_sideways_rad`, alternately left and right, and
    /// the rifle jolts back (`recoil_frac`, settling over `recoil_settle_s`); all from the settings.
    ///
    /// Example: `self.update_rifle(1.0 / 60.0);`
    pub(crate) fn update_rifle(&mut self, dt_s: f64) {
        let rifle: settings::RifleSettings = self.settings.rifle.clone();
        self.rifle_next_shot_s = (self.rifle_next_shot_s - dt_s).max(0.0);
        self.recoil_frac = if rifle.recoil_settle_s > 0.0 {
            (self.recoil_frac - dt_s / rifle.recoil_settle_s).max(0.0)
        } else {
            0.0
        };
        let is_firing_rifle: bool = self.gun == Gun::Rifle
            && self.is_trigger_held
            && self.can_shoot()
            && !self.is_overheated;
        if !is_firing_rifle {
            self.rifle_heat_frac = (self.rifle_heat_frac - dt_s / rifle.cool_down_s).max(0.0);
            self.is_overheated &= self.rifle_heat_frac > 0.0;
        } else if self.rifle_next_shot_s <= 0.0 {
            self.shot_time_s = rifle.pulse_s;
            self.rifle_next_shot_s = 1.0 / rifle.shots_per_s;
            self.has_rifle_shot_hit = false;
            self.has_shot_hit_wormholes = false;
            self.shot_offset = self.shot_noise.sample_rifle(&rifle, self.rifle_heat_frac);
            self.beam_speckle_frac =
                shot_noise::get_rifle_speckle_frac(&rifle, self.rifle_heat_frac);
            self.rifle_heat_frac += 1.0 / f64::from(rifle.shots_until_overheat);
            self.is_overheated = self.rifle_heat_frac >= 1.0;
            self.rifle_shot_count += 1;
            self.recoil_frac = f64::from(u8::from(rifle.recoil_settle_s > 0.0));
            let side: f64 = if self.rifle_shot_count.is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            let (dx_px, dy_px) = (
                side * rifle.recoil_sideways_rad / voxel_world::LOOK_RAD_PER_PX,
                -rifle.recoil_up_rad / voxel_world::LOOK_RAD_PER_PX,
            );
            self.voxel.player.look(dx_px, dy_px)
        }
    }

    /// `update_disabling` follows this player holding `E` (`is_holding_e`) on one of the other
    /// team's consoles that is still on, within `CONSOLE_REACH_M` straight ahead of the eye (toward
    /// the crosshair's point in third person: `get_shot_eye`): the seconds held on that same
    /// console, or nothing. The server times it too, and turns the console off.
    ///
    /// Example: `self.update_disabling(true, 1.0 / 60.0);`
    pub(crate) fn update_disabling(&mut self, is_holding_e: bool, dt_s: f64) {
        let shot_eye: Option<DMat4> = is_holding_e.then(|| self.get_shot_eye());
        let (voxel, Some(multiplayer)) = (&self.voxel, &mut self.multiplayer) else {
            return;
        };
        let game: net::GameState = multiplayer.client.game;
        let aimed: Option<u8> = match (multiplayer.client.team, shot_eye) {
            (Some(team), Some(eye)) if game.winner.is_none() => voxel
                .world
                .find_aimed_material(eye, self.settings.consoles.reach_m)
                .and_then(|m| voxel.world.consoles.iter().position(|c| c.material == m))
                .filter(|&i| voxel.world.consoles[i].team != team && game.off_mask >> i & 1 == 0)
                .map(|i| i as u8),
            _ => None,
        };
        multiplayer.disabling = match (aimed, multiplayer.disabling) {
            (Some(console), Some((held, held_s))) if console == held => {
                Some((console, held_s + dt_s))
            }
            (Some(console), _) => Some((console, 0.0)),
            (None, _) => None,
        };
    }

    /// `get_console_states` gives the consoles turned off (bit i for console i), and how far along
    /// turning off each console is, from 0 to 1: the most of everyone holding `E` on it, with this
    /// player's own seconds held and the server's timing of the others. Without a server, every
    /// console is on and nobody turns any off.
    ///
    /// Example: `let (off_mask, disabling_fracs) = self.get_console_states();`
    pub(crate) fn get_console_states(&self) -> (u32, [f64; net::MAX_CONSOLE_COUNT]) {
        let mut disabling_fracs: [f64; net::MAX_CONSOLE_COUNT] = [0.0; net::MAX_CONSOLE_COUNT];
        let Some(multiplayer) = &self.multiplayer else {
            return (0, disabling_fracs);
        };
        let hold_s: f64 = self.settings.consoles.hold_to_disable_s;
        let own: Option<(u8, f64)> = multiplayer
            .disabling
            .map(|(console, held_s)| (console, (held_s / hold_s).min(1.0)));
        for (console, frac) in multiplayer
            .client
            .players
            .iter()
            .filter_map(|player| Some((player.disabling_console?, player.disabling_frac)))
            .chain(own)
        {
            let most_frac: &mut f64 = &mut disabling_fracs[usize::from(console)];
            *most_frac = most_frac.max(frac);
        }
        (multiplayer.client.game.off_mask, disabling_fracs)
    }

    /// `update_gun` counts the time since this player last held a movement key (moving without
    /// crouching, or jumping), dropping a shot in flight while one is held, and moves the gun: its
    /// step phase and how far into stepping it is, the head bob's (so the gun steps with the eye), and
    /// its pose toward jogging while running and back to aiming otherwise.
    ///
    /// Example: `self.update_gun(&input, dt_s);`
    pub(crate) fn update_gun(&mut self, input: &Input, dt_s: f64) {
        let is_moving: bool = input.forward || input.back || input.left || input.right;
        let is_running: bool = self.voxel.player.is_sprinting;
        let is_walking_upright: bool = is_moving && !input.crouch;
        self.since_movement_key_s = if is_walking_upright || input.up {
            0.0
        } else {
            self.since_movement_key_s + dt_s
        };
        if !self.can_shoot() {
            self.shot_time_s = 0.0;
        }
        (self.gun_phase_rad, self.step_frac) =
            (self.voxel.player.bob_phase_rad, self.voxel.player.bob_frac);
        let target: f64 = f64::from(u8::from(is_running));
        self.run_pose_frac +=
            (target - self.run_pose_frac).clamp(-GUN_POSE_PER_S * dt_s, GUN_POSE_PER_S * dt_s);
    }

    /// `get_hit_effect` gives what the last hit on this player still does to the scene, fading over
    /// the player settings' `hit_effects.effect_s`: red edges, a blur from the edges inward, and a
    /// shake in a new direction each frame (the golden angle apart).
    ///
    /// Example: `let hit = self.get_hit_effect();`
    pub(crate) fn get_hit_effect(&self) -> render_metal::HitEffect {
        let when_hit = &self.player_settings.hit_effects;
        let shake_rad: f64 = self.total_frame_count as f64 * voxel_world::SPAWN_TURN_RAD;
        let shake_px: f64 = when_hit.shake_px * self.hit_effect_frac;
        render_metal::HitEffect {
            strength_frac: self.hit_effect_frac as f32,
            red_frac: when_hit.red_frac as f32,
            blur_px: when_hit.blur_px as f32,
            shake_px: [shake_rad.cos(), shake_rad.sin()].map(|c| (c * shake_px) as f32),
        }
    }

    /// `get_wand` gives this player's wand (`make_wand`, with its tip in the team's colour), or
    /// `None` while text covers the view (`is_view_covered`) or this player's own body shows in
    /// third person (the last frame packed).
    ///
    /// Example: `let wand = self.get_wand();`
    pub(crate) fn get_wand(&self) -> Option<render_metal::OverlayWand> {
        let team: u32 = self.get_team();
        (!self.is_view_covered() && !self.is_own_body_shown).then(|| {
            make_wand(
                self.run_pose_frac,
                self.gun_phase_rad,
                self.recoil_frac,
                team,
                self.gun,
                get_sway(
                    self.gun.get_model(),
                    self.step_frac,
                    self.run_pose_frac,
                    self.gun_phase_rad,
                    &self.player_settings.view.head_bob,
                ),
            )
        })
    }
}
