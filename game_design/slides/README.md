# Official slides

`index.html` is the official slide deck of Warped: the game as it is built on `staging/user-4gatepylon` on
2026-11-08 (commit `7d614d8`), with real pictures of the game. The preliminary design deck, from before the game was
built, is kept in [`../slides_preliminary/`](../slides_preliminary/README.md).

1. Title: Warped, a team shooter across five curved-space levels.
2. The game in play: two gameplay videos from YouTube.
3. The match: the line of five levels, the wavefront and the progression page.
4. Winning a level: pylons per team, holding `E` on a console, the console's pulse, the win.
5. Moving: walking, sprinting and its energy, crouching, jumping, head bob, third person, the map edge.
6. Two guns: the sniper and the rifle, with their numbers.
7. Shots are light: speckle, trails, ground dust, laser colours, friendly fire.
8. Damage and respawn: hit effects for any damage, "YOU DIED", the respawn.
9. Thrown wormholes: throwing, linking, shooting an enemy's pair.
10. What a player sees: the HUD, the minimaps, the menu, the lobby, banners and the exit prompt.
11. to 15. One slide per level: Dust, Snow Planet, City Sphere, Dark Forest, Lunar Surface.
16. Curved space: black holes, wormholes and the colour flash at a throat.
17. In progress: Dark Forest's parity, the looping worlds, the match balance.

