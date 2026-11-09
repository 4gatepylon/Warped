//! `thrown_wormholes` lets each player throw a linked pair of wormhole mouths, one purple and one
//! green, as on Lunar Surface (`../hyperbolic_wormholes_rust/src/thrown_wormholes.rs`, W-126 (same
//! play in every level)) in the forest's flat space: a throw is lobbed from the eye under its own
//! gravity, drops to the floor below whatever it hits, and the mouth then hovers there at eye
//! height. Once both of a player's mouths are down they link. `net.rs` (copied from Lunar Surface)
//! sends these types as they are.
//!
//! Example: `let flight = fly_throw(eye, thrown.get_next_color(), &settings.wormholes, find_hit);`
//!
//! Interface: `WormholeColor`, `MouthPlace`, `Flight`, `fly_throw`, `Marker`, `PlayerWormholes`,
//! `ThrownWormholes`, `PURPLE_COLOR`, `GREEN_COLOR`, `HIT_FLASH_COLOR`.
//!
//! Must know:
//! - `MouthPlace` keeps Lunar Surface's field names for the protocol: `u_m` is x, `v_m` is z and
//!   `h_m` is y (height), in the forest's meters; a flying throw's point is (x, y, z, 1).
//! - A throw flies straight through warps' bending and throats.

use glam::{DMat4, DVec3, DVec4, Vec3};

use crate::settings::WormholeSettings;

pub const PURPLE_COLOR: [f32; 3] = [0.62, 0.22, 1.0];
pub const GREEN_COLOR: [f32; 3] = [0.2, 1.0, 0.4];
pub const HIT_FLASH_COLOR: [f32; 3] = [1.0, 0.12, 0.06];
const FLIGHT_STEP_S: f64 = 1.0 / 120.0;
const BACK_OFF_M: f64 = 0.05;
const MAX_DROP_M: f64 = 200.0;
const FLYING_MARKER_M: f64 = 0.15;
const UNLINKED_MARKER_M: f64 = 0.3;

/// `WormholeColor` is which of a player's two mouths: the purple one is thrown first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WormholeColor {
    Purple,
    Green,
}

impl WormholeColor {
    /// `get_other` gives the other mouth's colour.
    ///
    /// Example: `let next = WormholeColor::Purple.get_other();`
    pub fn get_other(self) -> WormholeColor {
        match self {
            WormholeColor::Purple => WormholeColor::Green,
            WormholeColor::Green => WormholeColor::Purple,
        }
    }

    /// `get_name` gives the colour's name, for messages.
    ///
    /// Example: `let message = format!("{} wormhole placed", color.get_name());`
    pub fn get_name(self) -> &'static str {
        match self {
            WormholeColor::Purple => "Purple",
            WormholeColor::Green => "Green",
        }
    }

    /// `get_rgb` gives the colour itself, in linear RGB.
    ///
    /// Example: `let [r, g, b] = WormholeColor::Green.get_rgb();`
    pub fn get_rgb(self) -> [f32; 3] {
        match self {
            WormholeColor::Purple => PURPLE_COLOR,
            WormholeColor::Green => GREEN_COLOR,
        }
    }
}

/// `MouthPlace` is where a thrown mouth hovers: x (`u_m`), z (`v_m`) and height (`h_m`) of its
/// centre in meters, and the heading it flew in (`yaw_rad`, 0 along -z, turning toward +x), which
/// fits a snapshot in 16 bytes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MouthPlace {
    pub u_m: f64,
    pub v_m: f64,
    pub h_m: f64,
    pub yaw_rad: f64,
}

impl MouthPlace {
    /// `Landed` gives the place of a mouth that came down on `floor_m` moving along `velocity`:
    /// `hover_m` above it, heading the way it flew.
    ///
    /// Example: `let place = MouthPlace::Landed(floor_m, velocity, 1.68);`
    #[allow(non_snake_case)]
    pub fn Landed(floor_m: DVec3, velocity: DVec3, hover_m: f64) -> MouthPlace {
        MouthPlace {
            u_m: floor_m.x,
            v_m: floor_m.z,
            h_m: floor_m.y + hover_m,
            yaw_rad: velocity.x.atan2(-velocity.z),
        }
    }

    /// `get_centre_m` gives the mouth's centre.
    ///
    /// Example: `let centre_m: Vec3 = place.get_centre_m();`
    pub fn get_centre_m(&self) -> Vec3 {
        DVec3::new(self.u_m, self.h_m, self.v_m).as_vec3()
    }
}

