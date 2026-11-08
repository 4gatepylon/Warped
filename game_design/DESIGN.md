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

| Term | Meaning |
| --- | --- |
| Team | Blue or red, ≈5 players each. A game holds at most 8 players for now. |
| Match | One game between the two teams, played across the levels of one map. |
| Map | A line of levels: the blue levels, one purple level in the middle, then the red levels. |
| Level | One playable space. A blue or red level belongs to that team; the purple level to neither. |
| Depth | How far a level is from the purple level: 0 for purple, then 1, 2, … outward. |
| Round | One play of one level, which one team wins. |
| Wavefront | A team's next level to win on the other team's side. It only moves deeper. |
| Attacker | The team whose wavefront is being played. |
| Defender | The team that owns the level being played. |
| Lobby | The screen a player joins on: it shows each team's head count, and the player picks a team. |
| Spawn | Where a team's players enter a level. |
| Pylon | A team's objective in a level, near its spawn. |
| Console | One part of a pylon. The opposing team deactivates consoles to win the level. |
| Zone | One of a level's three regions: the blue zone, the middle zone and the red zone. |
| Route | One way across the middle zone, from one side to the other. |
| Floor | One walkable layer inside a level. |
| Warp object | A black hole or a wormhole. Level designers place them; players can't. |
| Energy | What sprinting uses up. It refills over time, twice as fast while crouched. |
| Map edge | The soft edge of a level: players slow down without end as they climb it, so they never leave. |

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

