//! `avatar_animation.rs` chooses each player's clip of the body model, W-106 (Teal robot player
//! model), from what this window sees of them over the network: standing (`Idle`), walking or
//! running (by their speed over the ground, the stride kept in step with it), in the air (`Jump`,
//! from takeoff), and, once eliminated, a body lying where they fell (`Death`) until they could
//! respawn.
//!
//! Interface: `AvatarAnimator::New`, `AvatarAnimator`, `Corpse`, `OWN_TRACK_ID`.
//!
//! Example: `let poses = animator.update(&world, &[(2, eye)], now_s);`

use glam::{DMat4, DVec4};

use crate::avatar_model::{AvatarClip, AvatarModel};
use crate::geometry::{AXIS_W, get_distance};
use crate::voxel_world::{VoxelPlayer, VoxelWorld};

/// `OWN_TRACK_ID` is the id this window gives its own player's eye, beside the others' ids.
pub const OWN_TRACK_ID: u32 = u32::MAX;
const SPEED_SMOOTHING_S: f64 = 0.12;
const STALE_SNAPSHOT_S: f64 = 0.25;
const MIN_WALK_M_PER_S: f64 = 0.8;
const WALK_CYCLE_M: f64 = 1.6;
const RUN_CYCLE_M: f64 = 2.4;
const MAX_STRIDES_PER_S: f64 = 2.5;
const MIN_AIR_S: f64 = 0.08;

/// `Track` is what this window remembers of one player: their id, last eye frame, feet frame and
/// ground point (`under`, the eye's foot on the level's base plane, which a jump doesn't move),
/// when the eye last moved, their smoothed speed over the ground, where they are in the stride
/// (fraction of a loop), and when they left the ground (`None` while on it).
struct Track {
    id: u32,
    eye: DMat4,
    feet: DMat4,
    under: DVec4,
    moved_s: f64,
    speed_m_per_s: f64,
    stride_frac: f64,
    left_ground_s: Option<f64>,
}

/// `Corpse` is an eliminated player's body: where it lies, when it fell and the player's team.
#[derive(Clone, Copy)]
pub struct Corpse {
    pub feet: DMat4,
    pub fell_s: f64,
    pub team: u32,
}

/// `AvatarAnimator` keeps a `Track` for every player in view and the corpses still lying.
pub struct AvatarAnimator {
    tracks: Vec<Track>,
    corpses: Vec<Corpse>,
    last_s: f64,
}

impl AvatarAnimator {
    /// `New` is an animator that has seen no one yet.
    ///
    /// Example: `let animator = AvatarAnimator::New();`
    #[allow(non_snake_case)]
    pub fn New() -> AvatarAnimator {
        AvatarAnimator {
            tracks: Vec::new(),
            corpses: Vec::new(),
            last_s: 0.0,
        }
    }

    /// `update` follows the players whose ids and eye frames are `eyes` to `now_s` (seconds, never
    /// going back) and gives each one's clip and time in it, in their order. A player not seen
    /// before stands idle at first.
    ///
    /// Example: `let poses = animator.update(&world, &[(2, eye)], 12.5);`
    pub fn update(
        &mut self,
        world: &VoxelWorld,
        eyes: &[(u32, DMat4)],
        now_s: f64,
    ) -> Vec<(AvatarClip, f64)> {
        let dt_s: f64 = (now_s - self.last_s).max(0.0);
        self.last_s = now_s;
        self.tracks
            .retain(|track| eyes.iter().any(|&(id, _)| id == track.id));
        let run_from_m_per_s: f64 = (world.player.walk_m_per_s + world.player.sprint_m_per_s) / 2.0;
        eyes.iter()
            .map(|&(id, eye)| {
                let seen: VoxelPlayer = VoxelPlayer::FromEye(world, eye);
                let feet: DMat4 = seen.get_feet_frame(world);
                let under: DVec4 = seen.body.col(AXIS_W);
                let is_on_ground: bool = world.is_eye_supported(eye);
                let track: &mut Track = match self.tracks.iter().position(|t| t.id == id) {
                    Some(k) => &mut self.tracks[k],
                    None => {
                        self.tracks.push(Track {
                            id,
                            eye,
                            feet,
                            under,
                            moved_s: now_s,
                            speed_m_per_s: 0.0,
                            stride_frac: 0.0,
                            left_ground_s: None,
                        });
                        self.tracks.last_mut().expect("a track was just added")
                    }
                };
                let ground: Ground = Ground {
                    feet,
                    under,
                    is_on_ground,
                    r: world.r,
                };
                track.follow(eye, ground, run_from_m_per_s, now_s, dt_s);
                track.get_pose(&world.body, run_from_m_per_s, now_s)
            })
            .collect()
    }

