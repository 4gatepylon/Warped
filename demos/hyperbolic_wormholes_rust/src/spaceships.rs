//! `spaceships.rs` flies a few spaceships across Lunar Surface's sky, purely for looks: a ring
//! ship and three fighters in a V. Each crosses the sky slowly on a great circle, from horizon to
//! horizon, then comes back round below the horizon in a fifth of that time, so it is in the sky
//! most of the time. `render_voxel.metal` draws them wherever a ray escapes to the
//! sky, behind everything, so they never touch the game.
//!
//! Interface: `GpuSpaceship`, `SPACESHIP_COUNT`, `get_spaceships`, `get_utc_s`.
//!
//! Example: `let ships = get_spaceships(get_utc_s());`
//!
//! Must know:
//! - Gotchas:
//!   - Where the ships are depends only on the time given, so windows whose clocks agree (the UTC
//!     clock, set by the network) show every player the same ships.

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::time::{SystemTime, UNIX_EPOCH};

use glam::DVec3;

pub const SPACESHIP_COUNT: usize = 4;
const HIDDEN_PER_CROSSING: f64 = 0.2;
const STROBE_PERIOD_S: f64 = 1.6;
const STROBE_FLASH_FRAC: f64 = 0.08;
const STROBE_OFFSET_PER_SHIP_FRAC: f64 = 0.37;
const ENGINE_FLICKER_HZ: [f64; 2] = [5.3, 2.9];
const ENGINE_FLICKER_FRAC: f64 = 0.08;
const RING_SPIN_RAD_PER_S: f64 = 0.35;

/// `Hull` is a spaceship's shape, numbered as the shader's `HULL_*` constants.
#[derive(Clone, Copy)]
enum Hull {
    Fighter = 0,
    RingShip = 1,
}

/// `Flight` is one spaceship's path and size: its hull; the highest point of its great circle
/// (elevation above the horizon, and azimuth from +x toward +z; the ship flies a quarter turn
/// further round, toward azimuth + 90°); how long it takes from horizon to horizon; how far into
/// its first crossing it is at time 0; its half length; and its place in a formation (ahead and to
/// starboard, in ship lengths, from the point the flight gives).
struct Flight {
    hull: Hull,
    top_elevation_deg: f64,
    top_azimuth_deg: f64,
    crossing_s: f64,
    offset_s: f64,
    half_length_rad: f64,
    formation_lengths: [f64; 2],
}

const FIGHTER_FLIGHT: Flight = Flight {
    hull: Hull::Fighter,
    top_elevation_deg: 84.0,
    top_azimuth_deg: 110.0,
    crossing_s: 110.0,
    offset_s: 40.0,
    half_length_rad: 0.02,
    formation_lengths: [0.0, 0.0],
};

const FLIGHTS: [Flight; SPACESHIP_COUNT] = [
    Flight {
        hull: Hull::RingShip,
        top_elevation_deg: 74.0,
        top_azimuth_deg: 215.0,
        crossing_s: 300.0,
        offset_s: 190.0,
        half_length_rad: 0.042,
        formation_lengths: [0.0, 0.0],
    },
    FIGHTER_FLIGHT,
    Flight {
        formation_lengths: [-1.3, -1.5],
        ..FIGHTER_FLIGHT
    },
    Flight {
        formation_lengths: [-1.3, 1.5],
        ..FIGHTER_FLIGHT
    },
];

/// `GpuSpaceship` is the shader's `Spaceship`: its centre's direction in the sky (at the world
/// origin, like the sun's) with its half length in radians, its unit heading (along the sky,
/// square to the centre) with its hull number, and its lights: the strobe's brightness (0 to 1),
/// the engines' brightness and the ring's turn in radians (the fourth number is unused).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuSpaceship {
    centre: [f32; 4],
    heading: [f32; 4],
    lights: [f32; 4],
}

/// `get_utc_s` gives the UTC clock's seconds since 1970, the time every window's sky shows.
///
/// Example: `let time_s = get_utc_s();`
pub fn get_utc_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs_f64()
}

/// `get_flight_angle_rad` gives how far round its great circle `flight` is at `time_s`, from its
/// highest point: from -90° (rising) through 0 to 90° (setting) during a crossing, then on to 270°
/// below the horizon.
///
/// Example: `let angle_rad = get_flight_angle_rad(&FLIGHTS[0], 12.0);`
fn get_flight_angle_rad(flight: &Flight, time_s: f64) -> f64 {
    let hidden_s: f64 = HIDDEN_PER_CROSSING * flight.crossing_s;
    let into_s: f64 = (time_s + flight.offset_s).rem_euclid(flight.crossing_s + hidden_s);
    if into_s < flight.crossing_s {
        -FRAC_PI_2 + PI * into_s / flight.crossing_s
    } else {
        FRAC_PI_2 + PI * (into_s - flight.crossing_s) / hidden_s
    }
}

