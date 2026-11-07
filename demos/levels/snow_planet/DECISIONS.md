# Decisions: Snow Planet demo

- **Question:** What could Snow Planet (blue, depth 1, T² × ℝ, paired bands, mid range) look and play like as a
  single-player voxel level with its warp objects, and does the ray-traced torus run fast enough?
- **Given:** the design doc (`game_design/DESIGN.md`) and the shared level brief: T² × ℝ with large loops, gravity down,
  black holes and wormholes that repeat with the space, a large level, mid range, 4 consoles per pylon, paired bands
  with 2–3 routes per middle band, the controls, HUD and testing rules, and extending the voxel ray tracers of
  `three_js_vibecode_demo/` with periodic wrapping in x and z.
- **Outcomes:** a playable demo in `index.html`, numeric checks of the physics (deflection angles, passages through
  mouths, collisions), screenshots of every viewpoint, and a frame-time comparison with `wormholes.html`.

## Scope

### D1. Single player, one laser, no damage

- **Choice:** one player who can switch teams (`T`). The laser is the sniper rifle's beam only: it shows once and fades
  over 2 s. There are no bullets, hits or overheating.
- **Alternatives:** a dummy enemy to shoot.
- **Rationale:** the brief asks for the beam and no damage model.

## Workload

### D2. Loop lengths 176 m lengthwise and 112 m widthwise **(needs input)**

- **Choice:** one period is 176 × 112 m (grid 176 × 24 × 112 blocks of 1 m). The spawns are 88 m apart either way.
- **Alternatives:** 160 × 96, 192 × 128, a square torus.
- **Rationale:** "Large" and mid range suggest 40–60 m middle bands and sight lines of 20–50 m. A longer lengthwise loop
  makes the middle zone the biggest, with room for 3 routes across the widthwise loop.
- **Revisit if:** playtests find the routes too long, or the level too small next to Dust and Lunar Surface.

### D3. Zone lengths: team zones 40 m, each middle band 48 m

- **Choice:** each team zone spans ±20 m around its spawn; each middle band is 48 m long, 96 m in all.
- **Alternatives:** shorter team zones (more middle).
- **Rationale:** the middle zone is the biggest (96 m against 40 m), and each team zone is deep enough for its pylon
  courtyard and widthwise rotation.

### D4. Mirror symmetry between the teams **(needs input)**

- **Choice:** only blue's half (x from −44 to 43) is built, then mirrored through x = 44 and x = 132 with team colours
  swapped. Each middle band is symmetric about its own centre.
