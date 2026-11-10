# Handoff: the glossary split and the worlds (2026-11-10)

This folder hands unfinished work to another agent. The work stopped because the session ran out of quota, not
because anything failed. Read this file first. Nothing in this folder is meant to be merged: the tools are scratch
scripts that don't follow the repo's code rules, and the pictures are drafts.

## What the user wants

PR #576 added `game_design/GLOSSARY.md` in one piece. The user reviewed it with 18 comments and then asked:

> can you please split that PR into multiple small PRs? each one would tackle each of my requests by writing up one
> section [...] run the tree ascii vis and names for map locations and other modifications per section by me
> succinctly before making a PR [...] you will need to rebase the PRs that come later in the stack for this and
> that's ok

The trees, names and defaults below were run by the user and approved. Build from them; don't re-ask.

### The 18 review comments on #576

1. "Words we retired" becomes "Words we retired on 20261110".
2. Remove every *Proposed* mark for the actual merge: the picks are final unless a comment below says otherwise.
3. Next to health and energy, add **ammo** for the gun.
4. Remove the bot/robot distinction. Later answer: "bot/robot skin/sprite is the actual skin on the sprite and
   sprite; bot can also refer to the entity itself when mentioned alone; player is the entity for the player which
   also has the bot sprite". A sprite is always the skin or visuals of any entity.
5. Add **map layout**: several level layouts plus overall rules such as which level leads to which.
6. Add **pages**: analogous to web pages; all menus are pages.
7. "Screens and windows" really means pages and SWGs on the window.
8. "Level view" is the **scene**.
9. The menu `Esc` opens in a level is the **level menu**.
10. Add a note that we may call things "in-game" (the ones that appear on `Tab` and so on) but that is not correct.
11. Add the HUD to that section: pages and UIEs/SWGs are one big section.
12. The HUD goes into that section, with a tree for pages and then for the internals of a page such as the HUD.
13. The user may say "speckle" for beam noise, so it is valid wording for now.
14. Under level features, add a note that if this changes the agent has to ask the user or make a PR to update it.
15. Curvature amount and curvature type are separate terms; "curvature" sometimes means one, sometimes the other.
16. Replace the "Not" column with "Might be called by the user". Official language always in code, PRs and docs;
    the user may speak loosely; the agent decides when it is obvious from context, or asks.
17. Instead of "Looks of the world", a tree per level of what each component is that exists now: a BOM.
18. Remove bunker from the terms; "structure" is synonymous with "object"; mention sprites and skins; in the BOM,
    list the literal structures, each with a unique name in its level, like CS:GO callouts.

### What the user added in chat afterwards

- **Two trees.** The pages tree has only pages. Each page then has its own tree, which is conceptually an SWG.
  "level page is the swg with the actual world render".
- **Page graph.** Below the pages tree, describe which pages connect to which, as out-edges per page.
- **World.** Add the term "world" (the game world), where the actual objects are enumerated. Partition each world
  into its zones (red zone, blue zone, middle zone), and list each zone's things: "red spawn", "red left pylon",
  "red mid pylon", "metallic bunker", and so on.
- **Map screenshots.** Embed screenshots of the map in the documentation, as jpg files next to it, with little red
  circles or rectangles around objects that say what they are. At most 3 screenshots per level if one would have
  too much text. Sections must be well defined.
- **Instance.** The word "instance" must be defined.
- **Wishlist PR.** Right after the world objects PR, a PR that adds two wishlist items: an option to toggle
  in-map labels while playing, and an admin mode to set those labels (and to set things in general; needs design).
- **Defaults accepted** ("your defaults are fine"): the level menu's MATCH, SCORES and so on are sub-pages; the SWG
  file of #580 folds into the pages section; left and right are as seen from each team's own spawn facing the
  other team; repeated pieces get one name per cluster; "in-level menu" and "in-game menu" go in the "Might be
  called by the user" column of the level menu.
- **Earlier rules that still hold.** PRs go against `staging/user-4gatepylon`, are not drafts, and are never
  auto-merged. Do NOT create an actual SWG file or language. One message in the chat was meant for another agent
  (about "interactive prototype design" and mocks); ignore it.

## Where things are

Merged into `staging/user-4gatepylon`: #569, #570, #571, #572, #573, #575 (the commit and PR rules, and the root
README). The commit rules are in force: follow the `pr-description` skill for every commit and PR.

Open PRs, all of them the OLD version of this work:

| PR | Branch | What to do |
| --- | --- | --- |
| #576 | `claude/game-glossary` | Superseded by the chain below. Close it, or force-push the chain's first branch to it. |
| #579 | `claude/agents-md-commit-rules` | Still wanted. Rebase it onto the top of the new chain (its `AGENTS.md` edit is one line). |
| #580 | `claude/ui-tree` | Superseded: its SWG file is folded into the pages section. Close it. |

The new chain, bottom to top, pushed but with no PRs yet. Each branch adds one commit (the wishlist one adds two):

