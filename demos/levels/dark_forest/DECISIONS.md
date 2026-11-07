# Decisions: Dark Forest demo

- **Question:** What could Dark Forest look and play like as one walkable level: the space, the "paired bands" layout,
  where the warp objects go and how they render, with Minecraft-like visuals?
- **Given:**
  - **From the design doc:** red team, depth 1, a dark forest planet themed on bases. T² × ℝ with gravity down, black
    holes and wormholes, a large level, mid-range combat with emphasis on positioning and moving fast through wormholes,
    4 consoles per pylon, and the "paired bands" zone layout.
  - **From the brief:** extend the flat voxel ray tracers (`wormholes.html`, `blackholes_laser.html`) with periodic
    wrapping in x and z, with the voxel grid as one period. Also the shared controls, HUD, views, laser and console
    rules.
- **Outcomes:**
  - **To judge:** a playable layout to argue with, and whether the routes, the gates and the black holes create
    mid-range fights.
  - **Measured:** the frame rate at auto quality (43–60 fps on an Apple M3 Pro, Chrome 156 with ANGLE Metal, macOS
    15.7.3), and the black hole deflection against exact Schwarzschild.

## Space

### D1. Loops of 112 m in x (widthwise) and 160 m in z (lengthwise) **(needs input)**

- **Choice:**
  - **z loop:** 160 m, so spawn to spawn is 80 m each way.
  - **x loop:** 112 m, so each middle band holds three 12–36 m lanes with ridges between them.
- **Alternatives:** square loops (128 × 128); a longer z loop (176–192 m).
- **Rationale:**
  - **Large but mid range:** 80 m spawn to spawn is about 13 s of walking or 8 s of sprinting. The open areas are 36–44
    m wide, which suits 20–40 m fights.
  - **Different periods:** the copies along x and along z don't line up into one square grid.
- **Revisit if:** playtests find crossings too quick for a "large" level.

### D2. Grid height 32 m, bedrock below, endless air above

- **Choice:** the grid is 32 blocks tall; the ground is at y = 5, so ridges, the walkway (y = 14) and canopies (up to
  y = 31) fit. Above the grid rays step straight to its top or escape to the sky.
- **Alternatives:** a taller grid (costs memory and ray steps).
- **Rationale:** nothing playable is higher than the canopy; up "goes on forever" as empty air.

### D3. A ray feels only the nearest copy of each warp object, faded out between 28 and 44 m **(needs input)**

- **Choice:**
  - **Nearest copy:** each object acts through its nearest periodic copy (minimum image).
  - **Fade:** its bending is multiplied by a smoothstep from 1 at 28 m to 0 at 44 m, less than half the shorter loop
    (56 m), so no ray ever feels two copies.
  - **Cost:** close passes behave like an isolated black hole with a 2.8% larger impact parameter. The shadow is about
    3% smaller, checked against the exact deflection to within 0.3–1.2% for b = 3–6 m.
- **Alternatives:**
  - **A sum over nearby copies:** its far field tends to a uniform pull toward the objects' height, like an infinite
    sheet, which bends every sight line in the level.
  - **Fading ln n itself rather than its gradient:** keeps the inner geometry exact, but its steep fall over 16 m made
    visible lens rings 28–44 m from every object (tried and dropped).
- **Rationale:** local lensing stays close to exact and nothing far away is bent, which keeps sight lines readable.

### D4. Fog that thins with height, which hides most of the copies at ground level

- **Choice:**
  - **Density:** 1/62 m⁻¹ up to y = 6 m (the forest floor is at y = 5 m), falling off with a 12 m scale height above
    that.
  - **At eye height:** 40 m keeps about half its contrast, and the first copy along x (112 m) about 17%.
  - **From above the canopy:** you see copies to the horizon.
- **Alternatives:** uniform fog (hides the copies everywhere, including the overview); no fog (mid-range readability
  suffers from far detail).
- **Rationale:** the brief asks for fog that hides most of the copies; the overview should still show that the level
  loops.

## Layout

