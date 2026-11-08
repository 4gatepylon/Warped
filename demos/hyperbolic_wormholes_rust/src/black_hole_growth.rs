//! `black_hole_growth.rs` sets the black hole's strength (W-68) from the game's consoles: each
//! console turned off grows the black hole, up to full strength with one console left. The strength
//! eases to each new target over the settings' `black_hole.transition_s`, and a forced strength
//! (`--black-hole-strength`) overrides it.
//!
//! Interface: `get_target_strength_frac`, `BlackHoleGrowth` (`New`, `update`, `strength_frac`).
//!
//! Example: `black_hole_growth.update(&mut world, game.off_mask, now_s);`

use crate::voxel_world::VoxelWorld;

/// `get_target_strength_frac` gives the strength the consoles in `off_mask` (bit i for console i)
/// call for, out of `console_count`: the share of consoles off, reaching 1 with one console left.
///
/// Example: `let target_frac = get_target_strength_frac(0b11, 6);`
pub fn get_target_strength_frac(off_mask: u32, console_count: usize) -> f64 {
    assert!(
        console_count >= 2 && off_mask.checked_shr(console_count as u32).unwrap_or(0) == 0,
        "get_target_strength_frac: mask {off_mask:#b} for {console_count} consoles, expected at least 2 consoles and bits only for them"
    );
    (f64::from(off_mask.count_ones()) / (console_count - 1) as f64).min(1.0)
}

/// `BlackHoleGrowth` is the black hole's strength over time: the strength it eases from, the target it eases
/// to and when that target was set (seconds on the caller's clock), the strength now
/// (`strength_frac`), and a forced strength that replaces the eased one, if any.
pub struct BlackHoleGrowth {
    forced_frac: Option<f64>,
    from_frac: f64,
    target_frac: f64,
    target_set_s: f64,
    pub strength_frac: f64,
}

impl BlackHoleGrowth {
    /// `New` gives the map's own strength (0), or always `forced_frac` (0 to 1) when given.
    ///
    /// Example: `let black_hole_growth = BlackHoleGrowth::New(arguments.black_hole_strength);`
    #[allow(non_snake_case)]
    pub fn New(forced_frac: Option<f64>) -> BlackHoleGrowth {
        BlackHoleGrowth {
            forced_frac,
            from_frac: 0.0,
            target_frac: 0.0,
            target_set_s: 0.0,
            strength_frac: 0.0,
        }
    }

    /// `get_eased_frac` gives the strength at `now_s`: from `from_frac` to `target_frac` along a
    /// smoothstep over `transition_s`.
    ///
    /// Example: `let strength_frac = self.get_eased_frac(12.0, 4.0);`
    fn get_eased_frac(&self, now_s: f64, transition_s: f64) -> f64 {
        let t: f64 = ((now_s - self.target_set_s) / transition_s).clamp(0.0, 1.0);
        self.from_frac + (self.target_frac - self.from_frac) * t * t * (3.0 - 2.0 * t)
    }