/// `Flight` is one throw: its colour, where it was every `FLIGHT_STEP_S` from the hand, and the
/// floor point it came down on with its velocity there, or `None` if it never landed.
#[derive(Clone)]
pub struct Flight {
    pub color: WormholeColor,
    pub points: Vec<DVec3>,
    pub landing: Option<(DVec3, DVec3)>,
}

impl Flight {
    /// `get_duration_s` gives how long the throw is in the air.
    ///
    /// Example: `if elapsed_s >= flight.get_duration_s() { land(); }`
    pub fn get_duration_s(&self) -> f64 {
        (self.points.len() - 1) as f64 * FLIGHT_STEP_S
    }

    /// `get_position` gives where the throw is `elapsed_s` after it left the hand (its end, after).
    ///
    /// Example: `let x = flight.get_position(0.25);`
    pub fn get_position(&self, elapsed_s: f64) -> DVec3 {
        let last: usize = self.points.len() - 1;
        let step: usize = ((elapsed_s / FLIGHT_STEP_S) as usize).min(last);
        self.points[step]
    }
}

/// `fly_throw` traces a throw of the `color` mouth from the eye of `camera` (its world matrix:
/// right, up, back, position), `settings.throw_up_rad` above its crosshair at
/// `settings.throw_speed_m_per_s`, falling at `settings.gravity_m_per_s_per_s`, until `find_hit`
/// (a point, a unit direction and a length, to how far along it the first surface is) finds a
/// surface, or for `settings.max_flight_s`. From a hit it drops straight down to the floor below;
/// the floor point is the landing.
///
/// Example: `let flight = fly_throw(eye, WormholeColor::Purple, &settings.wormholes, |x, v, l| None);`
pub fn fly_throw(
    camera: DMat4,
    color: WormholeColor,
    settings: &WormholeSettings,
    find_hit: impl Fn(DVec3, DVec3, f64) -> Option<f64>,
) -> Flight {
    let up_rad: f64 = settings.throw_up_rad;
    let mut x: DVec3 = camera.w_axis.truncate();
    let mut v: DVec3 = settings.throw_speed_m_per_s
        * (camera * DVec4::new(0.0, up_rad.sin(), -up_rad.cos(), 0.0)).truncate();
    let mut flight: Flight = Flight {
        color,
        points: vec![x],
        landing: None,
    };
    let step_count: usize = (settings.max_flight_s / FLIGHT_STEP_S).ceil() as usize;
    for _ in 0..step_count {
        let speed_m_per_s: f64 = v.length();
        let direction: DVec3 = v / speed_m_per_s;
        if let Some(hit_m) = find_hit(x, direction, speed_m_per_s * FLIGHT_STEP_S) {
            let hit: DVec3 = x + direction * hit_m;
            let before: DVec3 = hit - direction * BACK_OFF_M;
            flight.landing = find_hit(before, DVec3::NEG_Y, MAX_DROP_M)
                .map(|drop_m| (before - DVec3::Y * drop_m, v));
            flight.points.push(hit);
            return flight;
        }
        x += v * FLIGHT_STEP_S;
        v.y -= settings.gravity_m_per_s_per_s * FLIGHT_STEP_S;
        flight.points.push(x);
    }
    flight
}

/// `Marker` is a glowing ball drawn for a thrown mouth: its centre, its colour (linear RGB), its
/// radius in meters, and whether it is a mouth down (which shots hit) rather than a throw in the air.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Marker {
    pub centre_m: Vec3,
    pub rgb: [f32; 3],
    pub radius_m: f32,
    pub is_mouth: bool,
}

/// `PlayerWormholes` is what everyone sees of one player's wormholes: each colour's place once it
/// is down, the throw in the air, if any (its colour and where it is now, w = 1), and how damaged
/// the pair is by enemy shots (0 to 1, destroyed at 1; the server's tally, 0 from a client).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct PlayerWormholes {
    pub purple: Option<MouthPlace>,
    pub green: Option<MouthPlace>,
    pub flying: Option<(WormholeColor, DVec4)>,
    pub damage_frac: f64,
}

impl PlayerWormholes {
    /// `get_pair` gives the purple and the green place once both are down.
    ///
    /// Example: `if let Some([purple, green]) = wormholes.get_pair() { link(purple, green); }`
    pub fn get_pair(&self) -> Option<[MouthPlace; 2]> {
        Some([self.purple?, self.green?])
    }