### D5. Zones: red and blue 32 m deep, each middle band 48 m

- **Choice:** red zone z 144–16, Glade side 16–64, blue zone 64–96, Bog side 96–144. The middle is 60% of the level.
- **Alternatives:** deeper team zones (more room for bases, a smaller middle).
- **Rationale:** the middle zone must be the biggest; 32 m is enough for a road, a courtyard, a spawn bunker and towers.

### D6. 180° rotational symmetry about (56, 40) **(needs input)**

- **Choice:** everything is built for red's half and turned by 180° (x → 112 − x, z → 80 − z) for blue's, with team
  materials swapped. This maps the Glade side and the Bog side each onto itself.
- **Alternatives:**
  - **A mirror:** the same as Snow Planet's layout, but it makes the two middle bands copies of each other.
  - **A translation by half a loop:** this also gives the two middle bands the same layout.
- **Rationale:** it is fair, and the two ways to the other side can differ in character (trenches and an open glade,
  against a bog and a raised walkway), which gives teams a real choice.

### D7. Three routes per middle band, separated by ridges

- **Choice:**
  - **Glade side:** west trench, the Glade (open) and east trench.
  - **Bog side:** the Bog (open), and the forest floor with the canopy walkway above it (two routes stacked).
  - **Separators:** ridges 5–9 m high, covered in trees, which you can't climb or jump.
- **Alternatives:** two routes per band; ridges low enough to jump.
- **Rationale:**
  - **Theme:** the trench bunkers and walkway platforms are red-base structures, and give corners to defend.
  - **Combat focus:** the open areas give 20–40 m sight lines.

### D8. Connectors: a ridge gap from each trench to the Glade, and walkway stairs through a root tunnel to the Bog

- **Choice:**
  - **Glade side:** a 5 m gap at z 28–33 joins the west trench to the Glade, and its twin joins the east trench to the
    Glade at z 47–52.
  - **Bog side:** stairs from the walkway's west platform lead through a root tunnel into the Bog.
