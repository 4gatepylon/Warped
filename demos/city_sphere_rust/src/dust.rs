//! `dust.rs` throws up lunar dust and a puff of mist where a laser shot hits the ground. A
//! `DustBurst` starts when the shot's beam front reaches the ground: its grains (small tumbling
//! tetrahedra, cubes and octahedra) fly up and fall back under the map's gravity onto the ground's
//! slope, its mist puffs swell and drift, and all of them fade out within two seconds. Each frame,
//! `DustBurst::get_cloud` gives the grains and puffs for `render_voxel.rs` to draw in the world,
//! lit by the sun.
//!
//! Interface: `DustBurst::FromShot`, `DustBurst::get_cloud`, `DustBurst::is_over`, `DustCloud`,
//! `DustParticle`, `DustShape`, `MAX_DUST_BURST_COUNT`.
//!
//! Example: `let clouds: Vec<DustCloud> = bursts.iter().filter_map(|b| b.get_cloud(now_s)).collect();`

use std::f64::consts::TAU;

use glam::{DMat4, DQuat, DVec3, DVec4};

use crate::geometry::{
    AXIS_Y, get_local_direction, make_boost, minkowski_dot, transport_from_centre,
};
use crate::terrain::get_fermi_coordinates;
use crate::voxel_world::{Shot, VoxelWorld, geodesic_tangent, make_ground_frame, map_klein};

pub const MAX_DUST_BURST_COUNT: usize = 12;
const SNIPER_BURST: BurstSize = BurstSize {
    grain_count: 33,
    puff_count: 7,
    size_frac: 1.0,
};
const RIFLE_BURST: BurstSize = BurstSize {
    grain_count: 12,
    puff_count: 3,
    size_frac: 0.65,
};
const BACKSPRAY_FRAC: f64 = 0.45;
const LIFT_M: f64 = 0.02;
const MIN_NORMAL_UP: f64 = 0.2;
const GRAIN_CONE_HALF_ANGLE_RAD: f64 = 0.5;
const GRAIN_SPREAD_M: f64 = 0.08;
const GRAIN_MIN_SPEED_M_PER_S: f64 = 2.0;
const GRAIN_MAX_SPEED_M_PER_S: f64 = 6.0;
const GRAIN_MIN_RADIUS_M: f64 = 0.02;
const GRAIN_MAX_RADIUS_M: f64 = 0.032;
const GRAIN_MIN_SPIN_RAD_PER_S: f64 = 4.0;
const GRAIN_MAX_SPIN_RAD_PER_S: f64 = 14.0;
const GRAIN_SHAPES: [DustShape; 3] = [
    DustShape::Tetrahedron,
    DustShape::Cube,
    DustShape::Octahedron,
];
const GRAIN_MIN_LIFE_S: f64 = 1.2;
const GRAIN_MAX_LIFE_S: f64 = 2.0;
const GRAIN_OPACITY_FRAC: f64 = 0.95;
const PUFF_CONE_HALF_ANGLE_RAD: f64 = 1.2;
const PUFF_SPREAD_M: f64 = 0.1;
const PUFF_LIFT_M: f64 = 0.1;
const PUFF_MIN_SPEED_M_PER_S: f64 = 1.5;
const PUFF_MAX_SPEED_M_PER_S: f64 = 3.5;
const PUFF_DRAG_S: f64 = 0.5;
const PUFF_GROW_S: f64 = 0.4;
const PUFF_START_RADIUS_M: f64 = 0.12;
const PUFF_MIN_RADIUS_M: f64 = 0.6;
const PUFF_MAX_RADIUS_M: f64 = 1.2;
const PUFF_MIN_LIFE_S: f64 = 0.9;
const PUFF_MAX_LIFE_S: f64 = 1.5;
const PUFF_OPACITY_FRAC: f64 = 0.45;
const FADE_START_FRAC: f64 = 0.35;
const DUST_SRGB: [f64; 3] = [0.62, 0.57, 0.50];
const MIST_SRGB: [f64; 3] = [0.74, 0.70, 0.64];
const SHADE_VARIATION_FRAC: f64 = 0.15;
const DISPLAY_GAMMA: f64 = 2.2;
const SLOPE_PROBE_M: f64 = 0.25;
const SUN_PROBE_M: f64 = 0.5;
const MIN_HEADING_SQ: f64 = 1e-6;
const SEED_ROTATIONS: [u32; 2] = [21, 42];
const SPLITMIX_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const SPLITMIX_MULTIPLIERS: [u64; 2] = [0xBF58_476D_1CE4_E5B9, 0x94D0_49BB_1331_11EB];
const SPLITMIX_SHIFTS: [u32; 3] = [30, 27, 31];
const F64_MANTISSA_BITS: u32 = 53;

/// `BurstSize` is how big a gun's bursts are: how many grains and puffs, and a scale on their
/// speeds and the puffs' size.
struct BurstSize {
    grain_count: usize,
    puff_count: usize,
    size_frac: f64,
}

