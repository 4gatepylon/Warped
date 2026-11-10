# Levels

A level is everything that makes one playable level of Warped: its map (the layout, [README.md](README.md)), the
engine that runs it, its rules and settings, and how menus show it. This page specifies how a level is defined as data,
in one folder, so that a new level can be defined later without writing code, as long as it needs no space, topology
or feature that no engine has yet (the user, 2026-11-09: "Assume we won't need to support new map
topologies/space/types than what is currently there").

**Status (2026-11-09):** this is the spec, with its schemas and the validator's level checks
([What the validator checks](#what-the-validator-checks)); W-143 (declarative levels) in the wishlist. No program
reads `level.json` or `levels.json` yet, and no level folder has one; the [migration plan](#migration-plan) moves each level onto them
after the level crates become libraries of the one-window app. Until a level's migration PR, what
[Where each level is defined today](#where-each-level-is-defined-today) lists stays the truth.

## A level's files

| File | Holds | Written by | Read by |
| --- | --- | --- | --- |
| `common/maps/<name>/map.json` | the layout and its look: space, ground, structures, warps, lights, sky, spawns, pylons, regions, atmosphere, appearance ([README.md](README.md)) | the level's generator, named in its `source` (or by hand) | the level's engine; the validator |
| `common/maps/<name>/level.json` | the rules and settings: title, engine, the server settings that differ from the shared ones, laser colour, the growing black hole, map picture, credits | by hand | the app, the match, the engine; the validator |
| `common/maps/<name>/map_picture.png` (optional) | the map page's picture, from above | the engine's `--make-map-picture` | the app's menus, the engine's map page |
| `common/maps/levels.json` | the engines (what each can draw) and the matches (lines of levels) | by hand | the app, the match; the validator |
| `demos/warped_shared_rust/server_settings.yaml` | the one shared default of every server setting | by hand | every engine (through `warped_shared`) |

`<name>` is letters, digits and `_` (`lunar_surface`), the same in `map.json`'s `name`, `level.json`'s `name`, and
`levels.json`. A folder with a `map.json` but no `level.json` is a map but not a level (`house_demo`).

Two files, not one: `map.json` is generated (Dark Forest's is 8.6 MB, written by `forest_layout.rs`), so what a
person edits by hand (a speed, the title, the laser colour) doesn't sit in a file a script rewrites, and changing
a rule doesn't touch the layout's file, which every map reader parses strictly today.

## `level.json`

[`level.schema.json`](level.schema.json) describes it. Fields marked optional may be left out.

| Field | Meaning |
| --- | --- |
| `format` | always `"warped-level"` |
| `version` | the format's version, `1` |
| `name` | the folder's name, equal to the map's `name` |
| `title` | the level's name in menus and on the progression page: `"Lunar Surface"` |
| `engine` | which engine runs it, a key of `levels.json`'s `engines` |
| `server_settings` (optional) | the server settings that differ from the shared default on this level: `{"<section>": {"<key>": number}}`, only keys the default has ([Server settings](#server-settings)) |
| `laser_color` (optional) | `[r, g, b]`, 0 to 1: the one colour every team's lasers take on this level, given as the engine gives its team colours (it replaces them); without it, each team fires in its team's colour |
| `black_hole_growth` (optional) | `{"warp": "<name>"}`: each console turned off grows that black hole of the map (Lunar Surface, W-68), by the settings' `black_hole` section |
| `map_picture` (optional) | the map picture's file name in the level's folder, a PNG |
| `credits` (optional) | `[{"asset", "author", "license", "url"}]`: the level's own assets (textures, sky); the game's shared assets (guns, robot, font) and the map's models (README, Models) are credited once, elsewhere |

Team and depth are not a level's: a level's place in a match gives them (see [`levels.json`](#levelsjson)). The
pylon count isn't either: the map's pylons give it.

Dust's, for example:

```json
{
  "format": "warped-level",
  "version": 1,
  "name": "dust",
  "title": "Dust",
  "engine": "voxel_dust",
  "laser_color": [1.0, 0.8, 0.3],
  "map_picture": "map_picture.png",
  "server_settings": {"movement": {"jump_time_per_map_jump_time": 0.545}}
}
```

## Server settings

One shared default, and per level only what differs:

1. **Shared default:** `demos/warped_shared_rust/server_settings.yaml`, every section any engine reads, each key with
   Lunar Surface's value (W-126, same play in every level), commented as the copies are today. Sections only some
   engines read are in it too, with neutral values: `space.curvature_per_map_curvature: 1.0` (the map's own
   curvature), `glass` with City Sphere's values.
2. **The level's overrides:** `level.json`'s `server_settings`.
3. **The game's settings:** what the game master sets in the game menu (held by the match server and sent to each
   level server it starts), or `--server-settings <file>` for a level server run alone: the same shape as the
   default, partial or whole, applied to every level. A level the game master changed on its own adds a fourth
   layer for that level only, of the same shape.

A level plays by the default with each layer laid over it in order:

- **Merge:** an override's sections and keys must already be in the layer below, with a number where it has a
  number (an integer where it has an integer); a misspelt key fails when the server starts, naming the file and
  the key's path. Nothing is ever dropped or guessed.
- **Ranges:** the engine checks the merged values as it does today, on the server, when it starts.
- **Sections an engine doesn't use** are ignored as whole sections (City Sphere's `glass` on Lunar Surface); a key
  an engine doesn't know inside a section it uses still fails.
- **A whole file** given as game settings, such as an old copy of `server_settings.yaml`, still loads: it overrides
  every key, so the level plays by it exactly as before.
- **What players get:** the server still sends the merged settings it plays by in its welcome, unchanged.

The level overrides of today's levels, against that default (from the five `server_settings.yaml` copies and Dark
Forest's built-in ones):

| Level | `server_settings` in `level.json` |
| --- | --- |
| Lunar Surface | `{"space": {"curvature_per_map_curvature": 1.5}}` |
| Dust | `{"movement": {"jump_time_per_map_jump_time": 0.545}}` |
| Snow Planet | `{"movement": {"walk_speed_per_map_speed": 1.6, "sprint_speed_per_map_speed": 1.72, "jump_time_per_map_jump_time": 0.545}}` |
| City Sphere | `{"movement": {"walk_speed_per_map_speed": 1.2, "sprint_speed_per_map_speed": 1.44}}` |
| Dark Forest | `{"movement": {"walk_speed_per_map_speed": 1.6, "sprint_speed_per_map_speed": 1.72}}` |

New server settings go into the shared default with a value for every level, and a level that needs another value
overrides it: W-144 (gravity toward black holes and wormholes) adds a section whose strength "might vary" per map,
and W-145 (no walls at map edges) uses the existing `map_edge` section on every map.

Player settings (keys, mouse, view) are each player's, not a level's: one `player_settings.yaml` in
`warped_shared_rust` replaces the five identical copies, with no per-level layer.

## `levels.json`

[`levels.schema.json`](levels.schema.json) describes it: `{"format": "warped-levels", "version": 1, "engines":
{...}, "matches": {...}}`.

### Engines

An engine is a program (a level crate, and after the app work, its library) that can run levels. The set is fixed by
code: a level picks one by name, and `levels.json` says what each can draw, so the validator can refuse a level its
engine can't run. Each engine is `{"crate", "binary", "spaces", "styles", "lighting_models", "map_features",
"required_map_features", "level_features"}`:

- `crate`: its folder from the repository's root; `binary`: its program taking the level interface's flags
  (`--serve`, `--join`, `--bot`).
- `spaces`, `styles`, `lighting_models`: the map's `space.kind`, `appearance.style` and
  `appearance.lighting.model` it can draw.
- `map_features`: the optional parts of a map it can run, each a path into `map.json` from the fixed list in
  `levels.schema.json` (`$defs.map_feature`), such as `atmosphere.wind` or `lights.flicker_frac`; a list's path
  counts as present when any of its items has it. `required_map_features`: the ones a map must have for it.
- `level_features`: the optional parts of `level.json` it can run (`black_hole_growth`, `laser_color`).

The engines of today's levels:

| Engine | Crate (binary) | Spaces | Styles | Map features, beyond the required fields | Level features |
| --- | --- | --- | --- | --- | --- |
| `voxel_lunar` | `demos/hyperbolic_wormholes_rust` (`hyperbolic-wormholes`) | hyperbolic | `lunar`, `meadow` | `ground.tiling`, `ground.terrain`, `structures`, `structures.mesh`, `boxes`, `warps`, `lights`, `sky.planet`, `sky.milky_way`, `sky.fog_m`, `regions.zones`, `regions.boundary`, `regions.zone_walls` | `black_hole_growth` |
| `voxel_dust` | `demos/dust_rust` (`hyperbolic-wormholes`) | hyperbolic | `desert`, `lunar`, `meadow` | `voxel_lunar`'s, and `atmosphere.fog`, `atmosphere.sand`, `atmosphere.wind`, `appearance.laser_color` | `black_hole_growth` |
| `voxel_snow` | `demos/snow_planet_rust` (`snow-planet`) | hyperbolic; euclidean, drawn as hyperbolic space of radius 160 m (`Map::curve_flat`) | `snow`, `lunar`, `meadow` | `voxel_lunar`'s, and `space.torus_periods_m` (the level ends at its edge: it doesn't loop; W-145 drops it), `sky.snow_per_m3`, `regions.blizzards`, `appearance.laser_color` | `black_hole_growth` |
| `sphere_city` | `demos/city_sphere_rust` (`city-sphere`) | euclidean, needs `ground.sphere` | `city_sphere` | `ground.sphere`, `shells`, `boxes`, `lights` (not drawn: the core's light is the engine's), `variation`, `lasers` | none |
| `forest` | `demos/dark_forest_rust` (`dark-forest`) | euclidean | `forest` | `structures`, `structures.mesh`, `boxes`, `ladders`, `warps`, `lights`, `lights.flicker_frac`, `sky.fog_m`, `scatter`, `laser` | `laser_color` |

Every engine takes the required fields of a map (`ground.material`, `sky`, `spawns` and the rest), styles and
lighting models aside. Each engine's lighting models are `ambient_sun` and, for the three voxel engines,
`lunar_lambert`. The map features named `laser`, `lasers` and `appearance.laser_color` are the three ways maps give
a laser colour today; each engine keeps its own until its migration moves the colour to `level.json`'s
`laser_color`, and then loses it. The three voxel engines are copies of one engine; once they are merged (not
planned yet), they become one `voxel` engine with the union of their features, and their levels change one field.
When an engine learns a feature (W-145 gives Dark Forest a slowing edge, which its map can give as
`regions.boundary`), its entry in `levels.json` gains it in the same PR.

### Matches

`"matches": {"default": {"levels": ["dust", "snow_planet", "city_sphere", "dark_forest", "lunar_surface"]}}`: each
match is a line of levels, blue's deepest first, an odd number of distinct levels, at least 3
([DESIGN.md](../../game_design/DESIGN.md), Matches). A level's place gives its team and depth: the middle one is
purple, depth 0; those before it are blue's, those after it red's, and depth counts outward from the middle. It
replaces `demos/warped_match_rust/match_levels.yaml`, whose other fields go: `name` is the level's `title`, `map` its
folder, `map_picture` its `level.json`'s, `program` comes from the engine's `crate` and `binary` until levels are
libraries, and `folder`, `branch`, `status`, `stand_in_for`, `server_args` and `client_args` are no longer used.

## Names engines read

Some names in a map mean something to engines. They are part of the format; the validator checks the first.

- **Consoles and pylons:** a console is a `light` material named `<team> console <n>` (`blue console 1`), numbered
  from 1 within each team, and each structure or box that is made of one is one of that team's pylons (one console a
  pylon, as DESIGN.md says). Each team has as many pylons as the other, at least 1. Lunar Surface, Dust and Snow
  Planet follow this already; City Sphere numbers from 0 and Dark Forest leaves the number out (one material for
  both of a team's pylons): until their migrations rename them, the validator counts pylons by the materials whatever
  their number, and the engines keep their own rules (`map::get_console_team`, City Sphere's `pylons` and `consoles`
  box groups, Dark Forest's `Surface::RedConsole`).
- **City Sphere's box groups:** `stairs` (steps up to the glass level, whose holes open only as wide as the stairs
  under them), `lifts` (pads: holding jump in a hole above one rides up), `pylons` and `consoles`.
- **Style materials:** a style draws some materials by name. `lunar`: `regolith`, `dark regolith`, `basalt`, `solar
  panel`, `hazard paint`, `landing pad`, `hull`, `gold foil`; `desert`: `sand`, `dark sand`, `sandstone`, `sandstone
  blocks`; `city_sphere`: `street`, `building`, `painted panel`, `glass obstacle`, `stairs`; `forest`: `forest floor`,
  `bamboo`, `leaves`, `rock`, `rope`, `red paint`, `blue paint`, `candle wax`, `candle flame`. Other materials are
  drawn plainly in their colour.

## Defining a new level

1. **Pick the engine** whose spaces, styles and features (the table above) cover the level: a hyperbolic desert with
   wind is `voxel_dust`, a hollow sphere is `sphere_city`, a flat forest of ladders and decks is `forest`.
2. **Write the map**, `common/maps/<name>/map.json` ([README.md](README.md)): by hand for a small level, or, for a
   large one, with a generator script beside the engine's other tools that writes the file and is named in
   `source`. It needs spawns for both teams and as many pylons for each team.
3. **Write `common/maps/<name>/level.json`**: `name`, `title`, `engine`, and only what differs from the defaults
   (settings overrides, `laser_color`, `black_hole_growth`).
4. **Check it:** `make -C common/maps validate-maps`. It names every problem: an engine that can't draw a field,
   a misspelt field, uneven pylons, a missing spawn.
5. **Make its map picture** with the engine's `--make-map-picture <name>` into the level's folder, and name it in
   `map_picture`.
6. **Add it to a match** in `levels.json`, or leave it out to play it alone.
7. **Rebuild** the app: the build embeds every level folder (so every player of one build has the same levels), and
   the menus list the new level. Settings overrides fail at start-up if a key is misspelt.

## What the validator checks

`validate_map.py` checks a folder with a `level.json` as a level (`make validate-maps`, or `uv run python
validate_map.py <folder>`), after its map:

- `level.json` against `level.schema.json`, and `levels.json` against `levels.schema.json`;
- `levels.json`: each engine's crate is a folder of the repository; each match has an odd number of distinct levels,
  at least 3, each a folder with a `level.json`;
- the level's `name` is its map's, and its `engine` is in `levels.json`;
- the engine can draw the map's space, style and lighting model, every map feature the map has, and every level
  feature the level has, and the map has the engine's required map features;
- `black_hole_growth` names exactly one warp, a black hole;
- the laser colour is set at most once (in `level.json` or, until the level's migration, in one of the map's three
  old fields);
- each team has as many pylons as the other, at least 1, and a spawn (its own or one for `any`);
- `map_picture`, if given, is a file in the folder.

The server settings overrides are checked by the program that loads them (the validator has no YAML reader): a
`warped_shared` test loads every level's settings, as each server does when it starts.

## Compatibility

- **Maps:** nothing here changes `map.json`; the format stays at version 1, and every map stays valid. The
  migrations remove the three old laser-colour fields from the maps (and the format, raising its version to 2, after
  the last one), and rename City Sphere's and Dark Forest's consoles.
- **Map readers:** the spec says readers ignore what they don't use, but the Rust readers refuse unknown fields
  (`deny_unknown_fields`), and Dust, Snow Planet and City Sphere embed maps of other levels (`house_demo` and
  `lunar_surface`). The migrations make each engine read only its own levels' maps, so a change to
  one map can't break another level's build.
- **Server settings:** each level's merged settings equal its `server_settings.yaml` of today, so its welcome sends
  the same bytes and windows of other builds of the same commit still join; a test compares them for every level
  before its copy is deleted. An old settings file passed with `--server-settings` loads as before.
- **Player settings:** unchanged; the five identical files become one.
- **New formats:** `level.json` and `levels.json` start at version 1; a reader fails clearly on another version.

## Where each level is defined today

As of `staging/user-4gatepylon` at #482 (2026-11-09):

| | Lunar Surface | Dust | Snow Planet | City Sphere | Dark Forest |
| --- | --- | --- | --- | --- | --- |
| Crate | `hyperbolic_wormholes_rust` (and the match's copy) | `dust_rust` | `snow_planet_rust` | `city_sphere_rust` | `dark_forest_rust` |
| Map made by | `demos/levels/lunar_surface/export_map.mjs` | `tools/make_dust_map.py` (seed 121) | `tools/make_map.py` (seed 7) | `tools/make_map.py` (`variation.seed` 1192) | `src/forest_layout.rs` (`--export-map`, `scatter.seed`) |
| Map read by | `include_str!` in `map.rs` | the same | the same | `include_str!` in `city_world.rs` | from disk at run time (`--map <path>`, default `common/maps/dark_forest/map.json`) |
| Space | H³, the map's R = 40 m, played at 26.7 m (`space.curvature_per_map_curvature: 1.5` in settings) | H³, R = 50 m | E³ torus 112 × 72 m, drawn as H³ at R = 160 m, bounded (W-145: plain E³, a slowing edge) | E³, sphere of 24 m (`ground.sphere`) | E³, 60 × 112 m inside rock walls (W-145: no walls, a slowing edge, forest past it) |
| Pylons a team | 3: structures `<team> pylon`, consoles `<team> console 1-3` | 3, the same | 2, the same | 1: boxes in groups `pylons` and `consoles`, consoles `<team> console 0` | 2: structures `<team> pylon 0-1`, one material `<team> console` for both |
| Laser colour | team colours (no field) | map `appearance.laser_color` | map `appearance.laser_color` | map `lasers.team_colors` (the team colours again) | map `laser.rgb` |
| Features | the growing black hole (code: the map's only black hole, when it has consoles); crater edge; zone walls | wind, gusts, fog, sand (`atmosphere`); laser bend 2.4 (`atmosphere.wind.laser_gradient_per_view_gradient`) | one blizzard (`regions.blizzards`, with its slow, damage and shot reach); snowfall (`sky.snow_per_m3`); 5 black holes, none growing | glass shell (`shells`, reflections: settings `glass`); stairs and lifts (box groups) | ladders, decks and bridges (`ladders`, `boxes`); 5 lensed wormhole pairs (`warps`; the lens's cube atlas is the engine's); candles (`lights.flicker_frac`) |
| Server settings | `server_settings.yaml` (also the match's, identical) | copy: jump time 0.545, curvature 1.0 | copy: walk 1.6, sprint 1.72, jump time 0.545 | copy: walk 1.2, sprint 1.44, `glass` | none: `ServerSettings::Builtin` in `src/settings.rs`, a subset, walk 1.6, sprint 1.72 |
| Player settings | `player_settings.yaml` | identical copy | identical copy | identical copy | `HIT_EFFECTS` and `HEAD_BOB` constants (Lunar's values) |
| Map picture | none: drawn from the map in code (`map_layout.rs`) | `map_pictures/dust_r50m.png` (`--make-map-picture`) | `map_pictures/snow_planet.png` (`--map-picture`) | `assets/map_picture.png` (`make map-picture`) | `assets/map_picture.png` (`--map-picture <png>`) |
| Credits | `src/credits.rs` | `src/credits.rs` (its own surfaces) | `src/credits.rs` (Lunar's, unchanged) | `src/credits.rs` (adds 2 textures) | `src/credits.rs`; README's asset table |
| Title, port | `title.rs` "Lunar Surface", 47800 | 47830 | 47820 | 47810 | 47840 |
| In the match | `match_levels.yaml`: `program: self` | `.levels/dust/...` binary | `.levels/snow_planet/...` | `.levels/city_sphere/...` | `.levels/dark_forest/...`, `takes_bot_label: false` |

Duplicates and inconsistencies this design removes:

- **Server settings:** five `server_settings.yaml` copies (Lunar Surface's and the match's identical; Dust, Snow
  Planet and City Sphere one to four values apart, City adding `glass`) plus Dark Forest's sixth copy in code, a
  subset of the same values.
- **Player settings:** five identical `player_settings.yaml` copies plus Dark Forest's constants.
- **Laser colour:** three map fields for one thing (`laser.rgb`, `appearance.laser_color`, `lasers`), the README
  describing `appearance.laser_color` twice; City Sphere's `lasers.team_colors` only repeats the team colours.
- **Pylons and consoles:** three naming dialects (see [Names engines read](#names-engines-read)).
- **The growing black hole:** decided by a rule in code (the only black hole of a map with consoles), not by the
  level.
- **Map pictures:** four places and three flags for one job.
- **`match_levels.yaml`:** program paths into `.levels/` checkouts, and `branch` and `status` fields that are stale
  since the levels were merged.
- **Embedded maps:** `house_demo` and `lunar_surface` built into five crates, of which Dust, Snow Planet and City
  Sphere never play them as levels.

Duplicates left for later, outside levels: the team colours are constants in 13 places across 5 crates with three
slightly different values (beams `[0.3, 0.55, 1.0]`, minimap `[0.35, 0.6, 1.0]`, pictures `[90, 150, 255]`), and the
console materials' colours differ per map; making them one would change pictures, so it waits for the user.

## Migration plan

After the level-library PRs (each level crate a `lib.rs` the app calls), one PR at a time, each level in its own
commit, each proving that nothing changes in play or pictures: `make bench-views` in each level it touches before
and after, compared pixel by pixel (identical), and each level's play trace (`record_play_trace`) identical bit for
bit.

1. **`levels.json` and the five `level.json`** (data only, read by nothing yet): the files as above, with the
   overrides of [Server settings](#server-settings). The validator checks them against today's maps. (Drafts of
   all six pass already.)
2. **One server settings default:** `warped_shared_rust/server_settings.yaml` and `player_settings.yaml`,
   `warped_shared::level` (reads `level.json` and `levels.json`, the engine as an enum, fails fast on another
   version) and `warped_shared::settings_layers` (the merge). A test merges each level's settings and compares them
   with its `server_settings.yaml` of today (equal values, equal welcome JSON); then each crate reads the merged
   settings and deletes its copies, Dark Forest last (its built-in settings become its level's overrides).
3. **The match reads `levels.json`:** its order, titles and engines replace `match_levels.yaml`, which goes; Dark
   Forest's bot takes `--label` like the others.
4. **Map pictures and titles:** each picture moves into its level's folder (`git mv`, the same bytes) and is named
   in `level.json`; every engine's flag becomes `--make-map-picture <name>`; menus take `title`.
5. **Laser colour:** `level.json`'s `laser_color` (Dust's, Snow Planet's, Dark Forest's values moved verbatim);
   the generators stop writing the map fields; City Sphere drops `lasers` and uses the team colours it repeated.
6. **The growing black hole:** the voxel engines grow the warp `black_hole_growth` names, not the map's only black
   hole (Lunar Surface's file names it).
7. **Consoles:** City Sphere's and Dark Forest's generators number their consoles from 1, one material per pylon
   (same colours), and their engines find consoles by the shared rule; the validator then requires the numbered
   form.
8. **Maps by level:** each engine embeds only its levels' folders, found by a build script from `level.json`'s
   `engine` (no list to edit), and its map reader ignores fields it doesn't use, as the spec says; the map format
   goes to version 2 without the old laser fields.

Steps 2 to 8 touch each level crate's `settings.rs`, `map.rs` and menus, so each goes after the controls (W-140)
and app-shell (W-141) work in those files has merged.

## Questions for the user

These would make levels simpler still, but change the game or settings files people have, so they wait for a yes:

- **Speeds in m/s:** a map gives speeds and gravity, and settings multiply them (`walk_speed_per_map_speed`), so
  Dust and Snow Planet override the jump time only to cancel their maps' gravity (9.81 instead of 6 m/s²). Settings
  in m/s and seconds (walk 10 m/s, jump 0.58 s) would need no map speeds and fewer overrides, and the game menu
  would show real speeds; Dust's jump would change by about 1e-5 s, and saved settings files would need converting.
- **Gameplay numbers out of the map:** the blizzard's slow, damage and shot reach and the wind's laser bend are in
  the map, while City Sphere's reflections are in settings. In settings, the game master could tune them too.
- **One pylon look:** a map could place pylons as `{"team", "frame", "height_m"}` and every engine draw the same
  beacon and console; today their sizes (12 m, 3.2 m, City's box) and console colours differ per map.
- **Team colours:** one set for every level, in `warped_shared`.
