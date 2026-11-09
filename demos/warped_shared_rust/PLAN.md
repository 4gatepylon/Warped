# Plan: one way to move, shoot, get hit and see it, in every level

The user wants the health bars, the guns, and running, crouching, jumping and the rest shared by all five levels,
Dark Forest and City Sphere included. Step 1 (this crate) moved the files that were already the same; this plan is
step 2: make the play itself one implementation, with Lunar Surface's numbers, that every level calls. It is a plan
for review; nothing here is built yet.

## Where each part is today

| Part | Lunar Surface, Dust, Snow Planet, the match | City Sphere | Dark Forest |
| --- | --- | --- | --- |
| Walking, sprinting, crouching, jumping, sprint energy, head bob | `VoxelPlayer::step` in `voxel_world.rs` (H³, voxels and shapes), settings `movement`, `sprint_energy`, `view.head_bob` | `CityWorld::move_player` in `city_world.rs` (inside a sphere, gravity outward), the same settings, its own code | `Player::update` in `src/game/player.rs` (flat, f32, boxes), its own constants (walk 4.4 m/s, jump 5.4 m/s, gravity 16 m/s²), Lunar's sprint energy copied as constants |
| The two guns: fire, reload (sniper), heat and overheat (rifle), recoil, aim zoom, shot noise, the gun's motion | `Session` in `main.rs` (`update_gun`, `update_rifle`, `switch_gun`, `can_shoot`, `get_zoom`, `make_wand`, `get_sway`), `shot_noise.rs`, settings `sniper`, `rifle`, `aim`, `beam_speckle` | the same copy | one revolver (`src/game/weapon.rs`: 12-shot magazine, reload, hip and aimed spread), fps_demo viewmodel |
| Health, damage, respawn | the server counts `damage_frac += 1 / hits_to_kill` (`net.rs`); the shooter picks `hits_to_kill` (`take_hit_weight`: sniper 1, rifle 2 to the head or 12 to the body); `respawn.delay_s` 3 s; `when_hit` slows for 1 s | the same copy | online: the same `net.rs` copy; offline against bots: 100 HP, head and body damage, regeneration after 5 s |
| Hit effects | `HitEffect` (red edges, blur, shake) from `hit_effects` | the same | its own copy of `HitEffect` in `render/overlay.rs` |
| HUD: bars, crosshair, console bar, "YOU DIED" | `Session::get_overlay_rects`, `get_crosshair_rects`, `get_hud_lines`, `get_banner_lines` in `main.rs`; `hud_bars.rs` (shared now) | the same | `src/game/hud.rs` with copies of `hud_bars.rs` and the overlay rect types (DECISIONS.md D16) |

## Shared modules and their interfaces