/// `DustShape` is what a particle looks like: a puff of mist (a soft Gaussian ball) or a grain, a
/// small polyhedron about as big as a ball of its radius (`GRAIN_PLANES` in `render_voxel.metal`).
/// Its value is the shader's index for it (`GRAIN_PLANE_RANGES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DustShape {
    Puff = 0,
    Tetrahedron = 1,
    Cube = 2,
    Octahedron = 3,
}

/// `DustParticle` is one grain or puff as drawn now: its centre in its burst's frame (Klein
/// coordinates in meters, so within (size / R)² of true distances near the burst), its radius, its
/// linear albedo, its opacity at its centre, its shape, and how it is turned (from its own axes to
/// the burst's frame).
#[derive(Clone, Copy, Debug)]
pub struct DustParticle {
    pub position_m: DVec3,
    pub radius_m: f64,
    pub albedo: [f32; 3],
    pub opacity_frac: f64,
    pub shape: DustShape,
    pub orientation: DQuat,
}

/// `DustCloud` is a burst as drawn now: the frame of the hit point (y up), the sun's direction in
/// it with how much sunlight reaches the burst (1 sunlit, 0 in shadow), and its particles.
pub struct DustCloud {
    pub frame: DMat4,
    pub sun: DVec3,
    pub sun_frac: f64,
    pub particles: Vec<DustParticle>,
}

/// `Motion` is how a particle moves: a grain flies until it lands on the ground (`landing_s` after
/// the burst starts), and a puff slows to a stop while it swells to `final_radius_m`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Motion {
    Ballistic { landing_s: f64 },
    Drifting { final_radius_m: f64 },
}

/// `Particle` is how one grain or puff starts (position, velocity and turn in the burst's frame,
/// its spin as an axis times radians per second, and its radius or a puff's first radius) and looks
/// (albedo, opacity, shape), how it moves and how long it lasts.
#[derive(Clone, Copy, Debug)]
struct Particle {
    motion: Motion,
    start_m: DVec3,
    velocity_m_per_s: DVec3,
    start_orientation: DQuat,
    spin_rad_per_s: DVec3,
    radius_m: f64,
    life_s: f64,
    opacity_frac: f64,
    albedo: [f32; 3],
    shape: DustShape,
}

/// `Impact` is where a shot hit the ground, seen from the frame of the hit point (y up, z along the
/// ground toward the shooter): the ground's unit normal, the sun's unit direction and how much
/// sunlight reaches it.
#[derive(Clone, Copy)]
struct Impact {
    frame: DMat4,
    normal: DVec3,
    sun: DVec3,
    sun_frac: f64,
}

/// `DustBurst` is the dust and mist of one shot into the ground, from `start_s` (seconds into the
/// session, when the beam front arrives) to `end_s`, falling at `gravity_m_per_s2`.
pub struct DustBurst {
    impact: Impact,
    start_s: f64,
    end_s: f64,
    gravity_m_per_s2: f64,
    particles: Vec<Particle>,
}

/// `Random` is a splitmix64 generator, so a burst's particles depend only on its seed.
struct Random {
    state: u64,
}

impl Random {
    /// `next_frac` gives the next number, uniform in [0, 1).
    ///
    /// Example: `let u = random.next_frac();`
    fn next_frac(&mut self) -> f64 {
        self.state = self.state.wrapping_add(SPLITMIX_GAMMA);
        let mut z: u64 = self.state;
        z = (z ^ (z >> SPLITMIX_SHIFTS[0])).wrapping_mul(SPLITMIX_MULTIPLIERS[0]);
        z = (z ^ (z >> SPLITMIX_SHIFTS[1])).wrapping_mul(SPLITMIX_MULTIPLIERS[1]);
        z ^= z >> SPLITMIX_SHIFTS[2];
        (z >> (u64::BITS - F64_MANTISSA_BITS)) as f64 / (1u64 << F64_MANTISSA_BITS) as f64
    }

