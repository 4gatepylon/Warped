//! `dust` is a copy of `../hyperbolic_wormholes_rust/src/dust.rs` in this level's flat space: it
//! throws up dust and a puff of mist where a laser shot hits the ground. A `DustBurst` starts when
//! the shot's beam front reaches the ground: its grains fly up and fall back under gravity onto
//! the ground, its mist puffs swell and drift, and all of them fade out within two seconds. Each
//! frame, `DustBurst::get_cloud` gives the grains and puffs for the session to draw as particles
//! (the grains as small turning polygons, where Lunar Surface draws tumbling polyhedra).
//!
//! Interface: `DustBurst::FromShot`, `DustBurst::get_cloud`, `DustBurst::is_over`, `DustParticle`,
//! `DustShape`, `MAX_DUST_BURST_COUNT`.
//!
//! Example: `let particles: Vec<DustParticle> = burst.get_cloud(now_s);`

use std::f64::consts::TAU;

use glam::{DMat3, DQuat, DVec3};

/// `MAX_DUST_BURST_COUNT` is the most bursts kept; it and the other constants are Lunar Surface's.
pub const MAX_DUST_BURST_COUNT: usize = 12;
/// `DUST_GRAVITY_M_PER_S2` is how fast the grains fall: Earth's, as this level's sparks fall.
const DUST_GRAVITY_M_PER_S2: f64 = 9.8;
/// `MIN_HEADING_SQ` is the least squared length of a heading along the ground.
const MIN_HEADING_SQ: f64 = 1e-6;
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
/// small polyhedron about as big as a ball of its radius, which this level draws as a polygon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DustShape {
    Puff = 0,
    Tetrahedron = 1,
    Cube = 2,
    Octahedron = 3,
}

/// `DustParticle` is one grain or puff as drawn now: its centre (in its burst's frame, and in the
/// world once `DustBurst::get_cloud` gives it), its radius, its linear albedo, its opacity at its
/// centre, its shape, and how it is turned (from its own axes to the burst's frame).
#[derive(Clone, Copy, Debug)]
pub struct DustParticle {
    pub position_m: DVec3,
    pub radius_m: f64,
    pub albedo: [f32; 3],
    pub opacity_frac: f64,
    pub shape: DustShape,
    pub orientation: DQuat,
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

/// `Impact` is where a shot hit the ground: the turn from the frame of the hit point (y up from the
/// ground, z along the ground toward the shooter) to the world, and the point.
#[derive(Clone, Copy)]
struct Impact {
    world_from_frame: DMat3,
    spot_m: DVec3,
}

/// `DustBurst` is the dust and mist of one shot into the ground, from `start_s` (seconds into the
/// match, when the beam front arrives) to `end_s`, falling at `gravity_m_per_s2`.
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

/// `make_seed` gives a burst's seed from where and when it starts, so bursts differ.
///
/// Example: `let seed = make_seed(spot_m, 12.5);`
fn make_seed(spot_m: DVec3, start_s: f64) -> u64 {
    spot_m.x.to_bits()
        ^ spot_m.z.to_bits().rotate_left(SEED_ROTATIONS[0])
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
    /// `FromShot` makes the burst of a shot fired by the rifle (`is_rifle`) or the sniper that hit
    /// the flat ground at `spot_m`, flying along `direction`, whose beam front reaches the ground
    /// at `start_s`. The burst sprays up from the ground and a little back toward the shooter.
    ///
    /// Example: `let burst = DustBurst::FromShot(spot_m, direction, 12.5, false);`
    #[allow(non_snake_case)]
    pub fn FromShot(spot_m: DVec3, direction: DVec3, start_s: f64, is_rifle: bool) -> DustBurst {
        let toward: DVec3 = DVec3::new(-direction.x, 0.0, -direction.z);
        let heading: DVec3 = [toward, DVec3::Z]
            .into_iter()
            .find(|t| t.length_squared() > MIN_HEADING_SQ)
            .expect("FromShot: z lies along the ground")
            .normalize();
        let impact: Impact = Impact {
            world_from_frame: DMat3::from_cols(DVec3::Y.cross(heading), DVec3::Y, heading),
            spot_m,
        };
        let size: &BurstSize = if is_rifle {
            &RIFLE_BURST
        } else {
            &SNIPER_BURST
        };
        DustBurst::New(
            impact,
            DUST_GRAVITY_M_PER_S2,
            start_s,
            size,
            make_seed(spot_m, start_s),
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
        let normal: DVec3 = DVec3::Y;
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

    /// `get_cloud` gives the burst's grains and puffs as drawn at `now_s`, placed in the world:
    /// none before its beam arrives or once every particle has faded.
    ///
    /// Example: `let particles = burst.get_cloud(now_s);`
    pub fn get_cloud(&self, now_s: f64) -> Vec<DustParticle> {
        if now_s < self.start_s || self.is_over(now_s) {
            return Vec::new();
        }
        let age_s: f64 = now_s - self.start_s;
        self.particles
            .iter()
            .map(|p| p.get_state(age_s, self.gravity_m_per_s2))
            .filter(|p| p.opacity_frac > 0.0)
            .map(|p| DustParticle {
                position_m: self.impact.spot_m + self.impact.world_from_frame * p.position_m,
                ..p
            })
            .collect()
    }

    /// `is_over` says whether every particle has faded by `now_s`.
    ///
    /// Example: `bursts.retain(|burst| !burst.is_over(now_s));`
    pub fn is_over(&self, now_s: f64) -> bool {
        now_s >= self.end_s
    }
}
