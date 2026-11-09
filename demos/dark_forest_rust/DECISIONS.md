# Dark Forest decisions

Decisions made without the user, with the reason for each. **Weigh in** marks the ones the user should check.

- **D1. A bounded forest, no looping.** The WebGL version wraps around (T² × ℝ). The user asked for "just kind of a
  normal first-person shooter", so the forest is ordinary flat space with rock walls at its edge. **Weigh in:** a
  wrap-around forest is possible later, but needs every view and shot to cross the seam.
- **D2. Five wormholes, no black holes.** The user asked for "a bunch of wormholes" and no black hole. The map has
  five fixed pairs: three ground gates and a lift per team from the ground to its own treehouse deck, so both sides
  get the same shortcuts. The black-hole code stays, with a count of 0.
- **D3. The FPS demo's rasterizer.** Light travels straight here, so the Metal PBR rasterizer of `../fps_demo/`
  (shadows, SSAO, bloom, fog) is correct and much cheaper than the hyperbolic demo's ray tracer.
- **D4. Ladders as non-solid volumes in the map.** A new optional `ladders` field (a frame and half sizes) keeps the
  map in `common/maps/` and lets other readers ignore it. A player climbs by walking into a ladder, as in most
  shooters; there is no "use" key. **Weigh in:** climb speed (3.2 m/s) and whether you can shoot while climbing
  (you can).
- **D5. Small map, two heights.** 60 m by 112 m, decks at 7 m and a lookout at 12 m, to have it playable quickly;
  the two bases are mirror images by a half turn so neither side is favoured.