Every number comes from the code and the settings files: [`../DESIGN.md`](../DESIGN.md),
[`../FOUR_MAPS_HANDOFF.md`](../FOUR_MAPS_HANDOFF.md), each level's `README.md`, `DECISIONS.md`,
`server_settings.yaml` and `player_settings.yaml` in `demos/{hyperbolic_wormholes,dust,snow_planet,city_sphere,dark_forest,warped_match}_rust/`
(Dark Forest's are built into `src/settings.rs`, `src/game/player.rs` and `src/game/weapon.rs`), and the
[wishlist](../../demos/hyperbolic_wormholes_rust/USER_WISHLIST_CHECKLIST.md). Where a level differs from Lunar Surface,
the slide says so in a yellow "Differs" line. The one fact still marked "to confirm" is on slide 17.

## Viewing

Open `index.html` in a browser; it needs no server. Arrow keys, Space and Page Up/Down move between slides, Home and
End jump to the ends, and `#3` in the URL opens slide 3. Printing gives one PDF page per slide, with the videos'
thumbnails in place of their players.

## Videos

Slide 2 embeds two YouTube videos, [`AwuKH4_CE_k`](https://www.youtube.com/watch?v=AwuKH4_CE_k) and
[`B9_w3Jzb7zQ`](https://www.youtube.com/watch?v=B9_w3Jzb7zQ), so it needs a network connection. YouTube refuses to
play them in a page opened as a file (its "Error 153": a `file://` page sends no referrer). To play them in the deck,
serve it locally:

```bash
python3 -m http.server 8137 --directory game_design/slides
```

and open `http://localhost:8137/#2`. Opened as a file, the "Watch on YouTube" link under each player opens the video.

## Pictures

Every picture in `images/` is a real frame of the game, cropped and saved as a JPEG at most 1280 px wide and under
300 KB by a script that wasn't committed (PIL: crop, Lanczos resize, quality 84 or lower).

Rendered for this deck from commit `7d614d8` on an Apple M3 Pro, at 2560×1440, without a window (each level's own
bench views or photos, `--frame-count 1`):

| Picture | Level | View |
| --- | --- | --- |
| `lunar_middle.jpg` | Lunar Surface | `--bench-views`, `middle` |
| `lunar_middle_black_hole_full.jpg` | Lunar Surface | `--bench-views --black-hole-strength 1`, `middle` |
| `lunar_einstein_ring.jpg` | Lunar Surface | `--bench-views`, `einstein_ring` |
| `lunar_overlook.jpg` | Lunar Surface | `--bench-views`, `overlook` |
| `lunar_habitat_row.jpg` | Lunar Surface | `--bench-views`, `habitat_row` |
| `lunar_beacon.jpg` | Lunar Surface | `--bench-views`, `beacon` |
| `lunar_laser_into_wormhole.jpg` | Lunar Surface | `--bench-views`, `laser_into_wormhole`, cropped |
| `lunar_wormhole_exit.jpg` | Lunar Surface | `--bench-views`, `wormhole_exit` |
| `dust_blue_spawn.jpg` | Dust | `--bench-views --map dust`, `blue_spawn` |
| `dust_laser_across_wind.jpg` | Dust | `--bench-views --map dust`, `laser_across_wind` |
| `dust_laser_ground.jpg` | Dust | `--bench-views --map dust`, `laser_ground`, cropped |
| `dust_wormhole_exit.jpg` | Dust | `--bench-views --map dust`, `wormhole_exit` |
| `snow_photo_black_hole.jpg` | Snow Planet | `--bench-views`, `photo_black_hole` |
| `snow_whiteout_outside.jpg` | Snow Planet | `--bench-views`, `whiteout_outside` |
| `snow_photo_oblique.jpg` | Snow Planet | `--bench-views`, `photo_oblique` |
| `snow_laser_ground.jpg` | Snow Planet | `--bench-views`, `laser_ground`, cropped |
| `city_street_up_at_glass.jpg` | City Sphere | `--bench-views`, `street_up_at_glass` |
| `city_street_spawn_stairs.jpg` | City Sphere | `--bench-views`, `street_spawn` |
| `city_glass_reflections.jpg` | City Sphere | `--photos`, `glass_level_crowd_and_reflections` |
| `city_blue_pylon_on_glass.jpg` | City Sphere | `--photos`, `blue_pylon_exposed_on_glass` |

Reused from other agents' captures of 2026-11-08, all real game windows or photos, each checked against the game on
`7d614d8` (the levels' look hasn't changed since):

| Picture | Level | Capture | Commit |
| --- | --- | --- | --- |
| `hud_lunar.jpg`, `hud_lunar_bars.jpg`, `hud_lunar_minimap.jpg` | Lunar Surface | The W-126 audit's window frame, 2560×1600, joined to a local server on blue | `6c9a7e1` |
| `hud_city.jpg`, `minimap_city.jpg` | City Sphere | #429's window captures (HUD after; minimap after, on the street, 2× zoom) | `30e568d` |
| `city_sniper_beam.jpg`, `city_rifle_beam.jpg` | City Sphere | #435's beam looks, after, 2× zoom | #435's head |
| `city_throw_flight.jpg`, `city_throw_ring.jpg`, `city_throw_tint.jpg` | City Sphere | #435's `--route wormholes` window frames | #435's head |
| `city_third_person.jpg` | City Sphere | #435's third person and flashlight frames | #435's head |
| `city_ground_dust.jpg` | City Sphere | #435's frames of a sniper shot into the street | #435's head |
| `forest_hud.jpg`, `forest_hit.jpg`, `forest_console_bar.jpg`, `forest_you_died.jpg` | Dark Forest | #431's `--photo --photo-hud` frames (after) | `129bf20` and later |
| `forest_lift_robot.jpg` | Dark Forest | #426's `lift` bench view, after | #426's head |
| `forest_gate_robot.jpg` | Dark Forest | #426's photo through the centre gate, after | #426's head |
| `snow_blizzard_hit.jpg` | Snow Planet | #421's frames of a player freezing in the blizzard, 21.2 s | #421's head |
| `match_progression.jpg` | The match | `make play-all-levels`, blue's progression page at the end (`--progression-pictures`) | `1cfc3a6` |

Reused from captures of 2026-11-07, before the Milky Way was added to Lunar Surface's sky (the slides say so); what
they show, the thrown wormholes and the colour at a throat, looks the same today:

| Picture | Level | Capture |
| --- | --- | --- |
| `throw_purple_flight.jpg`, `throw_linked_pair.jpg` | Lunar Surface | The thrown wormholes' merge check (#197 to #204): a purple mouth thrown, then the linked pair |
| `throw_pair_hit.jpg` | Lunar Surface | The merge check of shooting an enemy's pair (#246 to #248): the owner sees its pair flash red |
| `wormhole_colour_forward.jpg`, `wormhole_colour_backward.jpg` | Lunar Surface | W-82's recheck after merging staging: forward at the throat, backward at depth 0.79 |
