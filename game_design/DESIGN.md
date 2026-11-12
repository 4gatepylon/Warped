# Warped game design

This document formalizes the notes in [`brainstorm/USER_THOUGHTS.md`](brainstorm/USER_THOUGHTS.md).
Every level and every weapon is described by the same parameters: the rows of its table. To add a
level or a weapon, add a column and fill in every row. To add a parameter, add a row and fill it in
for every column.

**Source of truth:** the Rust game in
[`demos/hyperbolic_wormholes_rust/`](../demos/hyperbolic_wormholes_rust/README.md). Where it builds
a rule, this document gives the game's rule and its default values, which come from
[`server_settings.yaml`](../demos/hyperbolic_wormholes_rust/server_settings.yaml) (numbers that
shape play, the same for everyone in a game) and
[`player_settings.yaml`](../demos/hyperbolic_wormholes_rust/player_settings.yaml) (each player's own
keys and view). When the game changes, change this document with it. Its
[wishlist](../demos/hyperbolic_wormholes_rust/USER_WISHLIST_CHECKLIST.md) tracks what is asked for
and in progress.

- **≈** marks a first guess, to be tuned in playtests.
- **TBD** marks something not designed yet.
- **Not built yet** marks a rule that is designed but that the Rust game doesn't have yet.

> NOTE: Almost every number in this document is configurable, such as consoles per pylon, the time
> to deactivate a console and pylons per team per level, and may change after playtests. The
> default map won't be the only map either: maps will be configurable too, though not yet.

## Terms

[`GLOSSARY.md`](GLOSSARY.md) holds the official terms of the game, one meaning each. Read it first. This document
still says *game* in places for what the glossary calls a *match*, and *map edge* for a *level edge*; fix those
when you edit them.

## Match

### Map

The default map has five levels:

```text
              blue                     purple                     red
Dust ─────── Snow Planet ─────── City Sphere ─────── Dark Forest ─────── Lunar Surface
depth 2      depth 1             depth 0             depth 1             depth 2
```

- A map has an odd number of levels: about half blue, about half red, and exactly one purple.
- The deeper a level, the bigger and more complex it is, and the slightly easier it is for its own
  team to defend. A team with more than two levels ranks them the same way, by depth.
- Every level is space themed. Blue levels are themed on nature and desolation, red levels on
  bases, and the purple level on a city or space station.
- **Not built yet:** the Rust game plays one level per game, Lunar Surface by default. After a win,
  the server's operator presses `Enter` to start a new game on the same level, with every console
  back on and every player's kills and deaths at 0.

### Winning a level

- Each team has a spawn and, near it, its pylons: 3 on Dust and Lunar Surface, 2 on Snow Planet and
  Dark Forest, and 1 on City Sphere, so the count rises from the middle level outward. The two spawns are on opposite sides of the level.