    /// `update` sets the strength at `now_s` for the consoles in `off_mask` and grows `world`'s
    /// black hole to it (`VoxelWorld::grow_black_hole`); a new target starts easing from the
    /// strength now. A world whose black hole doesn't grow stays as it is.
    ///
    /// Example: `black_hole_growth.update(&mut world, game.off_mask, now_s);`
    pub fn update(&mut self, world: &mut VoxelWorld, off_mask: u32, now_s: f64) {
        if !world.can_grow_black_hole() {
            return;
        }
        let transition_s: f64 = world.settings.black_hole.transition_s;
        let target_frac: f64 = get_target_strength_frac(off_mask, world.consoles.len());
        if target_frac != self.target_frac {
            self.from_frac = self.get_eased_frac(now_s, transition_s);
            (self.target_frac, self.target_set_s) = (target_frac, now_s);
        }
        self.strength_frac = self
            .forced_frac
            .unwrap_or_else(|| self.get_eased_frac(now_s, transition_s));
        world.grow_black_hole(self.strength_frac);
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the black hole's strength schedule.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Consoles off | none, some, all but one, all | `the_target_counts_consoles_off` |
    //! | Time since the target changed | at once, during the transition (rising), after it | `the_strength_eases_without_jumps` |
    //! | Target change | up (consoles off), down to 0 (a new game), mid-transition | `the_strength_eases_without_jumps` |
    //! | Map | grows (Lunar Surface), doesn't (no consoles) | `the_strength_eases_without_jumps`, `a_map_without_consoles_never_grows` |
    //! | Forced strength | none, given | `the_strength_eases_without_jumps`, `a_forced_strength_wins` |

    use super::*;
    use crate::map::{Map, get_map_folder};
    use crate::voxel_world::VoxelWarp;

    const CONSOLE_COUNT: usize = 6;
    const TRANSITION_S: f64 = 4.0;
    const SAMPLE_COUNT: usize = 40;
    const MAX_STEP_FRAC: f64 = 0.06;
    const TOLERANCE: f64 = 1e-12;

    fn load_lunar_surface() -> VoxelWorld {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        VoxelWorld::FromMap(&map).expect("a voxel world")
    }

    #[test]
    fn the_target_counts_consoles_off() {
        assert_eq!(get_target_strength_frac(0, CONSOLE_COUNT), 0.0);
        assert!((get_target_strength_frac(0b10_0010, CONSOLE_COUNT) - 0.4).abs() < TOLERANCE);
        assert_eq!(get_target_strength_frac(0b01_1111, CONSOLE_COUNT), 1.0);
        assert_eq!(get_target_strength_frac(0b11_1111, CONSOLE_COUNT), 1.0);
    }

    #[test]
    fn the_strength_eases_without_jumps() {
        let mut world: VoxelWorld = load_lunar_surface();
        assert_eq!(world.settings.black_hole.transition_s, TRANSITION_S);
        let console_count: usize = world.consoles.len();
        let one_off_frac: f64 = 1.0 / (console_count - 1) as f64;
        let map_hole_m: f64 = world.warps[0].m;
        let mut growth: BlackHoleGrowth = BlackHoleGrowth::New(None);
        growth.update(&mut world, 0b1, 0.0);
        assert_eq!(growth.strength_frac, 0.0);
        let mut last_frac: f64 = 0.0;
        for i in 1..=SAMPLE_COUNT {
            growth.update(
                &mut world,
                0b1,
                TRANSITION_S * i as f64 / SAMPLE_COUNT as f64,
            );
            assert!(growth.strength_frac >= last_frac);
            assert!(growth.strength_frac - last_frac < MAX_STEP_FRAC * one_off_frac);
            last_frac = growth.strength_frac;
        }
        assert!((last_frac - one_off_frac).abs() < TOLERANCE);
        growth.update(&mut world, 0b11, 2.0 * TRANSITION_S);
        growth.update(&mut world, 0, 2.5 * TRANSITION_S);
        let turned_frac: f64 = growth.strength_frac;
        assert!(turned_frac > one_off_frac && turned_frac < 2.0 * one_off_frac);
        growth.update(
            &mut world,
            0,
            2.5 * TRANSITION_S + TRANSITION_S / SAMPLE_COUNT as f64,
        );
        assert!(
            growth.strength_frac < turned_frac
                && turned_frac - growth.strength_frac < MAX_STEP_FRAC
        );
        growth.update(&mut world, 0, 4.0 * TRANSITION_S);
        assert_eq!(growth.strength_frac, 0.0);
        assert_eq!(world.warps[0].m, map_hole_m);
    }

    #[test]
    fn a_map_without_consoles_never_grows() {
        let mut map: Map =
            Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        map.materials.retain(|m| !m.name.contains(" console "));
        let mut world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        assert!(!world.can_grow_black_hole());
        let map_hole_m: f64 = world.warps[0].m;
        let mut growth: BlackHoleGrowth = BlackHoleGrowth::New(None);
        growth.update(&mut world, 0, TRANSITION_S);
        assert_eq!(growth.strength_frac, 0.0);
        assert_eq!(world.warps[0].m, map_hole_m);
    }

    #[test]
    fn a_forced_strength_wins() {
        let mut world: VoxelWorld = load_lunar_surface();
        let map_hole: VoxelWarp = world.warps[0].clone();
        let mut growth: BlackHoleGrowth = BlackHoleGrowth::New(Some(0.6));
        growth.update(&mut world, 0, 0.0);
        assert_eq!(growth.strength_frac, 0.6);
        assert!(world.warps[0].m > map_hole.m);
    }
}