    /// `add_corpse` lays player `victim_id`'s body, of `team`, where this window last saw them,
    /// falling at `now_s`; it does nothing for a player never seen. Call it before the `update`
    /// that no longer lists them, which forgets their track.
    ///
    /// Example: `animator.add_corpse(3, 1, 12.5);`
    pub fn add_corpse(&mut self, victim_id: u32, team: u32, now_s: f64) {
        if let Some(track) = self.tracks.iter().find(|t| t.id == victim_id) {
            self.corpses.push(Corpse {
                feet: track.feet,
                fell_s: now_s,
                team,
            });
        }
    }

    /// `get_corpses` gives the corpses still lying at `now_s`, forgetting those that fell more
    /// than `lie_s` ago, each with its `Death` pose.
    ///
    /// Example: `let corpses = animator.get_corpses(12.5, 3.0);`
    pub fn get_corpses(&mut self, now_s: f64, lie_s: f64) -> Vec<(Corpse, (AvatarClip, f64))> {
        self.corpses.retain(|c| now_s - c.fell_s <= lie_s);
        self.corpses
            .iter()
            .map(|&c| (c, (AvatarClip::Death, now_s - c.fell_s)))
            .collect()
    }
}

/// `Ground` is where a player's feet and ground point are this frame, whether they stand on
/// something, and the curvature radius R in meters.
struct Ground {
    feet: DMat4,
    under: DVec4,
    is_on_ground: bool,
    r: f64,
}

/// `choose_gait` gives the clip for moving at `speed_m_per_s` over the ground: idle below
/// `MIN_WALK_M_PER_S`, walking below `run_from_m_per_s`, running from there.
///
/// Example: `let clip = choose_gait(10.0, 14.0);`
fn choose_gait(speed_m_per_s: f64, run_from_m_per_s: f64) -> AvatarClip {
    if speed_m_per_s < MIN_WALK_M_PER_S {
        AvatarClip::Idle
    } else if speed_m_per_s < run_from_m_per_s {
        AvatarClip::Walk
    } else {
        AvatarClip::Run
    }
}

impl Track {
    /// `follow` takes the player's eye `eye` and `ground` at `now_s`: the speed over the ground
    /// (smoothed over `SPEED_SMOOTHING_S`, and zero once the eye has stood still for
    /// `STALE_SNAPSHOT_S`, as snapshots don't come every frame), the stride advanced by it (one
    /// loop per `WALK_CYCLE_M` or `RUN_CYCLE_M`, at most `MAX_STRIDES_PER_S`, so the feet slide
    /// at the game's fast speeds rather than blur), and whether they are off the ground.
    ///
    /// Example: `track.follow(eye, ground, 14.0, 12.5, 0.016);`
    fn follow(&mut self, eye: DMat4, ground: Ground, run_from_m_per_s: f64, now_s: f64, dt_s: f64) {
        let Ground {
            feet,
            under,
            is_on_ground,
            r,
        } = ground;
        if eye != self.eye {
            let since_s: f64 = (now_s - self.moved_s).max(dt_s).max(f64::EPSILON);
            let ground_m: f64 = get_distance(under, self.under) * r;
            let blend_frac: f64 = 1.0 - (-since_s / SPEED_SMOOTHING_S).exp();
            self.speed_m_per_s += (ground_m / since_s - self.speed_m_per_s) * blend_frac;
            (self.eye, self.feet, self.under, self.moved_s) = (eye, feet, under, now_s);
        } else if now_s - self.moved_s > STALE_SNAPSHOT_S {
            self.speed_m_per_s = 0.0;
        }
        let cycle_m: f64 = match choose_gait(self.speed_m_per_s, run_from_m_per_s) {
            AvatarClip::Run => RUN_CYCLE_M,
            _ => WALK_CYCLE_M,
        };
        let rate_per_s: f64 = (self.speed_m_per_s / cycle_m).min(MAX_STRIDES_PER_S);
        self.stride_frac = (self.stride_frac + rate_per_s * dt_s).fract();
        self.left_ground_s = match (is_on_ground, self.left_ground_s) {
            (true, _) => None,
            (false, None) => Some(now_s),
            (false, Some(left_s)) => Some(left_s),
        };
    }