| # | Branch | Holds |
| --- | --- | --- |
| 1 | `claude/glossary-rules` | The glossary's "How to use it", the `AGENTS.md` Glossary rule (16,333 bytes, under the 16 KiB cap), the `DESIGN.md` Terms pointer |
| 2 | `claude/glossary-structure-of-play` | game, match, level, world, map, level layout, map layout, zone, boundary plane, instance, ... |
| 3 | `claude/glossary-people-and-entities` | entity, player, bot, sprite, bot sprite, ammo, ... |
| 4 | `claude/glossary-programs-and-places` | the app, window, game server, level server, ... |
| 5 | `claude/glossary-pages-and-swgs` | terms, the pages tree, the page graph (from `demos/warped_mvp_rust/src/screens.rs`), the SWG model, the level page's SWG |
| 6 | `claude/glossary-world-objects` | object = structure, pylon, console, warp objects, laser, beam noise, ... |
| 7 | `claude/wishlist-add-world-labels` | W-210 (labels on world objects while playing) and W-211 (admin mode to set world labels), one commit each, in the wishlist commit format |
| 8 | `claude/glossary-handoff` | this folder only; drop it once the work is done |

## What is left

1. **Worlds.** Not written yet. Create `game_design/worlds/README.md` (the term, the naming rules below, and the
   NOTE of comment 14), one file per level with its world tree (zones, then named objects), and the pictures as
   jpg files beside them. Add a short "Worlds" section to the glossary that points there. Suggested PRs: one for
   the README, then one per level.
2. **Remaining glossary sections**, one PR each: Moving and looking; Space and physics (with curvature type and
   curvature amount); Building, measuring and shipping; Tracking work; Words we retired on 20261110. The earlier
   text is in `section_drafts/old_glossary_from_pr_576.md`: adapt it to the new third column and drop "Looks of the
   world" (its terms moved to World objects and to the worlds).
3. **Stack and PRs.** Open the PRs with the `pr-description` skill's template (it now has Categories and Agents
   sections), ready for review, never auto-merged. Rebase #579 on top. `gh stack init --base
   staging/user-4gatepylon <branches bottom to top>` adopts existing branches; `gh stack submit --auto --open
   --remote origin` opens them.
4. **Before merging the wishlist PR**, check that nobody else took W-210 or W-211 on staging.

## Naming rules for the worlds (approved)

- Every object in a world has a name unique in that world.
- Things in a team's zone are `<team> <thing>`. Where a team has several, they are left, mid and right as seen from
  that team's own spawn facing the other team. On the map pictures blue is on the left (top in Dark Forest), so
  blue's left is the top of the picture and red's left is the bottom.
- Middle-zone things have no team: "top" and "bottom" as on the main map, or "blue-side" and "red-side".
- Repeated pieces get one name per cluster, numbered from the team's spawn outward ("blue hesco wall 3").
- The names are in `tools/world_<level>.py` and on the pictures.

## The pictures

`pictures/` has 14 draft jpgs: Snow Planet (1), Dark Forest (3), Dust (3), City Sphere (2), Lunar Surface (3).

- Four levels use the game's own map picture from the repo: `demos/dust_rust/map_pictures/dust_r50m.png`,
  `demos/snow_planet_rust/map_pictures/snow_planet.png`, `demos/city_sphere_rust/assets/map_picture.png`,
  `demos/dark_forest_rust/assets/map_picture.png`. The marks were checked by eye and sit on the objects.
- **Lunar Surface has no map picture in the repo.** Its three pictures are drawn from its layout file and say so
  on the image. A real one needs the engine (Metal, so a Mac). The last version of these three was not looked at
  after its labels were moved.
- Some labels still overlap slightly (Dark Forest's middle zone, City Sphere near the lifts).
- Positions come from `common/maps/<level>/map.json`. Picture mappings, found by fitting landmarks:
  Snow Planet 16 px/m; Dark Forest 10 px/m; Dust 3.75 px/m in Fermi coordinates (R = 50 m); City Sphere is an
  equirectangular unwrap (longitude from `atan2(z, x)`, latitude from `asin(y / r)`); Lunar Surface is drawn at
  10 px/m in Fermi coordinates (R = 40 m, zone edges at 45 m).
- Left and right were derived from the pictures' handedness (a right-handed, Y-up world). This was not checked
  against the game's code; check it in play or in code before the names are final.

## The tools

`tools/` are scratch scripts. They need Python 3 and Pillow, and work on Linux (the font falls back to DejaVu
Sans Bold). Run `python3 tools/world_<level>.py` from anywhere; it rewrites that level's jpgs in `pictures/`.

- `annotate.py`: draws red circles and rectangles with names on a picture.
- `zone_objects.py`: lists a hyperbolic level's objects by zone (`python3 zone_objects.py dust 50 48`).
- `world_*.py`: each level's names, positions and crops.
- `mk_section.py`, `trace_block.sh`, `pr_bodies_v2.py`: helpers that add a glossary section as a commit, print the
  `AGENT TRACES` section, and write PR bodies. They have this machine's paths in them and are for reference only.

`section_drafts/02.md` to `06.md` are the section texts already committed in the chain.