/// `get_flight_point` gives where `flight` puts a ship `angle_rad` round its great circle from the
/// highest point: its direction and its unit heading.
///
/// Example: `let (centre, heading) = get_flight_point(&FLIGHTS[0], 0.3);`
fn get_flight_point(flight: &Flight, angle_rad: f64) -> (DVec3, DVec3) {
    let (elevation_rad, azimuth_rad) = (
        flight.top_elevation_deg.to_radians(),
        flight.top_azimuth_deg.to_radians(),
    );
    let top: DVec3 = DVec3::new(
        elevation_rad.cos() * azimuth_rad.cos(),
        elevation_rad.sin(),
        elevation_rad.cos() * azimuth_rad.sin(),
    );
    let across: DVec3 = DVec3::new(-azimuth_rad.sin(), 0.0, azimuth_rad.cos());
    let (sin, cos) = angle_rad.sin_cos();
    (cos * top + sin * across, cos * across - sin * top)
}

/// `get_spaceships` gives every spaceship at `time_s` (any clock in seconds; the window uses
/// `get_utc_s`), ready for the shader.
///
/// Example: `let ships = get_spaceships(get_utc_s());`
pub fn get_spaceships(time_s: f64) -> [GpuSpaceship; SPACESHIP_COUNT] {
    let ring_rad: f64 = (RING_SPIN_RAD_PER_S * time_s).rem_euclid(TAU);
    std::array::from_fn(|i| {
        let flight: &Flight = &FLIGHTS[i];
        let (point, heading) = get_flight_point(flight, get_flight_angle_rad(flight, time_s));
        let starboard: DVec3 = heading.cross(point);
        let length_rad: f64 = 2.0 * flight.half_length_rad;
        let [ahead, aside] = flight.formation_lengths;
        let centre: DVec3 =
            (point + length_rad * (ahead * heading + aside * starboard)).normalize();
        let heading: DVec3 = heading.reject_from_normalized(centre).normalize();
        let flash_frac: f64 = (time_s / STROBE_PERIOD_S + i as f64 * STROBE_OFFSET_PER_SHIP_FRAC)
            .rem_euclid(1.0)
            / STROBE_FLASH_FRAC;
        let flicker: f64 = (TAU * ENGINE_FLICKER_HZ[0] * time_s + i as f64).sin()
            * (TAU * ENGINE_FLICKER_HZ[1] * time_s).sin();
        let lights: [f64; 4] = [
            (1.0 - flash_frac).max(0.0),
            1.0 - ENGINE_FLICKER_FRAC * (0.5 + 0.5 * flicker),
            ring_rad,
            0.0,
        ];
        GpuSpaceship {
            centre: centre
                .as_vec3()
                .extend(flight.half_length_rad as f32)
                .to_array(),
            heading: heading
                .as_vec3()
                .extend(flight.hull as i32 as f32)
                .to_array(),
            lights: lights.map(|x| x as f32),
        }
    })
}

#[cfg(test)]
mod tests {
    //! Tests of where the spaceships fly.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Time | 0, today's UTC clock | `ships_are_unit_and_square`, `the_same_time_gives_the_same_sky` |
    //! | Time between two calls | none, a tenth of a second, a whole round | `the_same_time_gives_the_same_sky`, `ships_fly_slowly_forward`, `a_flight_comes_back` |
    //! | Place on the flight | rising, highest, setting, below the horizon | `flights_rise_and_set`, `ships_fly_slowly_forward` |
    //! | Ship | alone, in formation | `ships_are_unit_and_square`, `fighters_fly_in_formation` |

    use super::*;

    const TODAY_S: f64 = 1.794e9;
    const TOLERANCE: f64 = 1e-5;
    const SMALL_DT_S: f64 = 0.1;
    const SPEED_TOLERANCE_FRAC: f64 = 0.01;
    const COME_BACK_TOLERANCE_RAD: f64 = 1e-4;
    const FORMATION_GAP_LENGTHS: std::ops::Range<f64> = 1.5..2.5;
    const PARALLEL_COS: f64 = 0.99;
    const LEAD_FIGHTER: usize = 1;