    /// `next_between` gives the next number, uniform in [lo, hi).
    ///
    /// Example: `let speed_m_per_s = random.next_between(1.0, 3.0);`
    fn next_between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_frac()
    }

    /// `next_in_cone` gives a unit vector within `half_angle_rad` of unit vector `axis`, spread
    /// evenly over the cone's opening.
    ///
    /// Example: `let direction = random.next_in_cone(DVec3::Y, 0.5);`
    fn next_in_cone(&mut self, axis: DVec3, half_angle_rad: f64) -> DVec3 {
        let (e1, e2) = axis.any_orthonormal_pair();
        let theta_rad: f64 = half_angle_rad * self.next_frac().sqrt();
        let phi_rad: f64 = TAU * self.next_frac();
        axis * theta_rad.cos() + (e1 * phi_rad.cos() + e2 * phi_rad.sin()) * theta_rad.sin()
    }

    /// `next_on_disk` gives a point spread evenly over the disk of `radius_m` around the origin,
    /// across unit vector `normal`.
    ///
    /// Example: `let offset_m = random.next_on_disk(DVec3::Y, 0.1);`
    fn next_on_disk(&mut self, normal: DVec3, radius_m: f64) -> DVec3 {
        let (e1, e2) = normal.any_orthonormal_pair();
        let distance_m: f64 = radius_m * self.next_frac().sqrt();
        let phi_rad: f64 = TAU * self.next_frac();
        (e1 * phi_rad.cos() + e2 * phi_rad.sin()) * distance_m
    }

    /// `next_direction` gives a unit vector spread evenly over all directions.
    ///
    /// Example: `let axis = random.next_direction();`
    fn next_direction(&mut self) -> DVec3 {
        let z: f64 = 2.0 * self.next_frac() - 1.0;
        let phi_rad: f64 = TAU * self.next_frac();
        let across: f64 = (1.0 - z * z).sqrt();
        DVec3::new(across * phi_rad.cos(), across * phi_rad.sin(), z)
    }

    /// `next_rotation` gives a rotation spread evenly over all rotations (Shoemake's method).
    ///
    /// Example: `let orientation = random.next_rotation();`
    fn next_rotation(&mut self) -> DQuat {
        let u: f64 = self.next_frac();
        let (a_rad, b_rad) = (TAU * self.next_frac(), TAU * self.next_frac());
        let (first, second) = ((1.0 - u).sqrt(), u.sqrt());
        DQuat::from_xyzw(
            first * a_rad.sin(),
            first * a_rad.cos(),
            second * b_rad.sin(),
            second * b_rad.cos(),
        )
    }

    /// `next_albedo` gives the linear albedo of an sRGB colour, lighter or darker by up to
    /// `SHADE_VARIATION_FRAC`.
    ///
    /// Example: `let albedo = random.next_albedo(DUST_SRGB);`
    fn next_albedo(&mut self, srgb: [f64; 3]) -> [f32; 3] {
        let shade: f64 = 1.0 + SHADE_VARIATION_FRAC * (2.0 * self.next_frac() - 1.0);
        srgb.map(|c| (c.powf(DISPLAY_GAMMA) * shade) as f32)
    }
}

/// `fade` gives how much of a particle's opacity is left at `age_frac` of its life: all of it until
/// `FADE_START_FRAC`, then less and less, to none at the end.
///
/// Example: `let opacity_frac = particle.opacity_frac * fade(0.5);`
fn fade(age_frac: f64) -> f64 {
    let k: f64 = ((age_frac - FADE_START_FRAC) / (1.0 - FADE_START_FRAC)).clamp(0.0, 1.0);
    (1.0 - k) * (1.0 - k)
}

/// `make_impact_frame` gives the frame at ground point `spot` with y up (away from the ground
/// plane) and z along the ground toward `toward` (a tangent there), or along some other ground
/// direction when `toward` points straight up or down.
///
/// Example: `let frame = make_impact_frame(spot, -incoming);`
fn make_impact_frame(spot: DVec4, toward: DVec4) -> DMat4 {
    let cosh_h: f64 = (1.0 + spot.y * spot.y).sqrt();
    let g: DVec4 = DVec4::new(spot.x, 0.0, spot.z, spot.w) / cosh_h;
    let heading: DVec4 = [toward, DVec4::Z, DVec4::X]
        .into_iter()
        .map(|c| {
            let flat: DVec4 = DVec4::new(c.x, 0.0, c.z, c.w);
            flat + minkowski_dot(flat, g) * g
        })
        .find(|t| minkowski_dot(*t, *t) > MIN_HEADING_SQ)
        .expect("make_impact_frame: z or x lies along the ground at every ground point");
    make_ground_frame(g, heading) * make_boost(AXIS_Y, spot.y.asinh())
}

/// `find_ground_normal` gives the ground's unit normal at the origin of `frame` (a hit point with
/// y up) in that frame, from the terrain's slope `SLOPE_PROBE_M` either side, or straight up on a
/// map without terrain.
///
/// Example: `let normal = find_ground_normal(&world, frame);`
fn find_ground_normal(world: &VoxelWorld, frame: DMat4) -> DVec3 {
    let Some(terrain) = &world.terrain else {
        return DVec3::Y;
    };
    let get_height_m = |offset_m: DVec3| -> f64 {
        let x: DVec4 = frame * map_klein(offset_m / world.r);
        let [u_m, v_m, _] = get_fermi_coordinates(x, world.r);
        terrain.get_height_m(u_m, v_m)
    };
    let get_slope = |axis: DVec3| -> f64 {
        (get_height_m(axis * SLOPE_PROBE_M) - get_height_m(-axis * SLOPE_PROBE_M))
            / (2.0 * SLOPE_PROBE_M)
    };
    DVec3::new(-get_slope(DVec3::X), 1.0, -get_slope(DVec3::Z)).normalize()
}