    /// `get_markers` gives the glowing balls to draw for these wormholes: the throw in the air, and a
    /// mouth that is down while the other isn't.
    ///
    /// Example: `let markers: Vec<Marker> = wormholes.get_markers();`
    pub fn get_markers(&self) -> Vec<Marker> {
        let marker = |centre_m: Vec3, color: WormholeColor, radius_m: f64, is_mouth: bool| Marker {
            centre_m,
            rgb: color.get_rgb(),
            radius_m: radius_m as f32,
            is_mouth,
        };
        let flying = self
            .flying
            .map(|(color, x)| marker(x.truncate().as_vec3(), color, FLYING_MARKER_M, false));
        let unlinked = self
            .get_unlinked()
            .map(|(color, place)| marker(place.get_centre_m(), color, UNLINKED_MARKER_M, true));
        flying.into_iter().chain(unlinked).collect()
    }

    /// `get_places` lists the places of the mouths that are down.
    ///
    /// Example: `let others = wormholes.get_places();`
    pub fn get_places(&self) -> Vec<MouthPlace> {
        self.purple.into_iter().chain(self.green).collect()
    }

    /// `get_unlinked` gives the place of a mouth that is down while the other isn't, with its colour.
    ///
    /// Example: `if let Some((color, place)) = wormholes.get_unlinked() { mark(color, place); }`
    pub fn get_unlinked(&self) -> Option<(WormholeColor, MouthPlace)> {
        match (self.purple, self.green) {
            (Some(place), None) => Some((WormholeColor::Purple, place)),
            (None, Some(place)) => Some((WormholeColor::Green, place)),
            _ => None,
        }
    }
}

/// `ThrownWormholes` is this player's own wormholes: what everyone sees (`placed`), the throw in
/// the air with when it left the hand (seconds into the game), and which colour the next throw is.
#[derive(Clone)]
pub struct ThrownWormholes {
    pub placed: PlayerWormholes,
    flight: Option<(Flight, f64)>,
    next_color: WormholeColor,
}

impl ThrownWormholes {
    /// `New` gives a player with neither mouth down, whose first throw is purple.
    ///
    /// Example: `let mut thrown = ThrownWormholes::New();`
    #[allow(non_snake_case)]
    pub fn New() -> ThrownWormholes {
        ThrownWormholes {
            placed: PlayerWormholes::default(),
            flight: None,
            next_color: WormholeColor::Purple,
        }
    }

    /// `get_next_color` gives the colour the next throw is.
    ///
    /// Example: `let color = thrown.get_next_color();`
    pub fn get_next_color(&self) -> WormholeColor {
        self.next_color
    }

    /// `get_kept_places` lists the places of this player's mouths that the next throw won't move.
    ///
    /// Example: `let others = thrown.get_kept_places();`
    pub fn get_kept_places(&self) -> Vec<MouthPlace> {
        match self.next_color {
            WormholeColor::Purple => self.placed.green,
            WormholeColor::Green => self.placed.purple,
        }
        .into_iter()
        .collect()
    }

    /// `can_throw` says whether this player may throw a mouth: no throw is in the air and not both
    /// mouths are down (a linked pair stays until death or destruction clears it).
    ///
    /// Example: `if thrown.can_throw() { thrown.throw(flight, now_s); }`
    pub fn can_throw(&self) -> bool {
        self.flight.is_none() && self.placed.get_pair().is_none()
    }

    /// `throw` puts `flight` in the air from `now_s`.
    ///
    /// Example: `thrown.throw(fly_throw(eye, thrown.get_next_color(), &settings, find_hit), now_s);`
    pub fn throw(&mut self, flight: Flight, now_s: f64) {
        assert!(
            self.can_throw(),
            "throw: a throw is in the air or both mouths are down"
        );
        self.flight = Some((flight, now_s));
    }

    /// `update` moves the throw in the air to `now_s`. When it comes down, the mouth of its colour
    /// moves to its place `hover_m` above the landing if `can_place` allows it there, and the next
    /// throw is the other colour; otherwise it fizzles and the next throw is the same colour. It
    /// returns a message for the HUD when the throw ends.
    ///
    /// Example: `let message = thrown.update(now_s, 1.68, |place| can_place_mouth(place));`
    pub fn update(
        &mut self,
        now_s: f64,
        hover_m: f64,
        can_place: impl Fn(&MouthPlace) -> bool,
    ) -> Option<String> {
        let (flight, thrown_s) = self.flight.as_ref()?;
        let elapsed_s: f64 = now_s - thrown_s;
        if elapsed_s < flight.get_duration_s() {
            self.placed.flying = Some((flight.color, flight.get_position(elapsed_s).extend(1.0)));
            return None;
        }
        let color: WormholeColor = flight.color;
        let place: Option<MouthPlace> = flight
            .landing
            .map(|(floor_m, velocity)| MouthPlace::Landed(floor_m, velocity, hover_m));
        (self.flight, self.placed.flying) = (None, None);
        let name: &str = color.get_name();
        match place {
            Some(place) if can_place(&place) => {
                *match color {
                    WormholeColor::Purple => &mut self.placed.purple,
                    WormholeColor::Green => &mut self.placed.green,
                } = Some(place);
                self.next_color = color.get_other();
                Some(format!("{name} wormhole placed"))
            }
            Some(_) => Some(format!(
                "{name} wormhole fizzled: too close to a wall or another warp"
            )),
            None => Some(format!("{name} wormhole fizzled: it never landed")),
        }
    }