- **D6. Two pylons per team, one on the ground, one in the treehouse.** `game_design/DESIGN.md` gives the count; the
  placement makes a team both hold its ground and climb. Holding `E` for 3 s within 2.5 m turns a console off (the
  hyperbolic demo's defaults).
- **D7. The hyperbolic demo's network protocol, unchanged.** Copying `net.rs` as is (wormhole fields always empty)
  kept multiplayer working quickly and keeps the bytes compatible with that demo's tools. A later refactor should drop
  the wormhole fields.
- **D8. Robots in team colours, no new models.** The FPS demo's robot is tinted blue or red for other players, as the
  user allowed reusing the robot skins and guns.
- **D9. Dim, swampy daylight.** The FPS demo's clear HDRI is graded murky green with a weaker sun, darker exposure and
  thick green haze, and the bamboo canopy's shadows block most sunlight, as the user asked. Its sun is baked into the
  sky photo, so it can't be lowered. **Weigh in:** a dusk or overcast sky is a texture swap.
- **D10. 1K Poly Haven textures, downloaded.** CC0, 10 MB in all, so the demo downloads them like the FPS demo
  instead of committing them.
- **D11. The labyrinth is a seeded maze on 4 m cells.** A depth-first tree that prefers turning (70%) gives winding
  corridors with dead ends; opening 12% of the remaining walls adds loops, and 35% of the halfway walls are open.
  Red's half is generated and turned half a turn for blue's, so the halves are equal. **Weigh in:** cell size,
  loop share and wall density.
- **D12. The wormholes' far side is a cube atlas drawn at start-up.** Six 90° views of the static world from each
  mouth's centre (384 px faces), sampled where a ray leaves the lens, lined up on the floor and a 12 m sphere. A
  second live view per mouth would also show robots and lasers there but costs a full scene render per visible mouth
  each frame. **Weigh in:** whether moving things must show through wormholes.
- **D13. The lens's throat is half the walk-in throat.** The foil lens uses the hyperbolic demo's ρ = r_s/4, so rays
  that pass within about the walk-in throat (r_s/2) cross; bodies, lasers and light turn the same way through it
  (inversion, then diag(1, 1, -1)), so you come out facing what you saw.
- **D14. Compact map JSON.** Every culm and leaf stays listed (the format has no generator), so the map is written
  without indentation: 8.6 MB instead of about 20 MB, under the 10 MB limit.
- **D15. Robots and lasers show through the wormholes from live cube faces, found by depth.** Each frame, up to 4
  live slots redraw only the robots and laser effects (beams, the robots' eye glows, sparks, smoke) within 50 m of
  a mouth into six 384 px faces from its centre, with depth: for each visible mouth, nearest first, its partner and
  then itself. The lens pass then looks for a robot along the traced ray, at each bent step and across each robot's
  bounding sphere on the straight part, by comparing the ray's distance from the mouth with the faces' depth, and
  hides a robot behind the static forest with the atlas's alpha, now the static world's distance (log-encoded).
  Lasers and glows are shown where the traced ray passes them: their segments (up to 24, beams' dashes joined) go
  to the lens pass, which reads the live faces in the direction of the nearest point on the ray (W-129). Redrawing the whole forest per visible mouth (D12's live view)
  would cost about a full frame each; this costs only the robots. The parked branch `devin/dark-forest-lens-robots`
  drew robots over the atlas in the atlas's direction, so they slid with parallax and showed through bamboo walls;
  it was redone rather than finished. Robots in the live faces are lit with the player's last shadow cascade (which
  covers about 160 m around the view), not a cascade of their own. Pylons and their beams stay out of the live
  faces, as before (they aren't in the atlas either). The user confirmed this choice on 2026-11-08: "these choices
  you want me to confirm is fine".
- **D16. Lunar Surface's play is ported into this engine, not this level into Lunar's** (the lead's call for W-126,
  same play in every level; the user may overrule it). Lunar, Dust, Snow Planet and City Sphere share a voxel ray
  tracer in hyperbolic space; rebuilding the forest there would lose its rasterized bamboo, canopy shadows, light
  shafts and ladders. So Lunar's play code is copied here whole wherever it doesn't need the voxel world, starting
  with the HUD: `hud_bars.rs`, the overlay rectangles and their shader (`render/overlay.rs`, `shaders/overlay.metal`),
  the crosshair, the console bar, the hit effect and the "YOU DIED" banner, with Lunar's numbers (sprint energy
  7.5 s to empty, 7.5 s crouched and 15 s upright to refill; the hit effect's 0.6 s, red 0.6, 6 px shake and blur;
  the death dim 0.55). The forest's own health bar, ammo digits and pips, reload bar, kill counter and hit markers
  went with it, since Lunar shows none of them. The console lights at the top of the screen stay; the user confirmed
  this choice on 2026-11-08: "these choices you want me to confirm is fine".
- **D17. The server checks every console hold, as Lunar Surface's does (#434).** A hold counts only from a player whose
  view meets the console's box within the settings' 3 m reach (`console_reach.rs`, both grown by 0.5 m for the pylon
  model inside the box and the eye's movement between frames), and lasts Lunar's 10 s. Walls in between don't count,
  as on Lunar. A window fills its console bar only for such a hold, so what the player sees is what the server takes.
- **D18. The console bot walks with a player's own movement along the robots' paths.** `console_bot.rs` follows
  `nav.rs`'s grid of standing spots and climbs its ladders: up by lining up in front of a ladder and walking into it,
  down by stepping off the deck onto it and holding crouch, and it keeps jumping at a ladder's foot until it is out,
  since a ladder grabs anyone in the air inside it. It heads for any spot within reach of the console
  (`NavGraph::find_path_near`), not the console itself: in each treehouse a bamboo clump leaves the pylon hut's door
  0.93 m wide, too narrow for the robots' grid (0.9 m bodies) but wide enough for a player (0.6 m), so the bot holds
  `E` from 3.3 m on the deck, through the hut's wall, which D17's rule allows. **Weigh in:** whether to widen that door
  (a seeded, mirrored map change) so bots and players stand in it. From every spawn of both teams, a bot of each team
  reaches every enemy pylon in turn (`test_bots_walk_to_every_enemy_pylon`); online, a bot of either team turned off
  both enemy consoles and won in about 58 s.
- **D19. Bots run in a hidden window that draws nothing.** A `--bot` loads the scene (its collision and paths need it)
  but skips the wormhole lens capture and every frame's drawing, and steps its game 60 times a second, so a match's
  bots cost little GPU and put no window on the screen.
- **D20. Both of Lunar Surface's guns, drawn with the forest's particles.** `guns.rs` ports Lunar's gun code (the
  sniper's click, shot and reload, the rifle's fire rate, heat and recoil, switching, aim zoom, the gun's pose, head
  bob and sway, each beam flying out at 250 m/s, the hit weights) with Lunar's numbers in `settings.rs`, checked
  against Lunar's own `server_settings.yaml` and `player_settings.yaml`; `shot_noise.rs` and `dust.rs` are Lunar's,
  in flat space. The guns in hand are Lunar's models (the bolt-action rifle for the sniper, the revolver for the
  rifle), placed by Lunar's `make_wand`, and the view is Lunar's 75°. Lunar's beam looks (the sniper's blade, the
  rifle's laser) and their speckle need new particle shading in `src/shaders/`, which another agent owns now, so the
  beams are drawn as the forest's tracers (the shot's speckle share rides in the particle's seed for that later
  change) and the dust's grains as smoke. Where a beam lands, the forest's sparks and scorch marks stay, timed to the
  beam's arrival. The AI robots' bolts fly as rifle beams.
- **D21. Lunar Surface's jump, and aiming no longer slows.** A jump rises 1.25 m and stays 0.58 s in the air
  (Lunar's `jump_height_m` and a third of its map's jump time), so gravity is 30 m/s², tested against Lunar's own
  settings and map; aiming no longer slows the player or stops a sprint, as on Lunar.
- **D22. Joining a team puts the player at that team's spawn.** A window started at the red spawn and stayed there
  after the server put it on blue, so a blue player began among red's pylons; it now respawns at its team's spawn as
  soon as the server names the team, and a bot waits for that before it moves.

## Follow-ups for the next agent

- **Ground mist:** dropped by the user; don't add it.
- **Benchmarks:** at 2560x1440 (1280x720 pt at render scale 2) every `--bench-views` view has
  a median of 16.6-16.7 ms (vsync) and a GPU mean of 10.6-12.4 ms on the Apple Paravirtual
  GPU of a macOS VM; p99s of 20-36 ms are drawable waits on that VM.