/// `make_seed` gives a burst's seed from where and when it starts, so bursts differ.
///
/// Example: `let seed = make_seed(spot, 12.5);`
fn make_seed(spot: DVec4, start_s: f64) -> u64 {
    spot.x.to_bits()
        ^ spot.z.to_bits().rotate_left(SEED_ROTATIONS[0])
        ^ start_s.to_bits().rotate_left(SEED_ROTATIONS[1])
}

impl Particle {
    /// `MakeGrain` makes a dust grain, a polyhedron of `GRAIN_SHAPES` turned and spinning at
    /// random, thrown from near the hit point (`impact`'s ground normal) up a cone around `axis`,
    /// which lands on the ground plane under `gravity_m_per_s2`.
    ///
    /// Example: `let grain = Particle::MakeGrain(&mut random, &impact, axis, 6.0, &SNIPER_BURST);`
    #[allow(non_snake_case)]
    fn MakeGrain(
        random: &mut Random,
        normal: DVec3,
        axis: DVec3,
        gravity_m_per_s2: f64,
        size: &BurstSize,
    ) -> Particle {
        let direction: DVec3 = random.next_in_cone(axis, GRAIN_CONE_HALF_ANGLE_RAD);
        let speed_m_per_s: f64 =
            size.size_frac * random.next_between(GRAIN_MIN_SPEED_M_PER_S, GRAIN_MAX_SPEED_M_PER_S);
        let start_m: DVec3 = normal * LIFT_M + random.next_on_disk(normal, GRAIN_SPREAD_M);
        let velocity_m_per_s: DVec3 = direction * speed_m_per_s;
        let fall_m_per_s2: f64 = gravity_m_per_s2 * normal.y.max(MIN_NORMAL_UP);
        let (height_m, rise_m_per_s) = (normal.dot(start_m), normal.dot(velocity_m_per_s));
        let landing_s: f64 = (rise_m_per_s
            + (rise_m_per_s * rise_m_per_s + 2.0 * fall_m_per_s2 * height_m).sqrt())
            / fall_m_per_s2;
        Particle {
            motion: Motion::Ballistic { landing_s },
            start_m,
            velocity_m_per_s,
            start_orientation: random.next_rotation(),
            spin_rad_per_s: random.next_direction()
                * random.next_between(GRAIN_MIN_SPIN_RAD_PER_S, GRAIN_MAX_SPIN_RAD_PER_S),
            radius_m: random.next_between(GRAIN_MIN_RADIUS_M, GRAIN_MAX_RADIUS_M),
            life_s: random.next_between(GRAIN_MIN_LIFE_S, GRAIN_MAX_LIFE_S),
            opacity_frac: GRAIN_OPACITY_FRAC,
            albedo: random.next_albedo(DUST_SRGB),
            shape: GRAIN_SHAPES[(random.next_frac() * GRAIN_SHAPES.len() as f64) as usize],
        }
    }

    /// `MakePuff` makes a puff of mist that drifts out from just above the hit point along a cone
    /// around `axis`, slowing, while it swells.
    ///
    /// Example: `let puff = Particle::MakePuff(&mut random, DVec3::Y, axis, &SNIPER_BURST);`
    #[allow(non_snake_case)]
    fn MakePuff(random: &mut Random, normal: DVec3, axis: DVec3, size: &BurstSize) -> Particle {
        let direction: DVec3 = random.next_in_cone(axis, PUFF_CONE_HALF_ANGLE_RAD);
        let speed_m_per_s: f64 =
            size.size_frac * random.next_between(PUFF_MIN_SPEED_M_PER_S, PUFF_MAX_SPEED_M_PER_S);
        let final_radius_m: f64 =
            size.size_frac * random.next_between(PUFF_MIN_RADIUS_M, PUFF_MAX_RADIUS_M);
        Particle {
            motion: Motion::Drifting { final_radius_m },
            start_m: normal * PUFF_LIFT_M + random.next_on_disk(normal, PUFF_SPREAD_M),
            velocity_m_per_s: direction * speed_m_per_s,
            start_orientation: DQuat::IDENTITY,
            spin_rad_per_s: DVec3::ZERO,
            radius_m: PUFF_START_RADIUS_M,
            life_s: random.next_between(PUFF_MIN_LIFE_S, PUFF_MAX_LIFE_S),
            opacity_frac: PUFF_OPACITY_FRAC,
            albedo: random.next_albedo(MIST_SRGB),
            shape: DustShape::Puff,
        }
    }