Each module is pure logic, without a window or a GPU, so it is testable alone and builds on Linux (the `game/` plan's
`warped-core`, #375, can take it as it is). A level keeps its own space, collisions, tracing and drawing.

1. **`play_settings`** (from `settings.rs`): the sections every level shares, read once: `movement`,
   `sprint_energy`, `sniper`, `rifle`, `beam_speckle`, `aim`, `consoles`, `respawn`, `when_hit`, `flashlight`,
   `wormholes`, and the player's own `PerPlayerSettings` (keys, view, head bob, hit effects, minimap) with the one
   built-in `player_settings.yaml`, moved here. A level's own sections (Lunar's `space` and `black_hole`, City
   Sphere's `glass`) stay in the level: `ServerSettings<L> { play: PlaySettings, level: L }`, read by taking the
   shared sections out of the YAML mapping by name and handing the rest to the level's `L` (both keep
   `deny_unknown_fields`, which serde's `flatten` would lose). Each level keeps its own `server_settings.yaml`, so
   City's 2.4 sprint factor and Dust's jump time stay; the JSON the server sends on a welcome keeps its bytes
   (`compatible-changes` skill).
2. **`movement`**: the rules of moving, without the space. `MoveState` (crouch fraction, sprint energy, vertical
   speed, slowed time, the bob's phase and depth, time since the last movement key) and
   `MoveState::step(&mut self, input: &MoveInput, contact: Contact, settings: &PlaySettings, dt_s) -> MoveIntent`,
   where `MoveIntent` is the body's wanted velocity in its own frame (forward, right, up, in m/s), its eye height
   and whether a shot may fire. A level implements one trait for its ground:

   ```rust
   pub trait Ground {
       type Body;
       /// Moves `body` by `intent` for `dt_s` in this level's space, with its collisions, and says what it stands on.
       fn move_body(&self, body: &mut Self::Body, intent: &MoveIntent, dt_s: f64) -> Contact;
   }
   ```

   `VoxelWorld` implements it for Lunar, Dust, Snow and the match (H³, voxels, shapes, warps, the map's edge),
   `CityWorld` for City Sphere (the sphere, outward gravity, the glass and its holes), and Dark Forest's
   `collision.rs` for its forest (boxes, and its ladders as a contact kind of its own). The map's own speeds
   (`walk_speed_per_map_speed` times the map's) stay per map, as now.
3. **`weapons`**: `Gun` (sniper, rifle) and `Weapons`: the gun in hand, the sniper's reload, the rifle's heat and
   overheat, recoil and its settling, the aim's zoom, `shot_noise` (moved here), and the gun's pose (`make_wand`,
   `get_sway`). `Weapons::update(&mut self, input: &WeaponInput, intent: &MoveIntent, settings, dt_s) ->
   Vec<ShotRequest>`; each `ShotRequest` is a gun, an aim offset and a speckle. The level traces each shot in its
   own space (`trace_laser`, City's glass reflections, Dark Forest's lenses) and gives back what it hit.
4. **`health`**: `Health` with Lunar's model: `take_hit(gun, is_head) -> HitOutcome` (the weight from
   `take_hit_weight`, one rifle hit per shot), `damage_frac`, `is_eliminated`, `respawn_in_s`, the slow after a hit,
   and the `HitEffect` timing. The server's `damage_frac` bookkeeping in `net.rs` calls the same code, so a window
   and the server can't disagree.
5. **`hud`**: `HudState` (health, ammo or heat, energy, the console being held, the gun, aim, eliminated and the
   countdown, the banner) and `make_hud(&HudState, window_px) -> Hud { rects: Vec<OverlayRect>, lines: Vec<String> }`:
   the bars (`hud_bars`), the crosshair, the console bar, "YOU DIED" and the countdown, moved out of `main.rs`.
   Each renderer draws `rects` with its overlay shader (every Lunar copy's `render_metal.metal`, and Dark Forest's
   `overlay.metal`, already draw this struct) and `lines` with its own text.
6. **`overlay` split**: the rect types (`OverlayRect`, `RectStyle`, `RectPulse` and their constants) go into a module
   that needs nothing else, so Dark Forest can use them without `viewmodel`'s `GunModel`; `Overlay` itself stays
   with the Lunar renderers.

## Order (one PR each, each level switched in its own commit)

1. `play_settings`, used by the four Lunar copies and City Sphere. Shows nothing changed: each level's parsed
   settings and welcome JSON are equal before and after (a test per level).
2. `health`, then `weapons` (with `shot_noise`), then `hud`, each moved out of the Lunar copies' `main.rs` and
   called from there and from City Sphere's. These also move the last files that only waited for `settings`
   (`shot_noise`, `menu` once `credits` and `net::Score` are parameters).
3. `movement` with the `Ground` trait: `VoxelWorld` first (four levels at once, since they share `voxel_world.rs`'s
   walking code), then City Sphere's `CityWorld`.
4. Dark Forest, in this order: raise its `glam` from 0.30 to 0.34 (this crate's); depend on `warped_shared`; replace
   its copies of `hud_bars.rs`, the overlay rect types and `HitEffect` with the shared ones; then `hud`, `health`
   (Lunar's hits instead of 100 HP and regeneration, offline too), `weapons` (both guns and their numbers; its own
   PBR renderer keeps drawing the guns, posed by the shared gun pose), and last `movement` with its `Ground`.
5. The files that only waited for these (`thrown_wormholes`, `third_person`, `minimap`, `console_reach`), once a
   level-specific part of each is a parameter.

The other agents' areas (each level's `main.rs`, `net.rs`, `multiplayer.rs`, Snow Planet's looping world) are
touched only where a call moves to the shared crate, one level per commit, after merging their latest work.

## Showing that nothing changed in the four copies and City Sphere

- **Pictures:** `make bench-views` in each level before and after every PR, compared pixel by pixel, as step 1 did
  (every view identical).
- **Play traces:** before the first PR, a `--record-play-trace <script>` tool in each level plays a scripted input
  list at a fixed 60 Hz (walk, sprint until empty, crouch, jump, fire both guns, overheat, aim, take hits until
  eliminated, respawn) without a window, and writes each step's eye frame, move state, weapon state, health and HUD
  rects. Each PR reruns it and requires the same trace bit for bit.
- **Tests:** each level's own tests keep passing, and the W-126 (same play in every level) tests that already compare
  a level's jump and settings with Lunar's (Dust, Snow Planet, City Sphere) keep passing.
- **Matches:** `make e2e-match` in the match still finishes, and `make levels` builds every level.

## Showing that Dark Forest now plays like Lunar Surface

- The same play-trace script in a flat test room of Dark Forest and on Lunar Surface's flat spawn area: equal walking,
  sprinting and crouched speeds, jump height and time in the air, crouch time, sprint drain and refill times, fire
  rate, heat, recoil, zoom, hits to eliminate and respawn delay, within 1e-9 (the spaces differ, the rules don't).
- A W-126 test in Dark Forest like City Sphere's `measure_city_jump`: its jump and settings files against Lunar's.
- HUD pictures of Dark Forest and Lunar side by side (bars, crosshair, console bar, "YOU DIED"), and a playtest by
  the user (`interactive-review` skill), since feel can't be proven by tests.

## Questions for the user

- Lunar's walking speed is 10 m/s (2 × the map's 5 m/s) and Dark Forest's is 4.4 m/s, in a maze of 1.5-3 m
  corridors. Should Dark Forest take Lunar's speeds as they are, or keep a per-map speed (as every voxel map has)
  with Lunar's rules?
- Dark Forest's single revolver becomes Lunar's two guns (sniper and rifle): keep its revolver model for one of
  them, or draw Lunar's wand and block?
- Dark Forest's health regenerates (after 5 s at 12 HP/s); Lunar's doesn't. Drop regeneration everywhere, as Lunar
  plays?