    /// `to_dvec3` reads the xyz of a shader vector.
    ///
    /// Example: `let centre = to_dvec3(ship.centre);`
    fn to_dvec3(v: [f32; 4]) -> DVec3 {
        DVec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]))
    }

    /// `get_elevation_rad` gives how high above the horizon `flight` is at `time_s`.
    ///
    /// Example: `let elevation_rad = get_elevation_rad(&FLIGHTS[0], 0.0);`
    fn get_elevation_rad(flight: &Flight, time_s: f64) -> f64 {
        get_flight_point(flight, get_flight_angle_rad(flight, time_s))
            .0
            .y
            .asin()
    }

    #[test]
    fn ships_are_unit_and_square() {
        for time_s in [0.0, TODAY_S] {
            for ship in get_spaceships(time_s) {
                let (centre, heading) = (to_dvec3(ship.centre), to_dvec3(ship.heading));
                assert!((centre.length() - 1.0).abs() < TOLERANCE, "{ship:?}");
                assert!((heading.length() - 1.0).abs() < TOLERANCE, "{ship:?}");
                assert!(centre.dot(heading).abs() < TOLERANCE, "{ship:?}");
                assert!(ship.lights.iter().all(|x| x.is_finite() && *x >= 0.0));
            }
        }
    }

    #[test]
    fn the_same_time_gives_the_same_sky() {
        assert_eq!(get_spaceships(TODAY_S), get_spaceships(TODAY_S));
    }

    #[test]
    fn ships_fly_slowly_forward() {
        for flight in &FLIGHTS {
            let hidden_s: f64 = HIDDEN_PER_CROSSING * flight.crossing_s;
            let crossing_start_s: f64 =
                TODAY_S - (TODAY_S + flight.offset_s).rem_euclid(flight.crossing_s + hidden_s);
            for (time_s, leg_s) in [
                (
                    crossing_start_s + 0.5 * flight.crossing_s,
                    flight.crossing_s,
                ),
                (
                    crossing_start_s + flight.crossing_s + 0.5 * hidden_s,
                    hidden_s,
                ),
            ] {
                let moved_rad: f64 = get_flight_angle_rad(flight, time_s + SMALL_DT_S)
                    - get_flight_angle_rad(flight, time_s);
                let expected_rad: f64 = PI * SMALL_DT_S / leg_s;
                assert!(
                    (moved_rad - expected_rad).abs() < SPEED_TOLERANCE_FRAC * expected_rad,
                    "moved {moved_rad} rad, not {expected_rad}"
                );
                let (before, heading) =
                    get_flight_point(flight, get_flight_angle_rad(flight, time_s));
                let after: DVec3 =
                    get_flight_point(flight, get_flight_angle_rad(flight, time_s + SMALL_DT_S)).0;
                assert!(
                    (after - before).dot(heading) > 0.0,
                    "the ship flies forward"
                );
            }
        }
    }

    #[test]
    fn a_flight_comes_back() {
        for (i, flight) in FLIGHTS.iter().enumerate() {
            let round_s: f64 = (1.0 + HIDDEN_PER_CROSSING) * flight.crossing_s;
            let later = get_spaceships(TODAY_S + round_s);
            let moved_rad: f64 = to_dvec3(get_spaceships(TODAY_S)[i].centre)
                .angle_between(to_dvec3(later[i].centre));
            assert!(
                moved_rad < COME_BACK_TOLERANCE_RAD,
                "ship {i} is {moved_rad} rad off"
            );
        }
    }

    #[test]
    fn flights_rise_and_set() {
        for flight in &FLIGHTS {
            let rise_s: f64 = -flight.offset_s;
            let top_rad: f64 = flight.top_elevation_deg.to_radians();
            assert!(get_elevation_rad(flight, rise_s).abs() < TOLERANCE);
            let top_s: f64 = rise_s + 0.5 * flight.crossing_s;
            assert!((get_elevation_rad(flight, top_s) - top_rad).abs() < TOLERANCE);
            let set_s: f64 = rise_s + flight.crossing_s;
            assert!(get_elevation_rad(flight, set_s - SMALL_DT_S) > 0.0);
            let hidden_s: f64 = HIDDEN_PER_CROSSING * flight.crossing_s;
            assert!(get_elevation_rad(flight, set_s + 0.5 * hidden_s) < 0.0);
        }
    }

    #[test]
    fn fighters_fly_in_formation() {
        let ships = get_spaceships(TODAY_S);
        let lead: DVec3 = to_dvec3(ships[LEAD_FIGHTER].centre);
        for wing in &ships[LEAD_FIGHTER + 1..] {
            let gap_lengths: f64 =
                lead.angle_between(to_dvec3(wing.centre)) / (2.0 * FIGHTER_FLIGHT.half_length_rad);
            assert!(
                FORMATION_GAP_LENGTHS.contains(&gap_lengths),
                "{gap_lengths}"
            );
            assert!(
                to_dvec3(wing.heading).dot(to_dvec3(ships[LEAD_FIGHTER].heading)) > PARALLEL_COS
            );
        }
        let behind: DVec3 = to_dvec3(ships[LEAD_FIGHTER + 1].centre) - lead;
        assert!(
            behind.dot(to_dvec3(ships[LEAD_FIGHTER].heading)) < 0.0,
            "wingmen trail"
        );
    }
}