- A team wins a level by deactivating every console on the opposing team's pylons, as in capture
  the flag. The [wavefront](#wavefront) then decides which level is played next.
- A pylon with more consoles takes longer to deactivate.
- A console pulses with light once a second while it is on. While someone holds `E` on it, it
  pulses faster and faster, up to 8 times a second just before it turns off, and eases back down
  over about a second if the hold stops. It goes dark once it is off.
- A player can only deactivate the other team's consoles.
- What deactivating a pylon does, besides bringing its team closer to losing, varies per level: on
  Lunar Surface it grows the black hole (see [Lunar Surface](#lunar-surface)); on the other levels
  it is TBD.

| Parameter | Value |
| --- | --- |
| Pylons per team per level | Set per level: 3 on Dust and Lunar Surface, 2 on Snow Planet and Dark Forest, 1 on City Sphere (see [Levels](#levels)) |
| Consoles per pylon | 1 on every level |
| Control to deactivate a console | Hold `E` while aiming at it from within 3 m |
| Time to deactivate one console | 10 s |
| While deactivating | The player must keep aiming at the console and can't shoot. |
| Letting go of `E`, or aiming away | Resets that console's progress to zero. |
| Several players on one console | Each player's hold is timed on its own; holds don't add up. |
| A deactivated console | Stays off until the next game. |

### Wavefront

**Not built yet.** Each team defends its own levels and attacks the other team's. A match is a
sequence of rounds:

1. The first round is on City Sphere, the only level that is played exactly once. Its winner
   becomes the attacker, and the loser the defender.
2. Each team's wavefront starts at the other team's depth 1 level. Every later round is on the
   attacker's wavefront.
3. If the attacker wins, its wavefront moves one level deeper and the next round is there. An
   attacker that wins the defender's deepest level wins the match.
4. If the defender wins, the teams swap roles, and the next round is on the new attacker's
   wavefront, where it left off.
5. A match lasts at most 30 minutes. If no team has won by then, the match is a tie.

Each team's wavefront only moves deeper, and every level but City Sphere can be played many times.
For example:

| Round | Level | Attacker | Winner | Next round |
| --- | --- | --- | --- | --- |
| 1 | City Sphere | None | Blue | Blue attacks its wavefront, Dark Forest. |
| 2 | Dark Forest | Blue | Blue | Blue's wavefront moves deeper, to Lunar Surface. |
| 3 | Lunar Surface | Blue | Red | Red attacks its wavefront, Snow Planet. |
| 4 | Snow Planet | Red | Blue | Blue attacks again, at Lunar Surface, where it left off. |
| 5 | Lunar Surface | Blue | Blue | None: blue won red's deepest level, so blue wins the match. |

### Joining and teams

- A player joins a game server and lands in the lobby, which shows each team's head count. `1`
  joins blue and `2` joins red. A player can't shoot until the server has let them in.
- A game server on the internet has a password, which its operator shares with the players; the server admits only
  players who prove they know it, and the password itself never crosses the network.
- Friendly fire is a game setting, on by default (`shooting.is_friendly_fire_on`, W-156 (friendly fire, by game
  setting); the user: "add setting in game settings to toggle whether or not friendly fire is possible"). When on, a
  shot eliminates a teammate as it would an enemy, but only an enemy counts as a kill; when off, a shot doesn't
  hurt teammates. A bot never hurts a teammate, whatever the setting: a bot whose laser meets a teammate holds its
  fire and steps aside (W-155 (play against bots)).
- The game counts each player's kills and deaths, shown on the scoreboard (see [Screen](#screen)).

### Respawning

A player who is eliminated sees their view darken with "YOU DIED", a countdown and a bar that fills
over the wait, and comes back after the respawn time on a ring around their team's spawn, never on
top of another player, with full health and energy. A player can't choose to respawn early.

The respawn time grows the longer a level goes on, so dying costs more later (W-142 (growing respawn
time). In the user's words: "Respawns will happen on a cadence, and every n steps, it will increase in
time linearly so that it takes longer to respawn as time progresses. This means that as time
progresses, the stakes get basically higher to dying versus earlier on. The respawn rates should
start at you being stuck dead for 20 seconds, and they should go up linearly every five minutes by 10
seconds on a given level in the map." A player eliminated `t` seconds after the level (its round)
started waits `start_s + step_s × floor(t / period_s)`; the level's server decides the wait and sends
it with the elimination, so the countdown and bar show the server's wait. A new game on the level
starts the clock again.

| Parameter | Value |
| --- | --- |
| Respawn time at the level's start | 20 s (`respawn.start_s`) |
| Added every period | 10 s (`respawn.step_s`) |
| Period | 5 minutes (`respawn.period_s`) |

## Levels

| Parameter | Dust | Snow Planet | City Sphere | Dark Forest | Lunar Surface |
| --- | --- | --- | --- | --- | --- |
| Team | Blue | Blue | Purple | Red | Red |
| Depth | 2 | 1 | 0 | 1 | 2 |
| Theme | Dust (desert) planet | Snow planet | City, or space station | Dark forest planet | Lunar base in a crater |
| Space | Hyperbolic | T² × ℝ | Inside a sphere | T² × ℝ | Hyperbolic, curvature radius 26.7 m (the map's 40 m, curved 1.5 times as much by `space.curvature_per_map_curvature`) |
| Gravity | Down, onto the ground | Down | Outward, onto the sphere | Down | Down, onto the ground, ≈30 m/s² |
| Warp objects | Yes | Yes | No | Yes | 1 black hole, 1 wormhole |
| Size | Very large | Large | Smallest | Large | Very large: 256 m × 144 m playable |
| Pylons per team | 3 | 2 | 1 | 2 | 3 |
| Consoles per pylon | 1 | 1 | 1 | 1 | 1 |
| Combat focus | Long range | Mid range | Close quarters | Mid range | Long range |
| Zone layout | Open | Paired bands | Bands | Paired bands | Open |
| Detailed layout | TBD | TBD | See [City Sphere](#city-sphere) | TBD | See [Lunar Surface](#lunar-surface) |
| Built in the Rust game | No | No | No | No | Yes |

### Spaces

- **Inside a sphere:** flat space, but players walk on the inside of a hollow sphere and gravity
  pulls them outward onto its surface. With no warp objects, light travels in straight lines.
- **T² × ℝ:** flat space whose two horizontal axes loop back around (a torus) and whose vertical
  axis goes on forever. The loops are large. The warp objects curve space around them.
  - **Caveat, work in progress:** the levels described as toroidal here (Snow Planet and Dark
    Forest) are not actually toroidal yet. In the Rust game both are plain flat (Euclidean) space
    whose edge is a [map edge](#map-edge) that slows players without end; looping them around is a
    work in progress (Snow Planet's looping world waits on its own branch).
- **Hyperbolic:** hyperbolic space with warp objects. The pylons are far apart. Gravity pulls
  players down onto the ground, along the ground's normal under them; players never get far above
  the ground, so that simple rule is enough. Lunar Surface is a bounded patch of this space: its
  ground is tiled by squares with five at each corner, and a crater's wall is its
  [map edge](#map-edge). The topology of the other hyperbolic levels is TBD.

### Combat focus

- **Close quarters:** walls and corners, short sight lines.
- **Mid range:** positioning, and moving quickly through wormholes.
- **Long range:** long sight lines across a very large space.

### City Sphere

- Each team has one pylon, on the glass level in an exposed spot with no cover around it.
  The red spawn and pylon are blue's reflected through the sphere's center, so the two sides sit
  at antipodes.
- Walls divide the space into close quarters.
- City Sphere has two surfaces to walk on: the floor, the sphere's surface, and the glass level,
  the surface of a smaller sphere with the same center, which players walk up to. Gravity pulls
  outward on both.
- The glass level is glass and roofs the whole floor. Players on the floor and on the glass level
  see each other through it but can't shoot through it.
- City Sphere may change to the floor alone, or to a roof that is only partly glass.

### Lunar Surface

The level as the Rust game builds it, from the map
[`common/maps/lunar_surface/`](../common/maps/lunar_surface/map.json):

- A lunar base on a regolith plain inside an elliptical crater, with hills and mounds in the
  ground. The playable area is an ellipse 256 m long and 144 m wide; the crater's wall around it is
  the [map edge](#map-edge).
- The zones are bands across its length: blue more than 45 m from the middle on blue's side, red
  more than 45 m on red's side, and the middle zone, 90 m long, between them. Each team zone's edge
  is a faint see-through wall in the team's colour, 6 m high and 0.6 m thick, that stops neither
  players nor lasers.
- A black hole hangs about 7.5 m above the middle of the level (it bends light out to 24 m; its pull reaches 10 m, so about 8 m around the spot under it on the ground). A player who falls into
  it goes back to their spawn.
- Each console turned off, on either team's pylons, grows the black hole over 4 s, up to full strength with one
  console left. Its black part grows only a little (Schwarzschild radius 1.2 m to 2 m), while the area it distorts
  grows a lot: its reach grows to 28 m, and a wide lens around it bends light across that whole area.
- Time runs slower near the black hole, as in general relativity but 15 times stronger: each player's own
  movement, jumps and falls, sprint energy, turning off a console, and firing, reloading and cooling their guns
  slow down, never below 10% speed. Looking around, menus and respawning keep normal time. Other players see a
  slowed player move slowly. On the ground under the black hole, time runs at about 81% with no console off, 45%
  with three off and 25% with five. The HUD shows the black hole's strength and how fast the player's time runs.
- One wormhole joins a ridge to the red landing pad. Its mouths are 15 m in reach.
- Each player throws their own pair of wormhole mouths ([Thrown wormholes](#thrown-wormholes)): `Q` throws the
  purple one, then the green one, and nothing more once both are down. A lone mouth glows as a ball until its
  partner lands; shooting any mouth does nothing to it.
- Blue's side has a concrete bunker (a kazemat) at its spawn, rows of Hesco barriers, a relay mast, supply crates and
  a solar array. The middle has derelict field bunkers, a comms dome, a crater walkway, a relay mast, a solar array
  and crates. Red's side has Hesco barriers, a habitat module, a dome, a bunker garage as its spawn hangar, a
  watchtower, a landing pad, an antenna, a walkway, crates and a solar array. The bunkers are models players walk
  into. Each pylon is a 12 m sci-fi beacon whose console is the beacon itself; it beams its team colour 400 m up while
  its console is on.
- Players look like a robot in their team's colour (blue or red, its grey and black parts unchanged), animated by
  what they do: standing, walking, running, in the air, and, once eliminated, falling onto their back where they
  were hit and lying there until they respawn. Shots hit an invisible
  cylinder and head the robot's size, not the robot's own shape.
- Structures have sharp edges. The habitat modules' and walkways' windows are glass: players see
  out of them and shoot through them, but can't climb through them.
- The sky has the sun, a planet, stars, and spaceships that fly over, purely for looks, on the same
  clock for every player; far behind them hang a moon-sized battle station and a long capital ship.
- Lighting has soft sun shadows, lamps, light from the planet, a thin haze and bloom.

## Zones and routes

Directions in a level:

- **Lengthwise:** from one team's spawn toward the other's.
- **Widthwise:** along the ground, at right angles to lengthwise.

Every level follows these rules:

- A level splits lengthwise into three zones: the blue zone around blue's spawn and pylons, the
  middle zone, and the red zone around red's. The middle zone is always the biggest.
- The middle zone has 2–3 routes across it lengthwise. A route is a sequence of rooms and/or paths,
  with corners to defend.
- On most levels, one route is a big open area, joined to at least one other route by a connector.
- The blue and red zones are easy to cross widthwise, so a team can rotate quickly between routes.
- Each team zone's edge is a team boundary plane in every level (W-148, team boundary planes in every level): a
  faint see-through wall that glows slightly in the team's colour, brightest at its foot, and stops neither
  players nor lasers. What is seen through one shimmers like air over hot ground (W-149, shimmer through boundary
  planes): the picture behind it wobbles a little, rising and changing all the time, in every level alike. The
  shimmer is only where a wall covers the view, so it stops once you have crossed the wall and look away from it.
  The boundary takes the shape that fits the level's space: planes across the other levels' length, and in City
  Sphere, inside a sphere, a dome over each team's pylon on the glass level. A team's area there is inside its dome
  and everything else is the middle; each dome spans 0.2 of the way round the glass level (21.4 m across), which the
  game master can change (`team_domes.circumference_frac`), and it is fainter seen from inside than from outside.
- City Sphere's glass level is lit by the glowing ribs of its glass shell, which pulse (W-203, dimmer, pulsing
  lights on City Sphere's glass): from 0.3 to 0.7 of their full brightness and back every 16 s, rising for 10 s
  and falling for 6 s. The game master can change the two bounds, the period and the rising share (`glass_lights`).

| Zone layout | Shape |
| --- | --- |
| Bands | The spawns sit at antipodes. The middle zone is a wide band around the sphere, halfway between them, and the blue and red zones are thinner bands on either side of it. (City Sphere's team areas are now domes over the pylons, above; its spawns are in the middle.) |
| Paired bands | The zones are bands across one looping axis. Going all the way around that axis passes every zone twice, so each zone is a pair of bands: the two halves of a team's zone meet at its spawn, and the two middle bands are two separate ways to the other side. |
| Open | The zones are bands across the level's length, with the spawns far apart, and walls, obstacles and hills shape the routes. |

## Movement

Speeds are set per level as a multiple of the level map's own speeds (`movement` in
`server_settings.yaml`). On Lunar Surface:

| Parameter | Value |
| --- | --- |
| Walking speed | 10 m/s (twice the map's 5 m/s) |
| Sprint speed | 18 m/s (twice the map's 9 m/s) |
| Crouched speed | Half the walking speed |
| Crouched eye height | 0.6 of standing, reached in 0.15 s |
| Jump height | 1.25 m: high enough to get onto a 1 m block |
| Time in the air | 0.58 s, which sets gravity to ≈30 m/s² |
| Speed while hit | Half, for 1 s after each hit |

- A player climbs onto a block only by jumping; walking into a wall doesn't push them up it.
- The view bobs with each step: by 8 cm at 2 steps a second walking, and by 14 cm at 2.8 steps a
  second sprinting (each player's own setting).
- The gun in hand sways with those steps: the hands slide it 2 cm left and right walking and 4 cm
  sprinting, and a longer gun's muzzle lags the other way, so the 1.23 m sniper rifle swings 1.7° to
  3.4° and the 0.30 m revolver a quarter of that (each player's own setting).
- A player's path bends near a warp object like light does, and the view turns with it, so walking
  into a wormhole mouth takes the player through it.

### Sprinting

Players can sprint, with head bob, as in other first-person shooters. Sprinting uses up energy,
which has nothing to do with the guns' heat.

- Sprinting speeds up and slows down (W-202): while Shift is held with energy left, the legs' speed rises from
  walking speed to the sprint's top speed at 15 m/s² (`movement.sprint_up_m_per_s_per_s`); the moment Shift is let
  go or the energy runs out it falls back at 15 m/s² (`movement.sprint_down_m_per_s_per_s`). Walking itself starts
  and stops at once, in every level.

- Energy drains at a steady rate while a player sprints, and refills at a steady rate while they
  don't: as fast as it drains while crouched, and half as fast while walking or standing.
- Energy never goes above full. A player who runs out of energy can't sprint.
- By default a player can't shoot while jumping, walking upright or sprinting: a shot waits 0.25 s after the
  last such move (`movement.shoot_after_moving_s`); walking crouched doesn't hold a shot. Each of the three is a
  game setting (`shooting.can_shoot_while_jumping`, `can_shoot_while_walking`, `can_shoot_while_sprinting`; W-147
  (shooting while moving, by game setting); the user: "the ability to toggle via game settings whether you can shoot
  while jumping, and seperately while moving laterally (i.e. while walking) and seperately while sprinting"), and
  bots follow the same rule.

| Parameter | Value |
| --- | --- |
| Time to drain full energy while sprinting | 7.5 s |
| Time to refill from empty while crouched | 7.5 s |
| Time to refill from empty while walking or standing | 15 s |
| Energy at spawn | Full |

### Gravity toward warps

Black holes and wormhole mouths pull players along the ground (W-202, pull toward wormholes and black holes in
the game settings): the level's own warps, and the thrown mouths once linked. Inside a radius of a warp's centre a
player is accelerated straight toward it, by the same amount everywhere inside the radius. A player can walk near a
warp and be fine, but too close the pull wins and the player goes through.

- The pull builds up a drift, a velocity added to the player's walk. Inside a radius nothing brakes it; outside
  every radius it dies away quickly.
- Going through a mouth keeps the speed: the player comes out of the other mouth as fast as it went in, and that
  mouth's pull then slows it. A player who touches nothing swings back and forth through the pair.
- The pull is weaker than sprinting speeds up, so a sprinting player always gets away.
- The pull points at the warp along the ground; a warp overhead pulls by the part of its direction along the
  ground. A warp whose horizon is wider than the radius doesn't pull.
- A move a wall refuses ends the drift. Flying players are not pulled.
- A black hole pulls differently from a mouth: over twice the radius, with half the strength, and in all three
  directions, so one overhead also lifts (a player in the air can still walk). Inside its radius a player's legs
  give only 0.75 of their speed, which still lets a sprinting player get away. Inside its minimum radius it pulls
  no further and holds players out, so nobody is pulled in. The view darkens as a player gets closer (W-168, fading
  out into a black hole): the edges go black and blur, reaching further in, and the whole picture dims, up to 0.85
  dark at the minimum radius. Lunar Surface's black hole was made 1.2 times bigger and lowered to 7.5 m (W-202), so
  its pull and darkening reach the ground as Snow Planet's do; time under it runs as slowly as before the move.
- The game server's settings hold it (`gravity` in `server_settings.yaml`), and the game master can change it.
- The user approved these numbers after playing with them (2026-11-10). They are game settings only: nothing in a
  level changes them while playing. The equations are in `demos/warp_pull_js/EQUATIONS.md`.
- Every level has the same pull, drift, sprint and darkening ("all of the planets should use snow planet physics"):
  Dust, Snow Planet, Lunar Surface, the match, City Sphere and Dark Forest. Dark Forest's bots are not pulled.

| Parameter | Value |
| --- | --- |
| Radius | 5 m from the warp's centre, 2.5 bodies (`gravity.radius_m`) |
| Strength | 11.9 m/s² (`gravity.acceleration_m_per_s_per_s`): a fall from the radius reaches a thrown mouth's throat at 9 m/s |
| Shape | the same everywhere inside the radius (`gravity.profile`: `constant`; also `stronger_near_mouth`, `harmonic`) |
| Braking outside every radius | 15 m/s² (`gravity.brake_m_per_s_per_s`) |
| Black hole's radius | 2 times the radius, 10 m (`gravity.black_hole_radius_per_radius`) |
| Black hole's strength | 0.5 of the strength (`gravity.black_hole_acceleration_frac`) |
| Legs' speed near a black hole | 0.75 (`gravity.black_hole_speed_frac`) |
| Black hole's minimum radius | 3 m from its centre (`gravity.black_hole_min_radius_m`) |
| Darkness at the minimum radius | 0.85 (`gravity.black_hole_darkness_frac`) |

### Map edge

No level has a wall at its edge. Every level slows players the same way past its playable area
(Zeno's paradox): each step a player takes outward is cut by the share of the way they have come
toward a stopping line, so they slow down without end and never reach it. Walking back in, or
along the edge, is at full speed.

- **Lunar Surface and Dust:** the crater's wall is the edge: the slowing starts at its foot and the
  stopping line is halfway up. The view stretches out from the middle of the screen as players
  slow, as if they were stretched ever further.
- **Snow Planet:** the edge is a band 12 m wide outside the playable ellipse, over open snow, with
  the same stretching view.
- **Dark Forest:** the edge is a band 12 m wide outside the playable rectangle, and the bamboo
  forest goes on 30 m past the edge, so a player walking off the map always sees trees ahead,
  never empty space.
- **City Sphere:** a closed sphere, so it has no edge.

## Controls

Every key here is the default. Each player can rebind any action, in the menu or in their copy of
`player_settings.yaml`, and the menu saves the choice for later games.

| Key | Action |
| --- | --- |
| Mouse | Look around |
| `W`, `A`, `S`, `D` | Walk forward, left, back, right |
| `Shift` (held) | Sprint, while there is energy (see [Sprinting](#sprinting)) |
| `Space` | Jump |
| `Ctrl` (held) | Crouch: a lower view and half the walking speed |
| Right button (held) | Aim: zoom in (more with the sniper rifle) and turn more slowly while held |
| `F` (held) | Flashlight: a cone of light from the eye while held (other players don't see it yet) |
| `V` | Switch between first person and a third-person view from slightly behind and above the character, which pulls in when a wall is behind it; shots still leave the eye toward the crosshair, and aiming looks from the eye |
| Left click | Shoot, once no movement key has been held for 0.25 s (walking crouched doesn't count, and in the air with no key held is fine), so not while walking upright, sprinting, jumping or deactivating a console (hold it for a weapon that fires while held, such as the rifle) |
| Mouse wheel, `1`, `2` | Switch weapons (see [Weapons](#weapons)) |
| `E` (held) | Deactivate a console of the opposing team's pylons (see [Winning a level](#winning-a-level)) |
| `Q` | Throw your purple wormhole mouth, the next press your green one, then purple again, and so on (see [Thrown wormholes](#thrown-wormholes)) |
| `1`, `2` (in the lobby) | Join the blue or the red team |
| `Tab` (held) | Show the satellite map (the whole level, live, see [Screen](#screen)) over the game while held; the game goes on and the player can still move and look. Letting go hides it. |
| `Esc` | Open or close the menu (see [Screen](#screen)), which frees the mouse while it is open; its QUIT page leaves the game. The player stands still while it is open, but the game goes on. |
| `Q`, `E` (in the menu) | Turn to the menu's previous or next page |
| `Up`, `Down`, `Return`, `Left`, `Right` (in the menu) | Pick a setting, rebind a key (`Return`, then the new key), change a number |
| `-`, `=` | Lower or raise the render resolution |

## Screen

The screens a player meets, in order:

- **Main menu** (the Mac app, opened without flags): a list of the online games on the server the app was built for,
  joining a server by the address the player types (its IP and port, remembered for next time), joining a game on
  this Wi-Fi, and Quit, each game on either map. A game on a server asks once for its password (empty for a
  server without one) and keeps it; a server that refuses it, or runs another version, stops the window with a
  pop-up saying why, and the main menu comes back.
- **Loading:** before the first frame, a black screen titled LOADING names what is loading (the surface textures,
  the shaders, each gun) over a bar like the console bar, filling as it goes.
- **Lobby**: it says CONNECTING until the server answers, then CONNECTED with how many players
  each team has. `1` joins blue and `2` joins red; it then says JOINING THE GAME until the server lets the player
  in. A player can't shoot from the lobby.
- **Connection banners:** when a window can't play on its server, a banner says why, and the terminal says what to
  do:
  - NO ANSWER FROM THE SERVER: nothing came back for 2 s;
  - SERVER RUNS ANOTHER VERSION: the server's messages don't fit this build, so rebuild both from the same commit;
  - THE GAME IS FULL: the server already has 12 players.
- **In play:** the crosshair, bars and minimap below, and the other players as figures in their team's colour.
- **Death:** the view darkens and says YOU DIED with a countdown until the player respawns (see
  [Respawning](#respawning)).
- **Win:** when a team has won, every player sees `<TEAM> TEAM WINS`, and a new game starts when the server's
  operator presses `Enter` in its terminal, or, on a server in the cloud, 10 s after the win.
- **Exit:** `Esc` opens the menu; on its QUIT page (`Q` from SCORES, where it opens) "Do you want to leave the
  game?" leaves with `Return` or `Y`, and `Esc` goes back to the game.

What a player sees while playing:

- **Crosshair** in the middle of the screen.
- **Bars**, in the bottom left: health, the gun's ammo (one block for the sniper rifle, which
  empties when it fires and fills when it has reloaded), and sprint energy. They are segments that
  lean like `/`, translucent and glowing. While health or energy changes, soft swells of light run
  along the filled part: reddish-orange down the health bar after a hit, pale blue down the energy
  bar while sprinting, and fainter ones while energy refills, as strong and fast as the value
  changes.
- **Minimap**, in a corner (top right by default): a Poincaré disk of the ground within 60 m,
  turned so the player faces up, with other players as dots and shots as lines in their team's
  colours. Straight paths away from the player bend into arcs, as they do in that picture of the
  hyperbolic plane. Each player sets whether it shows, its corner, its size and its reach in
  `player_settings.yaml` (`minimap`).
- **Console bar:** while a player holds `E` on a console, a bar below the crosshair fills over the
  10 s it takes, and they can't shoot (see [Winning a level](#winning-a-level)).
- **Satellite map, while `Tab` is held:** the whole level from above, live, over the dimmed game, which goes on
  taking the player's input, until `Tab` is let go. It has the MAP page's marks, plus the player as a disk pointing
  the way they face, every other player in the game, near or far, as a dot in their team's colour, and a circle of
  the player's fog of war range around them (`main_map.fog_of_war_range_m` in `player_settings.yaml`, 60 m by
  default), outside which the picture is darker and blurred: the fog of war. A legend line names the marks.
- **Menu** (`Esc`, which frees the mouse; `Esc` again closes it and captures the mouse): its name on the first line,
  the names of its five pages on the line under it (the open one in brackets), then the page. It opens on SCORES.
  The left and right arrows, `Q` and `E`, or a click on a page's name turn the pages. On SETTINGS, `Down` or `Up`
  goes from the pages' names into the rows, where the left and right arrows change the chosen number instead, and
  past the first or last row comes back to the names. A `?` at the right of the first line shows the menu's keys
  while the mouse is over it; the keys aren't shown otherwise. Inside the one-window app a sixth page, MATCH, comes
  first, with the match's status (the current level, the round, which team attacks and the scores), and the menu
  opens on it. Inside the menu the arrows, `Return`, `Esc`, `Q` and `E` always work, whatever the key
  bindings say, so a bad binding can always be fixed. The player stands still while it is open,
  but the game goes on.
  - SCORES: the player count, and each player's team, kills and deaths in this game, sorted by
    team and then kills, with the player's own line marked and each team's totals. Before a game
    on a server it says there is none, and before the server's first scores it says to join a team.
  - SETTINGS: the player's own settings, one row each: every key binding (`Return`, then the new
    key, button or wheel; `Esc` cancels), then numbers changed with `Left` and `Right` (mouse look
    speed, motion blur, head bob walking and sprinting, how red, shaky and blurred a hit looks,
    the wormhole colour's strength, the Milky Way's brightness, and the picture's brightness,
    contrast, saturation and gamma, which change only how the finished picture looks on that
    player's screen, W-51, basic visuals, and the satellite map's fog of war range), then a row that saves them to
    `~/.config/warped/player_settings.yaml` (or the `--player-settings` file), which later games
    load. A line below says what changed or went wrong.
  - MAP: the paper map, the whole level from above and all of it clear, with the ground's relief,
    structures, consoles, wormhole mouths, the black hole, the spawns, the zones' edges and the
    map edge. It is not live: no players are on it. A legend line names the marks.
  - CREDITS: where the game's assets come from (their authors, sources and licenses), the font
    and the libraries the game is built on.
  - QUIT: "Do you want to leave the game?"; `Return` or `Y` leaves it (a level run alone quits; the one-window app
    goes back to its map menu), and `Esc` goes back to the game.
- **Hit effects:** a player who is hit sees red and a blur come in from the screen's edges, and the
  screen shakes (each player's own setting).
- **Motion blur:** a slight one (each player's own setting).
- **Window title:** the frame rate, the render time, the GPU's own time and the render scale.

## Bullets, lasers and light

- Warp objects bend light, so players see the world bent around them.
- A shot is a laser beam: a light ray from the gun's tip, with no drop by default. Its front flies out at
  250 m/s, and each point of it fades soon after the front passes.
- Bullet drop is a game setting, off by default (`shooting.drop_m_per_s_per_s`, W-157 (bullet drop, by game
  setting); the user: "add setting in game settings for bullet drop"): above 0, every shot's path curves down as a
  ball thrown at the beam's speed would fall (toward the floor in City Sphere), the drawn beam follows it, and
  every window traces it alike from the server's settings.
- Warp objects bend lasers as they bend light, and lasers pass through wormholes.
- Glass lets light through. On Lunar Surface the windows let lasers through too.
- A player moving through a wormhole's throat sees the world's colours shift for a moment, as if moving near light
  speed: a crisp light cyan, almost white, walking forward, a dark red walking backward, stronger nearer the throat
  and faster, none standing still or outside the throat's region. Only that player sees it.
- A shot that hits the ground throws up dust and a puff of mist where it lands, which every player
  sees. The grains fly up a few meters and fall back, and all of it fades within 2 s.

## Thrown wormholes

Each player has a pair of wormhole mouths of their own, one purple and one green, which they throw
anywhere in the level.

- **Throwing:** `Q` throws the purple mouth and the next press the green one. Once both are down,
  `Q` does nothing until the pair is gone (death or destruction); a throw that fizzles can be
  thrown again.
- **Flight:** a mouth is lobbed from the player's hand a little above the crosshair and curves down
  under its own gravity. It drops to the ground below whatever it hits, and hovers there at eye
  height, facing back the way it flew.
- **Fizzling:** a throw fizzles if it hasn't landed within its flight time, or if it lands too close
  to a wall or another warp object (their reaches may not overlap).
- **Linking:** once both of a player's mouths are down they link, like a level's wormholes: any
  player or laser that goes into one comes out of the other, bent on the way like light.
- **Looks:**
  - the view through a thrown mouth is tinted its colour;
  - a mouth in flight, or down while its partner isn't, glows as a small ball;
  - every player sees every player's mouths, and anyone can walk or shoot through them.
- **Once per level:** a player throws one pair in a level and no more (W-199, one wormhole per player per level).
  The mouths stay where they are when their thrower dies, so respawning gives no new pair. A throw that fizzles
  doesn't use the mouth up.
- **Shots don't destroy mouths** (W-166, clear rules for destroying thrown wormholes): a shot through a linked
  mouth is carried through it and does nothing to the pair, whoever threw it; a shot at a lone mouth's ball does
  nothing either. The game master can turn destroying back on by setting the hits below above 0.
- **Not built yet:** switching an enemy's pair off by holding `E` on a mouth, and a pair staying when its thrower
  leaves the level (it still goes with them).

| Parameter | Value |
| --- | --- |
| Throw speed | 16 m/s (`wormholes.throw_speed_m_per_s`) |
| Throw angle above the crosshair | 0.3 rad (`wormholes.throw_up_rad`) |
| Gravity on a mouth in flight | 20 m/s² (`wormholes.gravity_m_per_s_per_s`) |
| Longest flight before it fizzles | 4 s (`wormholes.max_flight_s`) |
| A mouth's size on Lunar Surface | 1.6 m Schwarzschild radius (`wormholes.schwarzschild_radius_m`); on the house, the house's mouths' size |
| How far its bending reaches on Lunar Surface | 6 m (`wormholes.reach_m`) |
| Hits that destroy an enemy's pair | 0 for both guns: never (`wormholes.sniper_hits_to_destroy`, `wormholes.rifle_hits_to_destroy`) |
| Hits that destroy an enemy's lone mouth | never |
| Red flash after each hit | 0.25 s (`wormholes.hit_flash_s`) |

## Weapons

Every weapon follows these rules:

- Ammo is infinite. The rifle overheats if it fires too long without a break; the sniper rifle
  reloads after each shot.
- Every shot shows a laser in the shooter's team color. Both teams see it, so it gives away where
  the shooter is.
- A player who is hit is slowed (see [Movement](#movement)).
- Each shot strays from the crosshair by a small random angle, sampled for that shot.
- Each shot's beam flickers with noise: every frame, random pixels of it turn black, white or drop out to show
  what is behind.
- Aiming (right button) zooms in over 0.15 s and turns the view more slowly by the same factor.
- The rules are a simplified version of those of tactical shooters such as CS:GO.
- A player who carries two weapons switches between them with the mouse wheel (either way swaps
  them), or with `1` for the first and `2` for the second. Every player carries both.
- The weapon in hand shows on the screen. It drops and swings while the player sprints, and points
  ahead again when they walk.

| Parameter | Sniper rifle (`1`) | Rifle (`2`) |
| --- | --- | --- |
| Looks | A wooden bolt-action rifle with a scope, drawn small so its body shows | The energy revolver |
| Fire rate, at most | 1 shot every 2 s (a 2 s reload) | 8 shots/s while the button is held |
| Overheats after | Never: it reloads after each shot | 20 shots in a row; cools down fully in 2 s without firing |
| Hits to kill, head (the top half-meter) | 1 | 2 |
| Hits to kill, body | 1 | 12 |
| Time to kill, sustained fire | One shot | ≈1.4 s to the body, 0.125 s to the head |
| Shot lasts (hits what the crosshair crosses) | 0.25 s | 0.06 s per shot |
| Laser | Shows once per shot; each point fades 0.25 s after the front passes | Pulses with each shot; each point fades after 0.05 s |
| Shot noise (each shot strays at random) | Small: 3 mrad on average, sampled exponentially, at most 15 mrad | Up to 2 mrad cold, growing with the rifle's heat to 20 mrad and shrinking as it cools |
| Beam look | A white lightsaber blade in a pale haze | A laser in the team's colour |
| Beam noise (each frame, a share of the beam's pixels turns black, white or drops out, 1 : 1 : 8) | Random per shot: 0.2 on average, sampled exponentially, at most 0.5 | None cold, growing with the rifle's heat to 0.5 |
| Recoil | None | Each shot kicks the view up 7.5 mrad and 2.5 mrad left or right, alternately, settling in 0.08 s |
| Aim zoom | 2.5× | 1.5× |

The rifle's fire rate and body hits to kill are tuned together, so that body shots kill within the
time to kill.

## Open questions

- **Consoles:** whether a hit that doesn't eliminate the player interrupts a deactivation.
- **Sprinting:** whether a kill should refill the killer's energy, as an earlier design had.
- **Hyperbolic levels:** the topology of Dust.
- **Warp objects:** how many black holes and wormholes each level but Lunar Surface has, and where.
- **Flashlight:** whether other players should see it.

These may change later: more than two levels per team.
