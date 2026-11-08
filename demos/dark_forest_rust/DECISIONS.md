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
  Lasers are added in the atlas's own direction. Redrawing the whole forest per visible mouth (D12's live view)
  would cost about a full frame each; this costs only the robots. The parked branch `devin/dark-forest-lens-robots`
  drew robots over the atlas in the atlas's direction, so they slid with parallax and showed through bamboo walls;
  it was redone rather than finished. Robots in the live faces are lit with the player's last shadow cascade (which
  covers about 160 m around the view), not a cascade of their own. Pylons and their beams stay out of the live
  faces, as before (they aren't in the atlas either). **Weigh in:** whether pylons and their beams should also
  show through the wormholes.

## Follow-ups for the next agent

- **Ground mist:** dropped by the user; don't add it.
- **Server console count:** `map::console_teams` once read boxes only, so the server saw 0
  consoles (the pylons are `scifi_beacon` mesh structures) and no online match could be won. It
  now counts mesh-structure console shapes in `game::world`'s order; rerun the two-bot match
  (`--serve --level-result-file`, two `--join --bot` windows, one with `--bot-objective
  consoles`) to confirm a win end to end.
- **Benchmarks:** at 2560x1440 (1280x720 pt at render scale 2) every `--bench-views` view has
  a median of 16.6-16.7 ms (vsync) and a GPU mean of 10.6-12.4 ms on the Apple Paravirtual
  GPU of a macOS VM; p99s of 20-36 ms are drawable waits on that VM.