    /// `get_state` gives the particle `age_s` after its burst started, falling at
    /// `gravity_m_per_s2` along -y: a grain on its arc, tumbling, or where it landed, still; a
    /// puff where it has drifted, at its size now; with what is left of its opacity.
    ///
    /// Example: `let drawn = particle.get_state(0.4, 6.0);`
    fn get_state(&self, age_s: f64, gravity_m_per_s2: f64) -> DustParticle {
        let (position_m, radius_m, flight_s) = match self.motion {
            Motion::Ballistic { landing_s } => {
                let t_s: f64 = age_s.min(landing_s);
                (
                    self.start_m + self.velocity_m_per_s * t_s
                        - DVec3::Y * (0.5 * gravity_m_per_s2 * t_s * t_s),
                    self.radius_m,
                    t_s,
                )
            }
            Motion::Drifting { final_radius_m } => (
                self.start_m
                    + self.velocity_m_per_s * (PUFF_DRAG_S * (1.0 - (-age_s / PUFF_DRAG_S).exp())),
                final_radius_m + (self.radius_m - final_radius_m) * (-age_s / PUFF_GROW_S).exp(),
                0.0,
            ),
        };
        DustParticle {
            position_m,
            radius_m,
            albedo: self.albedo,
            opacity_frac: self.opacity_frac * fade(age_s / self.life_s),
            shape: self.shape,
            orientation: DQuat::from_scaled_axis(self.spin_rad_per_s * flight_s)
                * self.start_orientation,
        }
    }
}

impl DustBurst {
    /// `FromShot` makes the burst of a shot fired by the rifle (`is_rifle`) or the sniper whose
    /// beam front reaches the ground at `start_s`, or `None` when the shot hit nothing, a block or
    /// a player. The burst sprays up the ground's slope and a little back toward the shooter,
    /// falls by the map's gravity (not the faster fall the server settings give players, D81),
    /// and is lit unless something stands between it and the sun.
    ///
    /// Example: `let burst = DustBurst::FromShot(&world, &shot, 12.5, false);`
    #[allow(non_snake_case)]
    pub fn FromShot(
        world: &VoxelWorld,
        shot: &Shot,
        start_s: f64,
        is_rifle: bool,
    ) -> Option<DustBurst> {
        if shot.hit_structure.is_some() {
            return None;
        }
        let spot: DVec4 = shot.spot?;
        let last = shot.segments.last()?;
        let frame: DMat4 = make_impact_frame(spot, -geodesic_tangent(last.a, last.b, last.length));
        let normal: DVec3 = find_ground_normal(world, frame);
        let sun: DVec3 =
            get_local_direction(frame, transport_from_centre(world.sun, spot)).normalize();
        let probe: DVec4 = frame * map_klein(normal * SUN_PROBE_M / world.r);
        let impact: Impact = Impact {
            frame,
            normal,
            sun,
            sun_frac: if world.is_sunlit(probe) { 1.0 } else { 0.0 },
        };
        let size: &BurstSize = if is_rifle {
            &RIFLE_BURST
        } else {
            &SNIPER_BURST
        };
        Some(DustBurst::New(
            impact,
            world.get_map_gravity_m_per_s2(),
            start_s,
            size,
            make_seed(spot, start_s),
        ))
    }

    /// `FromFloor` makes the burst of a shot fired by the rifle (`is_rifle`) or the sniper into a
    /// flat floor in a Euclidean level (City Sphere): `frame` is the hit point's rigid frame (y up
    /// from the floor, z along it toward the shooter, the hit point its last column), lit from
    /// straight above, starting at `start_s` and falling at the map's `gravity_m_per_s2`.
    ///
    /// Example: `let burst = DustBurst::FromFloor(frame, 12.5, 6.0, false);`
    #[allow(non_snake_case)]
    pub fn FromFloor(
        frame: DMat4,
        start_s: f64,
        gravity_m_per_s2: f64,
        is_rifle: bool,
    ) -> DustBurst {
        let impact: Impact = Impact {
            frame,
            normal: DVec3::Y,
            sun: DVec3::Y,
            sun_frac: 1.0,
        };
        let size: &BurstSize = if is_rifle {
            &RIFLE_BURST
        } else {
            &SNIPER_BURST
        };
        DustBurst::New(
            impact,
            gravity_m_per_s2,
            start_s,
            size,
            make_seed(frame.col(3), start_s),
        )
    }

    /// `New` makes a burst of `size` at `impact`, starting at `start_s` and falling at
    /// `gravity_m_per_s2`, its particles drawn from `seed`: grains then puffs, thrown around the
    /// ground's normal tilted `BACKSPRAY_FRAC` toward the shooter (+z).
    ///
    /// Example: `let burst = DustBurst::New(impact, 6.0, 0.0, &SNIPER_BURST, 7);`
    #[allow(non_snake_case)]
    fn New(
        impact: Impact,
        gravity_m_per_s2: f64,
        start_s: f64,
        size: &BurstSize,
        seed: u64,
    ) -> DustBurst {
        let normal: DVec3 = impact.normal;
        let back: DVec3 = (DVec3::Z - normal * normal.z).normalize_or_zero();
        let axis: DVec3 = (normal + back * BACKSPRAY_FRAC).normalize();
        let mut random: Random = Random { state: seed };
        let mut particles: Vec<Particle> = (0..size.grain_count)
            .map(|_| Particle::MakeGrain(&mut random, normal, axis, gravity_m_per_s2, size))
            .collect();
        particles.extend(
            (0..size.puff_count).map(|_| Particle::MakePuff(&mut random, normal, axis, size)),
        );
        let life_s: f64 = particles.iter().map(|p| p.life_s).fold(0.0, f64::max);
        DustBurst {
            impact,
            start_s,
            end_s: start_s + life_s,
            gravity_m_per_s2,
            particles,
        }
    }

