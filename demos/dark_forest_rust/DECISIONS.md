# Dark Forest decisions

Decisions made without the user, with the reason for each. **Weigh in** marks the ones the user should check.

- **D1. A bounded forest, no looping.** The WebGL version wraps around (T² × ℝ). The user asked for "just kind of a
  normal first-person shooter", so the forest is ordinary flat space with rock walls at its edge. **Weigh in:** a
  wrap-around forest is possible later, but needs every view and shot to cross the seam.
- **D2. No black holes or wormholes.** For the same reason; the WebGL version's black holes, wormholes and canopy
  walkway are replaced by trees, decks, ladders and bridges.
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
- **D9. Daylight, not dusk.** The FPS demo's HDRI sky with light fog (70 m) under dark green canopies; the WebGL
  version is at dusk. **Weigh in:** a dusk sky is a texture swap.
- **D10. 1K Poly Haven textures, downloaded.** CC0, 10 MB in all, so the demo downloads them like the FPS demo
  instead of committing them.