    /// `get_pose` gives the clip of `body` and the time in it for the player now: in the air for
    /// `MIN_AIR_S` or more, jumping; otherwise by `choose_gait`. A stride's time is its place in
    /// the loop times the clip's length, so walking and running hand over in step.
    ///
    /// Example: `let (clip, time_s) = track.get_pose(&world.body, 14.0, 12.5);`
    fn get_pose(&self, body: &AvatarModel, run_from_m_per_s: f64, now_s: f64) -> (AvatarClip, f64) {
        if let Some(left_s) = self.left_ground_s
            && now_s - left_s >= MIN_AIR_S
        {
            return (AvatarClip::Jump, now_s - left_s - MIN_AIR_S);
        }
        match choose_gait(self.speed_m_per_s, run_from_m_per_s) {
            AvatarClip::Idle => (AvatarClip::Idle, now_s),
            clip => (clip, self.stride_frac * body.get_duration_s(clip)),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for `avatar_animation.rs` with one player moving along flat ground at Lunar Surface's
    //! blue spawn, seen at 60 frames a second.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Speed over the ground | still, walking speed, sprinting speed | `the_gait_follows_the_speed` |
    //! | Ground | on it, just left (under `MIN_AIR_S`), in the air (`Jump` from takeoff) | `jumping_starts_in_the_air` |
    //! | Snapshots | a repeat within `STALE_SNAPSHOT_S` (speed kept), none for longer (idle) | `snapshots_may_repeat` |
    //! | Stride at the hand-over | walking, then running at the same place in the loop | `the_gait_follows_the_speed` |
    //! | Corpse | of a player never seen (none), lying (with its team), older than its time (gone) | `corpses_lie_until_respawn` |

    use super::*;
    use crate::map::{Map, get_map_folder};

    const FRAME_S: f64 = 1.0 / 60.0;
    const START_M: [f64; 2] = [-27.0, 0.0];
    const LIE_S: f64 = 3.0;

    /// `load_world` gives the Lunar Surface world.
    ///
    /// Example: `let world = load_world();`
    fn load_world() -> VoxelWorld {
        VoxelWorld::FromMap(&Map::Load(&get_map_folder("lunar_surface")).expect("the map"))
            .expect("the world")
    }

    /// `make_player` gives player 2's id and eye, standing `u_m` along the ground from
    /// `START_M`, looking along it, with the eye `rise_m` above standing height.
    ///
    /// Example: `let player = make_player(&world, 1.5, 0.0);`
    fn make_player(world: &VoxelWorld, u_m: f64, rise_m: f64) -> (u32, DMat4) {
        let at_m: [f64; 2] = [START_M[0] + u_m, START_M[1]];
        let mut body: VoxelPlayer = VoxelPlayer::AtView(world, at_m, [40.0, 0.0], None, 0.0);
        body.h_m += rise_m;
        (2, body.get_camera_frame(world))
    }

    /// `walk` moves the player at `speed_m_per_s` for `duration_s` from `u_m`, updating
    /// `animator` every frame from `start_s`, and gives the last pose, the place reached and the
    /// time.
    ///
    /// Example: `let (pose, u_m, now_s) = walk(&world, &mut animator, 10.0, 1.0, 0.0, 0.0);`
    fn walk(
        world: &VoxelWorld,
        animator: &mut AvatarAnimator,
        speed_m_per_s: f64,
        duration_s: f64,
        u_m: f64,
        start_s: f64,
    ) -> ((AvatarClip, f64), f64, f64) {
        let frame_count: usize = (duration_s / FRAME_S).round() as usize;
        let mut pose: (AvatarClip, f64) = (AvatarClip::Idle, 0.0);
        for i in 1..=frame_count {
            let t_s: f64 = i as f64 * FRAME_S;
            let player: (u32, DMat4) = make_player(world, u_m + speed_m_per_s * t_s, 0.0);
            pose = animator.update(world, &[player], start_s + t_s)[0];
        }
        (pose, u_m + speed_m_per_s * duration_s, start_s + duration_s)
    }

    #[test]
    fn the_gait_follows_the_speed() {
        let world: VoxelWorld = load_world();
        let mut animator: AvatarAnimator = AvatarAnimator::New();
        let (still, u_m, now_s) = walk(&world, &mut animator, 0.0, 0.5, 0.0, 0.0);
        assert_eq!(still.0, AvatarClip::Idle);
        let walk_m_per_s: f64 = world.player.walk_m_per_s;
        let (walking, mut u_m, mut now_s) =
            walk(&world, &mut animator, walk_m_per_s, 1.0, u_m, now_s);
        assert_eq!(walking.0, AvatarClip::Walk);
        let get_frac = |(clip, time_s): (AvatarClip, f64)| time_s / world.body.get_duration_s(clip);
        let mut last: (AvatarClip, f64) = walking;
        let sprint_m_per_s: f64 = world.player.sprint_m_per_s;
        while last.0 == AvatarClip::Walk {
            let next: (AvatarClip, f64);
            (next, u_m, now_s) = walk(&world, &mut animator, sprint_m_per_s, FRAME_S, u_m, now_s);
            let step_frac: f64 = (get_frac(next) - get_frac(last)).rem_euclid(1.0);
            assert!(
                step_frac <= MAX_STRIDES_PER_S * FRAME_S + 1e-9,
                "the stride jumps {step_frac} of a loop"
            );
            assert!(now_s < 3.0, "never runs");
            last = next;
        }
        assert_eq!(last.0, AvatarClip::Run);
    }

    #[test]
    fn jumping_starts_in_the_air() {
        let world: VoxelWorld = load_world();
        let mut animator: AvatarAnimator = AvatarAnimator::New();
        let standing: (u32, DMat4) = make_player(&world, 0.0, 0.0);
        assert_eq!(
            animator.update(&world, &[standing], 0.0)[0].0,
            AvatarClip::Idle
        );
        let airborne: (u32, DMat4) = make_player(&world, 0.0, 1.0);
        let just_left: (AvatarClip, f64) =
            animator.update(&world, std::slice::from_ref(&airborne), FRAME_S)[0];
        assert_eq!(just_left.0, AvatarClip::Idle);
        let later_s: f64 = FRAME_S + MIN_AIR_S + 0.2;
        let (clip, time_s) = animator.update(&world, &[airborne], later_s)[0];
        assert_eq!(clip, AvatarClip::Jump);
        assert!((time_s - 0.2).abs() < 1e-9, "the jump is {time_s} s in");
    }

    #[test]
    fn snapshots_may_repeat() {
        let world: VoxelWorld = load_world();
        let mut animator: AvatarAnimator = AvatarAnimator::New();
        let walk_m_per_s: f64 = world.player.walk_m_per_s;
        let (_, u_m, now_s) = walk(&world, &mut animator, walk_m_per_s, 1.0, 0.0, 0.0);
        let last: (u32, DMat4) = make_player(&world, u_m, 0.0);
        let repeat: (AvatarClip, f64) =
            animator.update(&world, std::slice::from_ref(&last), now_s + FRAME_S)[0];
        assert_eq!(repeat.0, AvatarClip::Walk);
        let stale_s: f64 = now_s + STALE_SNAPSHOT_S + FRAME_S;
        assert_eq!(
            animator.update(&world, &[last], stale_s)[0].0,
            AvatarClip::Idle
        );
    }

    #[test]
    fn corpses_lie_until_respawn() {
        let world: VoxelWorld = load_world();
        let mut animator: AvatarAnimator = AvatarAnimator::New();
        animator.add_corpse(2, 1, 0.0);
        assert!(animator.get_corpses(0.0, LIE_S).is_empty());
        animator.update(&world, &[make_player(&world, 0.0, 0.0)], 0.0);
        animator.add_corpse(2, 1, 1.0);
        let lying: Vec<(Corpse, (AvatarClip, f64))> = animator.get_corpses(2.0, LIE_S);
        assert_eq!(lying.len(), 1);
        assert_eq!(lying[0].1, (AvatarClip::Death, 1.0));
        assert_eq!(lying[0].0.team, 1);
        assert!(
            animator
                .get_corpses(1.0 + LIE_S + FRAME_S, LIE_S)
                .is_empty()
        );
    }
}