- **Alternatives:** connectors at the band centre (Snow Planet's choice); more connectors.
- **Rationale:** each team gets the connector nearer its own side, so rotating between routes is easiest close to home.

### D9. Pylon in a walled courtyard on the road, spawn bunker 24 m away

- **Choice:**
  - **Red:** the pylon at (56, 0) has 4 consoles on the ground 3.5 m from its axis, inside a 16 m walled courtyard with
    four gates. The spawn is at (32.5, 0.5), inside a bunker on the road.
  - **Blue:** the same turned by 180°: pylon (56, 80), spawn (79.5, 79.5).
- **Alternatives:** the spawn and pylon side by side; the pylon off the road.
- **Rationale:** the pylon is "near the spawn" but has its own defended space. The Glade and the walkway both lead
  straight at it, so attackers have two directions.

### D10. Readable darkness: dusk lighting with team-coloured lights

- **Choice:**
  - **Sky:** a dusk sky with a dim gas giant, sky light dimmed by canopies, and a 3.5% ambient floor.
  - **Lights:** red lamps, red moss and banners in the red zone; blue in the blue zone; amber lamps and teal glowing
    plants in the middle. Rows of red (blue) lights mark the zone edges.
- **Alternatives:** night with no sky light; uniform lighting.
- **Rationale:** the brief asks for darker and moodier but still readable; team colours tell you the zone at a glance.

## Warp objects

### D11. Two black holes, r_s = 1 m, over the open areas

- **Choice:**
  - **Placement:** over the Glade (56, 10.5, 40) and the Bog (0, 10, 120), 5.5–6 m above the ground, with a shadow
    radius of 2.6 m.
  - **Accretion disk:** an orange disk at 2.5–6.5 r_s lights the ground.
  - **Players:** they can't get closer than 2.2 m, and the holes don't pull them.
- **Alternatives:** larger holes; holes at eye height (sight lines bent harder, but players walk into them).
- **Rationale:** they bend the open areas' long sight lines without blocking the routes; the disks are landmarks in the
  dark.

### D12. Three wormholes, r_s = 1 m, mouths at eye height above a glowing pit **(needs input)**

- **Choice:**
  - **Glade–Bog gates:** one on each team's side of the middle, switching between the two ways to the other side.
  - **Trench gate:** from the west trench near red to the east trench near blue (20 m forward and across). Each team's
    entrance is the other's exit.
  - **Mouths:** each has bypass lanes more than 4 m from it, so you can pass without going through.
- **Alternatives:** gates at the band centres (both teams reach them at once); gates from the base straight to the enemy
  base (too strong).
- **Rationale:** the gates are the fast routes worth fighting over: holding your side's Glade–Bog gate lets you rotate,
  and the Trench gate exit is a forward position the enemy spawns near.
- **Revisit if:** the Trench gate makes rushes too fast.

### D13. Players cross a wormhole by following the light ray they are on

- **Choice:** within 3 m of a mouth and moving toward it, you follow the light path along your velocity, at your speed
  measured in proper distance and with no gravity. The view turns with the path and levels out after.
- **Alternatives:** a trigger sphere that teleports you (cheaper, but what you see in the mouth and where you end up
  disagree).
- **Rationale:** as in `wormholes.html`, the crosshair shows where you will end up. An off-centre entry flings you as it
  flings light.

## Rendering

### D14. Isotropic refractive-index model for both kinds of warp object

- **Choice:** black holes use the isotropic Schwarzschild index (1 + u)³ / (1 − u) rather than
  `blackholes_laser.html`'s force law in Schwarzschild coordinates. Mouths use the `wormholes.html` foil. A ray is
  absorbed within 2 horizon radii of a black hole, inside its photon sphere.
- **Alternatives:** the force law for holes and the index for mouths.
- **Rationale:** one model makes adding objects' effects consistent; each object alone is still exact.

### D15. Minecraft-style flood-fill light instead of shadow rays

- **Choice:** three 0–15 light channels (sky, warm, cool) are flooded once at load; leaves cost 2 levels.
  - **Brightness:** f / (4 − 3f).
  - **Sampling:** trilinearly, half a block in front of each face.
- **Alternatives:** a shadow ray to each lamp per pixel (much slower).
- **Rationale:** it is Minecraft's look, costs one texture read per hit, and darkens the forest floor under canopies.

### D16. Auto quality renders about 320k pixels and steps down below 40 fps

- **Choice:** the same scheme as `wormholes.html`, with a 320k-pixel budget and a floor of 100k.
- **Rationale:** it runs at 43–60 fps at auto quality on an M3 Pro.

## Movement and play

### D17. Gravity 25 m/s², jump 8.2 m/s, walk 6 m/s, sprint 9.5 m/s **(needs input)**

- **Choice:** a 1.3 m jump (onto one block, not two), falls 1.5 times faster than rises, as in the laser gravity demos;
  fly mode at 12 m/s (30 m/s sprinting). The design leaves the sprint speed TBD.
- **Alternatives:** the laser gravity demos' 30 m/s² with 7 and 18 m/s.
- **Rationale:** close to common shooter speeds; 18 m/s sprinting would cross the level in 4 s.

### D18. Consoles: within 4.5 m of the pylon's axis, facing it within 40°

- **Choice:** hold E to deactivate the next active console in 10 s; letting go resets it; the laser can't fire while
  deactivating; all 4 off shows "Level won".
- **Rationale:** "about 3 m" from the pylon's 4 m plinth; 40° tolerates aiming at any part of the crystal.

### D19. Laser: one shot at a time from the gun, 320 m long, fading over 2 s, no drop

- **Choice:**
  - **Origin:** the gun, 0.3 m ahead, 0.22 m right and 0.2 m below the eye, aimed along the view.
  - **Path:** traced along the true light path; a new shot replaces the old beam.
  - **Gravity:** none, since the design says lasers show no drop.
- **Alternatives:** firing from the eye (the shooter couldn't see their own beam); several beams at once.
- **Rationale:** it matches the design's sniper laser, and the fog hides the beam well before 320 m.
