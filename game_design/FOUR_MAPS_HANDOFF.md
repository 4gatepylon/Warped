# Four map demos and the match: handoff

State on 2026-11-08, when the four level demos, the match and their side PRs were merged into
`staging/user-4gatepylon`. Read this first if you continue this work; the details live in each folder's
`README.md` and `DECISIONS.md`, and the rules in `INSIDE_GAME_DESIGN.md`.

## What is here

| Level | Folder | PR | Port | State |
|---|---|---|---|---|
| Dust (W-121) | `demos/dust_rust/` | #393 | 47830 | Playable; bot win writes the result file. Hyperbolic desert; one-direction wind with ±20% speed wobble, light bends downwind and the laser 2.4× more (map `laser` field); sand ground-hit puffs; sky-only storm; golden laser. |
| Snow Planet (W-120) | `demos/snow_planet_rust/` | #396 | 47820 | Playable; bot win checked. One central cylinder blizzard (slows, freezes, shortens lasers, bends light); 5 seeded black holes (3 north, 2 offset south); 1 wormhole pair; 6 rounded mounds; green lasers; snow ground-hit puffs. All bench views under 16.7 ms. |
| City Sphere (W-119) | `demos/city_sphere_rust/` | #397 | 47810 | Playable; bot win checked. Outward gravity inside a sphere, glass level with reflecting shots, 1 exposed pylon per team on the glass, metal hex deck, white glass hex frames, core halo. All views under 16.7 ms. |
| Dark Forest (W-122) | `demos/dark_forest_rust/` | #399 | 47840 | Playable; a two-bot match on the merged tree wrote `{"winner": "blue"}`. Seeded mirrored bamboo maze, treehouses, ladders, bridges, candles; no black holes; 5 lensed wormhole pairs (3 ground gates, a lift per team); golden laser. |
| Lunar Surface | `demos/hyperbolic_wormholes_rust/` (and the match's copy) | #410, #409 | 47800 | The original demo; new photo-like Tab → E map page (30° sun). |
| Match (W-123) | `demos/warped_match_rust/` | #403 | 47850 | Runs the five levels in order (Dust, Snow, City, Forest, Lunar), each as its own program; progression page with tinted level boxes and map pictures. |

Also merged: #406 (`INSIDE_GAME_DESIGN.md`: pylons per team 3-2-1-2-3, Dust to Lunar) and #386 (wishlist items W-118 to W-124).

## Level interface (every level)

```
<bin> --serve --port P [--map M] --level-result-file F   # writes {"winner":"blue"|"red"} via F.partial + rename, stays up
<bin> --join ADDR --port P [--map M] [--team blue|red]
<bin> --join ADDR --port P --bot --exit-after-s S --team T --bot-objective consoles
```

The match's `match_levels.yaml` says which program plays each level. `make levels` (default
`LEVEL_SOURCE=checkout`) builds the four levels from this checkout.

## Known gaps, in priority order

1. **Dust speed.** 20 of 24 bench views are under 16.7 ms at 2560×1440; `wormhole`, `wormhole_exit` and
   `laser_into_wormhole` are 17.6–19.3 ms. Next step: precompute ground height into the 0.25 m `MoundGrid`
   (the bump-layer count is set in both `terrain.rs` and the shader; keep them equal or hits won't match
   what players see). `demos/dust_rust/DECISIONS.md` item 17. Done on `claude/dust-wormhole-speed`
   (DECISIONS.md item 18): with the whole height baked, every view's median and p99 is under 16.7 ms on an
   M3 Pro; `laser_into_wormhole`, at 15.8 ms (p99 16.4), has the least headroom.
2. **Match coverage.** On the merged tree, `make e2e-match` (two bots) played City Sphere, Dark Forest, Snow
   Planet and Dust with no stall, and red won in 4 rounds; Lunar Surface wasn't reached. Unsteered bots end
   matches early, so a run through all five levels still needs steering. The defender usually wins: three
   balance options are in `demos/warped_match_rust/DECISIONS.md`, for the user.
3. **p99 frame times** on the Devin VM are dominated by its display; measure on a real Mac.
4. **Deferred features:** Snow Planet's looping world (flat torus, a stacked PR, not built); robots and
   players visible through Dark Forest's wormholes (unfinished on branch `devin/dark-forest-lens-robots`);
   City Sphere floor windows (user kept option A, none).

## Decisions the user made (keep them)

- Laser colours: Snow green for all; Dust and Forest golden white; City and Lunar team red/blue.
- All levels reuse the hyperbolic demo's robots, guns, `scifi_beacon` pylons and consoles.
- Every map gets a slight seeded, team-mirrored randomness in placements.
- Visuals are frozen ("looks good enough"); the next work is correctness and speed.
- Defaults kept: Dust laser bend 2.4×, City floor windows A (none), Lunar map sun 30°.
- The user decides merges into `staging/global` and `main`.

## Checks

The repo has no CI for these folders. In each folder: `cargo build --locked && cargo test && cargo clippy
-- -D warnings && cargo fmt --check`; `cd common/maps && uv run python validate_map.py <maps>`; bench views at
2560×1440 (`make bench-views`). Graphical changes need before/after screenshots in the PR. On the macOS
Devin VM: `export PATH="$(brew --prefix rustup)/bin:$PATH"`, and `brew install cmake uv`.