- Each team has a spawn and, near it, a pylon. The two spawns are on opposite sides of the level.
- A team wins a level by deactivating every console on the opposing team's pylon, as in capture
  the flag. The [wavefront](#wavefront) then decides which level is played next.
- A pylon with more consoles takes longer to deactivate.
- A console pulses with light once a second while it is on. While someone holds `E` on it, it
  pulses faster and faster, up to 8 times a second just before it turns off, and eases back down
  over about a second if the hold stops. It goes dark once it is off.
- A player can only deactivate the other team's consoles.

| Parameter | Value |
| --- | --- |
| Pylons per team per level | 1 |
| Consoles per pylon | Set per level (see [Levels](#levels)) |
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
- Friendly fire is on: a shot eliminates a teammate as it would an enemy, but only an enemy counts
  as a kill.
- The game counts each player's kills and deaths, shown on the scoreboard (see [Screen](#screen)).

### Respawning

A player who is eliminated sees their view darken with "YOU DIED" and a countdown, and comes back
after the respawn time on a ring around their team's spawn, never on top of another player, with
full health and energy. A player can't choose to respawn early.

| Parameter | Value |
| --- | --- |
| Respawn time | 3 s (`respawn.delay_s`) |

## Levels

| Parameter | Dust | Snow Planet | City Sphere | Dark Forest | Lunar Surface |
| --- | --- | --- | --- | --- | --- |
| Team | Blue | Blue | Purple | Red | Red |
| Depth | 2 | 1 | 0 | 1 | 2 |
| Theme | Dust (desert) planet | Snow planet | City, or space station | Dark forest planet | Lunar base in a crater |
| Space | Hyperbolic | T² × ℝ | Inside a sphere | T² × ℝ | Hyperbolic, curvature radius 40 m |
| Gravity | Down, onto the ground | Down | Outward, onto the sphere | Down | Down, onto the ground, ≈30 m/s² |
| Warp objects | Yes | Yes | No | Yes | 1 black hole, 1 wormhole |
| Size | Very large | Large | Smallest | Large | Very large: 256 m × 144 m playable |
| Consoles per pylon | 5 | 4 | 4 | 4 | 5 |
| Combat focus | Long range | Mid range | Close quarters | Mid range | Long range |
| Zone layout | Open | Paired bands | Bands | Paired bands | Open |
| Detailed layout | TBD | TBD | See [City Sphere](#city-sphere) | TBD | See [Lunar Surface](#lunar-surface) |
| Built in the Rust game | No | No | No | No | Yes |

### Spaces

- **Inside a sphere:** flat space, but players walk on the inside of a hollow sphere and gravity
  pulls them outward onto its surface. With no warp objects, light travels in straight lines.
- **T² × ℝ:** flat space whose two horizontal axes loop back around (a torus) and whose vertical
  axis goes on forever. The loops are large. The warp objects curve space around them.
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

- The blue spawn and pylon form a short segment. The red spawn and pylon are that segment
  reflected through the sphere's center, so the two sides sit at antipodes.
- Walls divide the space into close quarters.
- City Sphere has two floors: the sphere's surface, and the surface of a smaller sphere with the
  same center, which players walk up to. Gravity pulls outward on both floors.
- The inner floor is glass and roofs the whole outer floor. Players on either floor see each other
  through it but can't shoot through it.
- City Sphere may change to one floor, or to a roof that is only partly glass.

### Lunar Surface

The level as the Rust game builds it, from the map
[`common/maps/lunar_surface/`](../common/maps/lunar_surface/map.json):

- A lunar base on a regolith plain inside an elliptical crater, with hills and mounds in the
  ground. The playable area is an ellipse 256 m long and 144 m wide; the crater's wall around it is
  the [map edge](#map-edge).
- The zones are bands across its length: blue more than 45 m from the middle on blue's side, red
  more than 45 m on red's side, and the middle zone, 90 m long, between them.
- A black hole hangs about 17 m above the middle of the level; its pull reaches 24 m. A player who falls into
  it goes back to their spawn.
- One wormhole joins a ridge to the red landing pad. Its mouths are 15 m in reach.
- Blue's side has a lander, the blue pylon, a relay mast, supply crates and a solar array. The
  middle has derelict habitat modules, a comms dome, a crater walkway, a relay mast, a solar array
  and crates. Red's side has barriers, a habitat module, a dome, the red pylon, a spawn hangar, a
  watchtower, a landing pad, an antenna, a walkway, crates and a solar array.
- Structures have sharp edges. The habitat modules' and walkways' windows are glass: players see
  out of them and shoot through them, but can't climb through them.
- The sky has the sun, a planet, stars, and spaceships that fly over, purely for looks, on the same
  clock for every player.
- Lighting has soft sun shadows, lamps, light from the planet, a thin haze and bloom.

## Zones and routes

Directions in a level:

- **Lengthwise:** from one team's spawn toward the other's.
- **Widthwise:** along the ground, at right angles to lengthwise.

Every level follows these rules:

- A level splits lengthwise into three zones: the blue zone around blue's spawn and pylon, the
  middle zone, and the red zone around red's. The middle zone is always the biggest.
- The middle zone has 2–3 routes across it lengthwise. A route is a sequence of rooms and/or paths,
  with corners to defend.
- On most levels, one route is a big open area, joined to at least one other route by a connector.
- The blue and red zones are easy to cross widthwise, so a team can rotate quickly between routes.

| Zone layout | Shape |
| --- | --- |
| Bands | The spawns sit at antipodes. The middle zone is a wide band around the sphere, halfway between them, and the blue and red zones are thinner bands on either side of it. |
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
- A player's path bends near a warp object like light does, and the view turns with it, so walking
  into a wormhole mouth takes the player through it.

### Sprinting

Players can sprint, with head bob, as in other first-person shooters. Sprinting uses up energy,
which has nothing to do with the guns' heat.

- Energy drains at a steady rate while a player sprints, and refills at a steady rate while they
  don't: as fast as it drains while crouched, and half as fast while walking or standing.
- Energy never goes above full. A player who runs out of energy can't sprint.
- A player can't shoot while sprinting.

| Parameter | Value |
| --- | --- |
| Time to drain full energy while sprinting | 7.5 s |
| Time to refill from empty while crouched | 7.5 s |
| Time to refill from empty while walking or standing | 15 s |
| Energy at spawn | Full |

### Map edge

A level has no wall at its edge. On Lunar Surface the crater's wall is the edge: from its foot,
each step a player takes outward is cut by the share of the way they have climbed toward halfway
up, so they slow down without end and never get past halfway. Their view stretches out from the
middle of the screen as they slow, as if they were stretched ever further. Walking back in is at
full speed.

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
| Left click | Shoot, standing still or crouched (walking crouched too), not while walking upright, sprinting, in the air or deactivating a console (hold it for a weapon that fires while held, such as the rifle) |
| Mouse wheel, `1`, `2` | Switch weapons (see [Weapons](#weapons)) |
| `E` (held) | Deactivate a console of the opposing team's pylon (see [Winning a level](#winning-a-level)) |
| `1`, `2` (in the lobby) | Join the blue or the red team |
| `Tab` | Open or close the menu (see [Screen](#screen)). The player stands still while it is open, but the game goes on. |
| `Q`, `E` (in the menu) | Turn to the menu's previous or next page |
| `Up`, `Down`, `Return`, `Left`, `Right` (in the menu) | Pick a setting, rebind a key (`Return`, then the new key), change a number |
| `-`, `=` | Lower or raise the render resolution |
| `Esc` | Free the mouse; with the mouse free, ask "Do you want to exit?" (`Return` or `Y` exits, `Esc` or `N` goes back to the game) |

## Screen

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
  hyperbolic plane.
- **Menu** (`Tab`), with three pages:
  - SETTINGS: the player's own settings (key bindings, mouse speed and view effects).
  - MAP: the whole level from above, with the ground's relief, structures, consoles, wormhole
    mouths, the black hole, the spawns, the zones' edges and the map edge, the player as a disk
    pointing the way they face, and a circle of the minimap's reach around them (the rest darker
    and blurred).
  - SCORES: the player count, and each player's team, kills and deaths, sorted by team and kills,
    with each team's totals.
- **Hit effects:** a player who is hit sees red and a blur come in from the screen's edges, and the
  screen shakes (each player's own setting).
- **Motion blur:** a slight one (each player's own setting).

## Bullets, lasers and light

- Warp objects bend light, so players see the world bent around them.
- A shot is a laser beam: a light ray from the gun's tip, with no drop. Its front flies out at
  250 m/s, and each point of it fades soon after the front passes.
- Warp objects bend lasers as they bend light, and lasers pass through wormholes.
- Glass lets light through. On Lunar Surface the windows let lasers through too.
- A player coming out of a wormhole sees the world blueshift toward cyan, as if moving at 15% of
  light speed, fading back within 1.5 s. Only that player sees it.
- A shot that hits the ground throws up dust and a puff of mist where it lands, which every player
  sees. The grains fly up a few meters and fall back, and all of it fades within 2 s.

## Weapons

Every weapon follows these rules:

- Ammo is infinite. The rifle overheats if it fires too long without a break; the sniper rifle
  reloads after each shot.
- Every shot shows a laser in the shooter's team color. Both teams see it, so it gives away where
  the shooter is.
- A player who is hit is slowed (see [Movement](#movement)).
- Each shot strays from the crosshair by a small random angle, sampled for that shot.
- Aiming (right button) zooms in over 0.15 s and turns the view more slowly by the same factor.
- The rules are a simplified version of those of tactical shooters such as CS:GO.
- A player who carries two weapons switches between them with the mouse wheel (either way swaps
  them), or with `1` for the first and `2` for the second. Every player carries both.
- The weapon in hand shows on the screen. It drops and swings while the player sprints, and points
  ahead again when they walk.

| Parameter | Sniper rifle (`1`) | Rifle (`2`) |
| --- | --- | --- |
| Looks | A thin wand with a tip in the team's colour | A grey block |
| Fire rate, at most | 1 shot every 2 s (a 2 s reload) | 8 shots/s while the button is held |
| Overheats after | Never: it reloads after each shot | 20 shots in a row; cools down fully in 2 s without firing |
| Hits to kill, head (the top half-meter) | 1 | 2 |
| Hits to kill, body | 1 | 12 |
| Time to kill, sustained fire | One shot | ≈1.4 s to the body, 0.125 s to the head |
| Shot lasts (hits what the crosshair crosses) | 0.25 s | 0.06 s per shot |
| Laser | Shows once per shot; each point fades 0.25 s after the front passes | Pulses with each shot; each point fades after 0.05 s |
| Shot noise (each shot strays at random) | Small: 3 mrad on average, sampled exponentially, at most 15 mrad | Up to 2 mrad cold, growing with the rifle's heat to 20 mrad and shrinking as it cools |
| Recoil | None | Each shot kicks the view up 7.5 mrad and 2.5 mrad left or right, alternately, settling in 0.08 s |
| Aim zoom | 2.5× | 1.5× |

The rifle's fire rate and body hits to kill are tuned together, so that body shots kill within the
time to kill.

## Open questions

- **Consoles:** whether a hit that doesn't eliminate the player interrupts a deactivation.
- **Respawning:** whether the respawn time should grow as a round goes on. An earlier design had
  20 s at the start of a round plus 10 s every 5 minutes.
- **Sprinting:** whether a kill should refill the killer's energy, as an earlier design had.
- **Hyperbolic levels:** the topology of Dust.
- **Warp objects:** how many black holes and wormholes each level but Lunar Surface has, and where.
- **Flashlight:** whether other players should see it.

These may change later: more than one pylon per team per level, players placing warp objects, and
more than two levels per team. The wishlist has work in progress on both: three pylons per team
that strengthen the black hole as they go down (W-68), and wormholes each player throws (W-60).