- **Alternatives:** 180° rotation symmetry; an asymmetric level that favours blue, its owner ("slightly easier for its
  own team to defend").
- **Rationale:** fairness first, and every route keeps its z position for both teams. The defender's edge from the
  design doc isn't implemented.

### D5. Routes: outpost way and lake way

- **Choice:** +x from blue is the outpost way: outpost huts, an open snowfield, a rock ridge path. −x is the lake way:
  an ice canyon, a frozen lake, a pine forest. Rock ridges 8–10 m high separate the routes and taper off in the team
  zones, so each team can rotate widthwise there. An 8 m gap at each band's centre connects the first route to the big
  open area.
- **Alternatives:** the same three routes on both ways.
- **Rationale:** the two ways should feel different and use the theme (snow, ice, lake, pines, rock), while each has a
  big open area joined to another route, as the design doc asks.

### D6. Spawns and pylons

- **Choice:** each spawn pad is 19 m from its pylon, in the same team zone. Each pylon is a glowing 2 × 2 × 7 m column
  in a courtyard with 2 m walls, ice windows and four openings. Its 4 consoles are 2 × 2 m panels on its four sides.
- **Alternatives:** the pylon right at the spawn.
- **Rationale:** close enough to defend, far enough that attackers can reach it. The openings give corners to defend.

### D7. Two wormholes, both linking the two ways **(needs input)**

- **Choice:** W1 links the outpost courtyard to the ice canyon (z = 15), W2 the top of the rock ridge to the pine
  forest (z = 93). Mouths have r_s = 1.6 m, with their centres at eye height above the floor.
- **Alternatives:** a wormhole between two routes of the same way (as in the slides' sketch); one linking the team zones.
- **Rationale:** without wormholes, the two ways only meet in the team zones, 88 m apart. Mouths at the band centres are
  equally far from both spawns, so they are fast routes worth fighting over.

### D8. Every mouth faces +z

- **Choice:** both mouths of each wormhole face +z, so the gluing map is diag(1, 1, −1) and walking +x into a mouth on
  one way comes out moving −x on the other, that is, still toward the same enemy.
- **Alternatives:** mouths facing ±x, which would send players back toward their own spawn.
- **Rationale:** a fast route should keep you heading where you were going.

### D9. Two black holes, over the open areas **(needs input)**

- **Choice:** r_s = 1.5 m at (44, 7, 59) over the snowfield, and r_s = 1.8 m at (132, 7.5, 59) over the frozen lake.
- **Alternatives:** holes at head height; more, smaller holes.
- **Rationale:** they bend the long sight lines across the open areas (mid range), and they stay out of players' reach.

### D10. Ice is see-through but solid

- **Choice:** ice blocks let 62% of the light through each entry face, but stop players and the laser.
- **Alternatives:** ice that the laser passes; opaque ice.
- **Rationale:** like City Sphere's glass floor: you see enemies through the ice walls of the canyon but can't shoot
  them through it.

### D11. Movement numbers **(needs input)**

- **Choice:** walk 6 m/s, sprint 10 m/s, gravity 30 m/s² (1.6 times stronger when falling), jump 8.5 m/s (a 1.13 m
  hop), automatic single-block steps, eye height 1.6 m. Fly mode moves at 14 or 36 m/s and sprints without energy.
- **Alternatives:** the design's open question on sprint speed.
- **Rationale:** walking speed is close to tactical shooters'; gravity and the jump come from the laser gravity demos.

### D12. Near a mouth, you move along light paths

- **Choice:** within 2.5 r_s (4 m) of a mouth, each step follows a light path and gravity is off. Each frame starts
  from your walking direction, and your view turns with the path.
- **Alternatives:** a teleport trigger at the mouth.
- **Rationale:** this is how you go through a throat at all, since its sphere is only 0.4 m across. Walking within about
  2.25 m of the centre line takes you through, but you come out turned by about 90° per metre off centre (the path
  winds round the throat first). Gameplay may need that straightened **(needs input)**.

### D13. Players aren't pulled by black holes

- **Choice:** no pull. A player can't reach a hole on foot, and a flying player who enters a horizon sees black.
- **Alternatives:** a game-scaled pull.
- **Rationale:** as in the laser gravity demos.

### D14. Pylon reach and facing

- **Choice:** E works within 3 m of the pylon's 8 × 8 m footprint, while facing within 45° of its centre. It works on the
  active console nearest you. Walking or looking away resets that console's progress, as letting go does.
- **Alternatives:** consoles in a fixed order.
- **Rationale:** working on the console you stand next to reads naturally.

### D15. Laser timing and origin **(needs input)**

- **Choice:** a shot every 0.4 s at most, up to 4 beams shown at once, 400 m range. The beam starts 0.22 m right of and
  0.2 m below the eye, parallel to the view, and takes the shooter's team colour.
- **Alternatives:** the sniper rifle's ≈2 s rate; starting the beam exactly at the eye (it would show as a dot).
- **Rationale:** a faster rate makes the demo quicker to explore.

## Procedure

### D16. Rendering: the wormhole ray tracer, wrapped, with free-box steps

- **Choice:** extend `wormholes.html`'s ray tracer: wrapped block lookups, black holes added to the bending, and empty
  space crossed in boxes sized by a precomputed max-norm distance to the nearest block. The ground layer is left out of
  that distance and treated as a plane.
- **Alternatives:** a plain cell-by-cell walk, which is too slow over 300 m rays.
- **Rationale:** rays in a torus never leave the grid sideways, so they need a cheap way through open space.

### D17. Nearest copy only, fading out between 20 and 32 m **(needs input)**

- **Choice:** a ray feels only the nearest periodic copy of each warp object, and that bending fades smoothly to zero
  between 20 and 32 m. The effects of several objects are added, as in the demos.
- **Alternatives:** summing a few nearby copies; the full 1/r² tail out to half a loop (34–50 m).
- **Rationale:** the sum over a 2D lattice doesn't converge, and with the long tail the far copies swirled the whole
  horizon. Deflections stay within 1.4% of the exact value for rays passing within 10 m, and within 6% at 15 m.
- **Revisit if:** a ray's deflection at 15–30 m matters for gameplay.

### D18. Height fog: 130 m at the ground, 24 m scale height

- **Choice:** fog that thins with height. Through it, a copy one widthwise loop away (112 m) keeps about 42% of its
  contrast, and one lengthwise loop away (176 m) about 26%.
- **Alternatives:** uniform fog, which also washes out the overview.
- **Rationale:** the nearest copies read clearly and the rest fade, and the high viewpoints stay clear.

### D19. No gravitational redshift

- **Choice:** colours are not red- or blueshifted near warp objects.
- **Alternatives:** the demos' shift.
- **Rationale:** with these small objects, the shift stained the snow round them brown and flashed the screen blue in
  a throat, which hurts readability without changing any light path.

### D20. Snowfall as a screen overlay

- **Choice:** four layers of flakes fixed to view directions, hidden behind anything closer than their depth.
- **Alternatives:** ray-traced flakes.
- **Rationale:** cheap and atmospheric. It plays no part in the level.

## Environment

### D21. Tested in headless Chrome with SwiftShader **(needs input)**

- **Choice:** three.js 0.160.0 from the CDN, WebGL2, auto resolution as in `wormholes.html` (about 380k pixels, lowered
  while under 40 fps). Frame time measured only in headless Chrome on the CPU: 200–450 ms per frame at 640 × 360,
  against 644 ms for `wormholes.html` at the same size.
- **Alternatives:** measuring on an Apple-silicon GPU, which wasn't available to the build.
- **Rationale:** the comparison shows it does less work per pixel than a demo that runs at about 40 fps.
- **Revisit if:** auto quality drops below a useful resolution on a real Mac.

## Analysis

### D22. Numeric checks through `window.__demo`

- **Choice:** checks run in the page:
  - deflection angles at b = 5–15 m against the exact Schwarzschild integral;
  - rays into W1 and W2, and into a copy of W1 one loop away, coming out of the partner mouth;
  - walks into each mouth;
  - a wall stop, the ridge stairs and the jump height;
  - energy drain and refill rates;
  - deactivating every console.
- **Alternatives:** separate unit tests (the demo is one HTML file).
- **Rationale:** the checks drive the same code the demo runs.