    /// `clear` removes both mouths and any throw in the air (on death); the next throw is purple.
    ///
    /// Example: `thrown.clear();`
    pub fn clear(&mut self) {
        *self = ThrownWormholes::New();
    }
}

#[cfg(test)]
mod tests {
    //! Tests of thrown wormholes over flat ground (the plane y = 0), thrown from an eye looking
    //! along -z.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Mouth place | its centre; heading the way it flew | `a_landed_mouth_hovers_and_heads_along_its_flight` |
    //! | Throw | lands on the ground ahead; hits a wall and drops to the floor below it; flies off with nothing to hit (never lands) | `a_throw_lands_ahead_on_the_ground`, `a_throw_at_a_wall_drops_below_it`, `a_throw_that_never_lands_fizzles` |
    //! | Colours | purple then green, then no third throw; a refused place keeps the colour; death clears both | `throws_go_purple_then_green_then_stop`, `a_refused_place_fizzles`, `clearing_starts_over_with_purple` |
    //! | What others see | none down (nothing kept), a throw in the air with one mouth down (two markers, that mouth kept), both down (no markers) | `markers_show_throws_and_unlinked_mouths` |

    use super::*;
    use crate::settings::ServerSettings;

    const EYE_M: f64 = 1.68;
    const WALL_M: f64 = 5.0;
    const TOLERANCE_M: f64 = 1e-6;
    const LANDING_TOLERANCE_M: f64 = 0.01;
    const MIN_RANGE_M: f64 = 6.0;
    const SKY_UP_RAD: f64 = 1.5;
    const LATE_S: f64 = 100.0;

    /// `find_ground_hit` finds the ground plane y = 0 along a straight piece of length `length_m`.
    fn find_ground_hit(x: DVec3, v: DVec3, length_m: f64) -> Option<f64> {
        (x.y >= 0.0 && v.y < 0.0)
            .then(|| x.y / -v.y)
            .filter(|&hit_m| hit_m <= length_m)
    }

    /// `find_wall_or_ground_hit` adds a wall, the plane z = -`WALL_M`, to `find_ground_hit`.
    fn find_wall_or_ground_hit(x: DVec3, v: DVec3, length_m: f64) -> Option<f64> {
        let wall: Option<f64> = (x.z > -WALL_M && v.z < 0.0)
            .then(|| (x.z + WALL_M) / -v.z)
            .filter(|&hit_m| hit_m <= length_m);
        match (wall, find_ground_hit(x, v, length_m)) {
            (Some(w), Some(g)) => Some(w.min(g)),
            (w, g) => w.or(g),
        }
    }

    /// `get_eye` gives an eye at `EYE_M` over the origin looking along -z, pitched up by `pitch_rad`.
    fn get_eye(pitch_rad: f64) -> DMat4 {
        DMat4::from_translation(DVec3::Y * EYE_M) * DMat4::from_rotation_x(pitch_rad)
    }

    /// `throw_from_eye` throws the purple mouth from `get_eye(pitch_rad)` with the built-in settings.
    fn throw_from_eye(
        pitch_rad: f64,
        find_hit: impl Fn(DVec3, DVec3, f64) -> Option<f64>,
    ) -> Flight {
        let settings: WormholeSettings = ServerSettings::Builtin().wormholes;
        fly_throw(
            get_eye(pitch_rad),
            WormholeColor::Purple,
            &settings,
            find_hit,
        )
    }

    #[test]
    fn a_landed_mouth_hovers_and_heads_along_its_flight() {
        let place: MouthPlace = MouthPlace::Landed(
            DVec3::new(2.0, 7.0, -3.0),
            DVec3::new(1.0, -2.0, -1.0),
            EYE_M,
        );
        let centre_m: Vec3 = place.get_centre_m();
        assert!(
            (centre_m - Vec3::new(2.0, 8.68, -3.0)).length() < 1e-5,
            "{centre_m}"
        );
        assert!((place.yaw_rad - std::f64::consts::FRAC_PI_4).abs() < TOLERANCE_M);
    }