    /// `get_cloud` gives the burst as drawn at `now_s`, or `None` before its beam arrives or once
    /// every particle has faded.
    ///
    /// Example: `let cloud = burst.get_cloud(now_s);`
    pub fn get_cloud(&self, now_s: f64) -> Option<DustCloud> {
        if now_s < self.start_s || self.is_over(now_s) {
            return None;
        }
        let age_s: f64 = now_s - self.start_s;
        Some(DustCloud {
            frame: self.impact.frame,
            sun: self.impact.sun,
            sun_frac: self.impact.sun_frac,
            particles: self
                .particles
                .iter()
                .map(|p| p.get_state(age_s, self.gravity_m_per_s2))
                .filter(|p| p.opacity_frac > 0.0)
                .collect(),
        })
    }

    /// `is_over` says whether every particle has faded by `now_s`.
    ///
    /// Example: `bursts.retain(|burst| !burst.is_over(now_s));`
    pub fn is_over(&self, now_s: f64) -> bool {
        now_s >= self.end_s
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the dust thrown up by shots into the ground.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Time since the beam arrived | before it, grains in the air, grains landed, every particle faded | `nothing_shows_before_the_beam_arrives`, `grains_rise_then_land_and_stay`, `the_burst_fades_and_ends` |
    //! | Ground at the hit | flat, sloped | `grains_never_sink_below_the_ground` |
    //! | Particle kind | grain, puff | `grains_rise_then_land_and_stay`, `puffs_swell_and_slow` |
    //! | Grain's turn | in flight (tumbling), landed (still) | `grains_are_tumbling_polyhedra` |
    //! | Gun | sniper, rifle | `rifle_bursts_are_smaller` |
    //! | Seed | the same, another | `bursts_depend_only_on_their_seed` |
    //! | What the shot hit | nothing, a block, the flat ground, the crater's wall | `only_shots_into_the_ground_raise_dust` |

    use super::*;
    use crate::geometry::AXIS_W;
    use crate::map::{Map, get_map_folder};
    use crate::voxel_world::VoxelPlayer;

    const GRAVITY_M_PER_S2: f64 = 6.0;
    const START_S: f64 = 10.0;
    const SEED: u64 = 7;
    const OTHER_SEED: u64 = 8;
    const EARLY_S: f64 = 0.01;
    const STEP_S: f64 = 0.02;
    const SETTLED_S: f64 = 0.3;
    const MIN_PEAK_M: f64 = 1.0;
    const MIN_LIFE_S: f64 = 1.0;
    const MAX_LIFE_S: f64 = 2.0;
    const NEAR_SPOT_M: f64 = 0.2;
    const TOLERANCE_M: f64 = 1e-9;
    const SLOPED_NORMAL: DVec3 = DVec3::new(0.5, 1.0, 0.2);
    const SPOT_TOLERANCE: f64 = 1e-6;
    const FLAT_NORMAL_MIN_Y: f64 = 0.999;
    const WALL_NORMAL_MAX_Y: f64 = 0.99;
    const SKY_PITCH_RAD: f64 = 1.2;
    const FLOOR_PITCH_RAD: f64 = -0.4;
    const MID_LIFE_FRAC: f64 = 0.5;
    const LATE_LIFE_FRAC: f64 = 0.9;
    const PUFF_EARLY_S: f64 = 0.3;
    const PUFF_LATE_S: f64 = 1.0;

    /// `make_burst` makes a sniper or rifle burst at the origin on ground with `normal`, from `seed`.
    fn make_burst(normal: DVec3, size: &BurstSize, seed: u64) -> DustBurst {
        let impact: Impact = Impact {
            frame: DMat4::IDENTITY,
            normal: normal.normalize(),
            sun: DVec3::Y,
            sun_frac: 1.0,
        };
        DustBurst::New(impact, GRAVITY_M_PER_S2, START_S, size, seed)
    }

    /// `get_ages_s` gives ages from 0 to past the burst's end, `STEP_S` apart.
    fn get_ages_s(burst: &DustBurst) -> Vec<f64> {
        let step_count: usize = ((burst.end_s - burst.start_s) / STEP_S).ceil() as usize + 1;
        (0..=step_count).map(|k| k as f64 * STEP_S).collect()
    }

    /// `nothing_shows_before_the_beam_arrives` checks that a burst starts at its start, at the hit.
    #[test]
    fn nothing_shows_before_the_beam_arrives() {
        let burst: DustBurst = make_burst(DVec3::Y, &SNIPER_BURST, SEED);
        assert!(burst.get_cloud(START_S - EARLY_S).is_none());
        assert!(!burst.is_over(START_S - EARLY_S));
        let cloud: DustCloud = burst
            .get_cloud(START_S)
            .expect("the burst shows at its start");
        assert!(
            cloud
                .particles
                .iter()
                .all(|p| p.position_m.length() < NEAR_SPOT_M)
        );
    }

    /// `grains_rise_then_land_and_stay` checks that grains fly up (some over a meter), come down
    /// onto the ground and then stay where they landed.
    #[test]
    fn grains_rise_then_land_and_stay() {
        let burst: DustBurst = make_burst(DVec3::Y, &SNIPER_BURST, SEED);
        let mut peak_m: f64 = 0.0;
        for grain in &burst.particles {
            let Motion::Ballistic { landing_s } = grain.motion else {
                continue;
            };
            let get_position_m = |t_s: f64| grain.get_state(t_s, GRAVITY_M_PER_S2).position_m;
            let apex_s: f64 = grain.velocity_m_per_s.y / GRAVITY_M_PER_S2;
            assert!(get_position_m(apex_s).y > get_position_m(0.0).y);
            assert!(get_position_m(landing_s).y.abs() < TOLERANCE_M);
            let settled_m: DVec3 =
                get_position_m(landing_s + SETTLED_S) - get_position_m(landing_s);
            assert!(settled_m.length() < TOLERANCE_M);
            peak_m = peak_m.max(get_position_m(apex_s).y);
        }
        assert!(peak_m > MIN_PEAK_M, "the highest grain rises {peak_m} m");
    }

    /// `the_burst_fades_and_ends` checks that particles start fully opaque, fade, and that the
    /// burst is over within one to two seconds.
    #[test]
    fn the_burst_fades_and_ends() {
        let burst: DustBurst = make_burst(DVec3::Y, &SNIPER_BURST, SEED);
        let life_s: f64 = burst.end_s - burst.start_s;
        assert!(
            (MIN_LIFE_S..=MAX_LIFE_S).contains(&life_s),
            "it lasts {life_s} s"
        );
        let get_opacity = |age_s: f64| -> f64 {
            burst
                .get_cloud(START_S + age_s)
                .map_or(0.0, |c| c.particles.iter().map(|p| p.opacity_frac).sum())
        };
        let full_opacity: f64 = burst.particles.iter().map(|p| p.opacity_frac).sum();
        assert!((get_opacity(0.0) - full_opacity).abs() < TOLERANCE_M);
        assert!(get_opacity(MID_LIFE_FRAC * life_s) < full_opacity);
        assert!(get_opacity(LATE_LIFE_FRAC * life_s) < get_opacity(MID_LIFE_FRAC * life_s));
        assert!(burst.is_over(burst.end_s));
        assert!(burst.get_cloud(burst.end_s).is_none());
    }

    /// `grains_never_sink_below_the_ground` checks that grains stay above the ground's plane on
    /// flat and on sloped ground.
    #[test]
    fn grains_never_sink_below_the_ground() {
        for normal in [DVec3::Y, SLOPED_NORMAL.normalize()] {
            let burst: DustBurst = make_burst(normal, &SNIPER_BURST, SEED);
            for age_s in get_ages_s(&burst) {
                for grain in &burst.particles {
                    let position_m: DVec3 = grain.get_state(age_s, GRAVITY_M_PER_S2).position_m;
                    assert!(normal.dot(position_m) > -TOLERANCE_M);
                }
            }
        }
    }

    /// `grains_are_tumbling_polyhedra` checks that a sniper burst's grains include each polyhedron,
    /// turn while they fly and stop turning once they land, and that puffs stay puffs.
    #[test]
    fn grains_are_tumbling_polyhedra() {
        let burst: DustBurst = make_burst(DVec3::Y, &SNIPER_BURST, SEED);
        for shape in GRAIN_SHAPES {
            assert!(burst.particles.iter().any(|p| p.shape == shape));
        }
        for particle in &burst.particles {
            let get_turn = |t_s: f64| particle.get_state(t_s, GRAVITY_M_PER_S2).orientation;
            match particle.motion {
                Motion::Ballistic { landing_s } => {
                    assert!(GRAIN_SHAPES.contains(&particle.shape));
                    let flying_rad: f64 = get_turn(0.0).angle_between(get_turn(STEP_S));
                    assert!(flying_rad > 0.0);
                    assert_eq!(get_turn(landing_s), get_turn(landing_s + SETTLED_S));
                    assert!(get_turn(STEP_S).is_normalized());
                }
                Motion::Drifting { .. } => assert_eq!(particle.shape, DustShape::Puff),
            }
        }
    }

    /// `puffs_swell_and_slow` checks that mist puffs grow from their first radius and slow down.
    #[test]
    fn puffs_swell_and_slow() {
        let burst: DustBurst = make_burst(DVec3::Y, &SNIPER_BURST, SEED);
        let puffs: Vec<&Particle> = burst
            .particles
            .iter()
            .filter(|p| matches!(p.motion, Motion::Drifting { .. }))
            .collect();
        assert_eq!(puffs.len(), SNIPER_BURST.puff_count);
        for puff in puffs {
            let get_state = |t_s: f64| puff.get_state(t_s, GRAVITY_M_PER_S2);
            assert!((get_state(0.0).radius_m - PUFF_START_RADIUS_M).abs() < TOLERANCE_M);
            assert!(get_state(PUFF_EARLY_S).radius_m > get_state(0.0).radius_m);
            assert!(get_state(PUFF_LATE_S).radius_m > get_state(PUFF_EARLY_S).radius_m);
            let early_m: f64 = (get_state(STEP_S).position_m - get_state(0.0).position_m).length();
            let late_m: f64 = (get_state(PUFF_LATE_S + STEP_S).position_m
                - get_state(PUFF_LATE_S).position_m)
                .length();
            assert!(late_m < early_m);
        }
    }

    /// `rifle_bursts_are_smaller` checks that a rifle's burst has fewer particles and smaller puffs.
    #[test]
    fn rifle_bursts_are_smaller() {
        let sniper: DustBurst = make_burst(DVec3::Y, &SNIPER_BURST, SEED);
        let rifle: DustBurst = make_burst(DVec3::Y, &RIFLE_BURST, SEED);
        assert!(rifle.particles.len() < sniper.particles.len());
        let get_largest_m = |burst: &DustBurst| -> f64 {
            burst
                .particles
                .iter()
                .map(|p| match p.motion {
                    Motion::Drifting { final_radius_m } => final_radius_m,
                    Motion::Ballistic { .. } => 0.0,
                })
                .fold(0.0, f64::max)
        };
        assert!(get_largest_m(&rifle) < get_largest_m(&sniper));
    }

    /// `bursts_depend_only_on_their_seed` checks that the same seed gives the same particles and
    /// another seed different ones.
    #[test]
    fn bursts_depend_only_on_their_seed() {
        let get_positions_m = |seed: u64| -> Vec<DVec3> {
            make_burst(DVec3::Y, &SNIPER_BURST, seed)
                .get_cloud(START_S + MID_LIFE_FRAC)
                .expect("the burst shows")
                .particles
                .iter()
                .map(|p| p.position_m)
                .collect()
        };
        assert_eq!(get_positions_m(SEED), get_positions_m(SEED));
        assert_ne!(get_positions_m(SEED), get_positions_m(OTHER_SEED));
    }

    /// `only_shots_into_the_ground_raise_dust` checks Lunar Surface shots: into the sky and into the
    /// red pylon raise none; into the flat crater floor and into the crater's wall raise a burst at
    /// the hit, sloped on the wall.
    #[test]
    fn only_shots_into_the_ground_raise_dust() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("Lunar Surface builds");
        let shoot = |at_m: [f64; 2], target_m: [f64; 2], pitch_rad: f64| -> Shot {
            let player: VoxelPlayer = VoxelPlayer::AtView(&world, at_m, target_m, None, pitch_rad);
            world.trace_laser(player.get_camera_frame(&world))
        };
        let sky: Shot = shoot([-27.0, 0.0], [40.0, 0.0], SKY_PITCH_RAD);
        assert!(!sky.is_hit);
        assert!(DustBurst::FromShot(&world, &sky, START_S, false).is_none());
        let pylon: Shot = shoot([-22.0, 40.5], [-14.0, 40.05], -0.004);
        assert!(pylon.hit_structure.is_some());
        assert!(DustBurst::FromShot(&world, &pylon, START_S, false).is_none());
        for (at_m, target_m, pitch_rad, is_wall) in [
            ([-27.0, 0.0], [40.0, 0.0], FLOOR_PITCH_RAD, false),
            ([-30.0, -30.0], [-60.0, -90.0], 0.0, true),
        ] {
            let shot: Shot = shoot(at_m, target_m, pitch_rad);
            assert!(shot.is_hit && shot.hit_structure.is_none());
            let burst: DustBurst =
                DustBurst::FromShot(&world, &shot, START_S, false).expect("the ground raises dust");
            let spot: DVec4 = shot.spot.expect("a hit has a spot");
            assert!((burst.impact.frame.col(AXIS_W) - spot).length() < SPOT_TOLERANCE);
            if is_wall {
                assert!(burst.impact.normal.y < WALL_NORMAL_MAX_Y);
            } else {
                assert!(burst.impact.normal.y > FLAT_NORMAL_MIN_Y);
            }
        }
    }
}