    #[test]
    fn a_throw_lands_ahead_on_the_ground() {
        let flight: Flight = throw_from_eye(0.0, find_ground_hit);
        let (x, _) = flight.landing.expect("it lands");
        assert!(x.y.abs() < LANDING_TOLERANCE_M, "{x}");
        assert!(-x.z > MIN_RANGE_M && x.x.abs() < TOLERANCE_M, "{x}");
    }

    #[test]
    fn a_throw_at_a_wall_drops_below_it() {
        let flight: Flight = throw_from_eye(0.0, find_wall_or_ground_hit);
        let (x, _) = flight.landing.expect("it lands");
        assert!(x.y.abs() < LANDING_TOLERANCE_M, "{x}");
        assert!(x.z > -WALL_M && x.z < -WALL_M + 2.0 * BACK_OFF_M, "{x}");
    }

    #[test]
    fn a_throw_that_never_lands_fizzles() {
        let flight: Flight = throw_from_eye(SKY_UP_RAD, |_, _, _| None);
        assert!(flight.landing.is_none());
        let mut thrown: ThrownWormholes = ThrownWormholes::New();
        thrown.throw(flight, 0.0);
        let message: String = thrown.update(LATE_S, EYE_M, |_| true).expect("it ends");
        assert!(message.contains("never landed"), "{message}");
        assert_eq!(thrown.placed, PlayerWormholes::default());
    }

    #[test]
    fn throws_go_purple_then_green_then_stop() {
        let mut thrown: ThrownWormholes = ThrownWormholes::New();
        let mut colors: Vec<WormholeColor> = Vec::new();
        while thrown.can_throw() {
            let color: WormholeColor = thrown.get_next_color();
            colors.push(color);
            let settings: WormholeSettings = ServerSettings::Builtin().wormholes;
            thrown.throw(
                fly_throw(get_eye(0.0), color, &settings, find_ground_hit),
                0.0,
            );
            assert!(!thrown.can_throw());
            assert!(thrown.update(0.0, EYE_M, |_| true).is_none());
            assert!(thrown.placed.flying.is_some());
            thrown.update(LATE_S, EYE_M, |_| true).expect("it lands");
        }
        use WormholeColor::{Green, Purple};
        assert_eq!(colors, [Purple, Green]);
        assert!(thrown.placed.get_pair().is_some() && thrown.placed.flying.is_none());
    }

    #[test]
    fn a_refused_place_fizzles() {
        let mut thrown: ThrownWormholes = ThrownWormholes::New();
        thrown.throw(throw_from_eye(0.0, find_ground_hit), 0.0);
        let message: String = thrown.update(LATE_S, EYE_M, |_| false).expect("it ends");
        assert!(message.contains("too close"), "{message}");
        assert_eq!(thrown.get_next_color(), WormholeColor::Purple);
        assert!(thrown.placed.purple.is_none());
    }

    #[test]
    fn markers_show_throws_and_unlinked_mouths() {
        let mut thrown: ThrownWormholes = ThrownWormholes::New();
        assert!(thrown.get_kept_places().is_empty());
        thrown.throw(throw_from_eye(0.0, find_ground_hit), 0.0);
        thrown.update(LATE_S, EYE_M, |_| true).expect("it lands");
        let purple: MouthPlace = thrown.placed.purple.expect("purple is down");
        let settings: WormholeSettings = ServerSettings::Builtin().wormholes;
        let color: WormholeColor = thrown.get_next_color();
        thrown.throw(
            fly_throw(get_eye(0.0), color, &settings, find_ground_hit),
            0.0,
        );
        thrown.update(0.0, EYE_M, |_| true);
        assert_eq!(thrown.get_kept_places(), [purple]);
        let looks: Vec<([f32; 3], bool)> = thrown
            .placed
            .get_markers()
            .iter()
            .map(|m| (m.rgb, m.is_mouth))
            .collect();
        assert_eq!(looks, [(GREEN_COLOR, false), (PURPLE_COLOR, true)]);
        thrown.update(LATE_S, EYE_M, |_| true).expect("it lands");
        assert!(thrown.placed.get_markers().is_empty());
    }

    #[test]
    fn clearing_starts_over_with_purple() {
        let mut thrown: ThrownWormholes = ThrownWormholes::New();
        thrown.throw(throw_from_eye(0.0, find_ground_hit), 0.0);
        thrown.update(LATE_S, EYE_M, |_| true).expect("it lands");
        assert!(thrown.placed.get_unlinked().is_some());
        thrown.clear();
        assert!(thrown.can_throw());
        assert_eq!(thrown.get_next_color(), WormholeColor::Purple);
        assert_eq!(thrown.placed, PlayerWormholes::default());
    }
}
