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
| Pylon | One of a team's 1 to 3 objectives in a level, near its spawn. |
| Console | One part of a pylon. The opposing team deactivates consoles to win the level. |
| Zone | One of a level's three regions: the blue zone, the middle zone and the red zone. |
| Route | One way across the middle zone, from one side to the other. |
| Floor | One walkable layer inside a level. |
| Warp object | A black hole or a wormhole. Level designers place them, and each player throws a pair of wormhole mouths of their own (see [Thrown wormholes](#thrown-wormholes)). |
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
  hurt teammates.
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

- Each team has one pylon, on top of the glass floor in an exposed spot with no cover around it.
  The red spawn and pylon are blue's reflected through the sphere's center, so the two sides sit
  at antipodes.
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
  more than 45 m on red's side, and the middle zone, 90 m long, between them. Each team zone's edge
  is a faint see-through wall in the team's colour, 6 m high and 0.6 m thick, that stops neither
  players nor lasers.
- A black hole hangs about 17 m above the middle of the level; its pull reaches 24 m. A player who falls into
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
  partner lands: shooting your own or a teammate's ball does nothing, and one shot at an enemy's ball destroys it.
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
- The gun in hand sways with those steps: the hands slide it 2 cm left and right walking and 4 cm
  sprinting, and a longer gun's muzzle lags the other way, so the 1.23 m sniper rifle swings 1.7° to
  3.4° and the 0.30 m revolver a quarter of that (each player's own setting).
- A player's path bends near a warp object like light does, and the view turns with it, so walking
  into a wormhole mouth takes the player through it.

### Sprinting

Players can sprint, with head bob, as in other first-person shooters. Sprinting uses up energy,
which has nothing to do with the guns' heat.

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

Black holes and wormhole mouths pull players along the ground, in every level (W-144): the
level's own warps, and the thrown mouths once linked. Each adds a speed toward itself to the
player's walk, so running toward one is faster, running away slower, and a player standing still
drifts toward it. The pull is relatively weak but noticeable: strongest near the warp, falling off
with distance, and never stopping a player who walks away.

- The pull points at the warp along the ground; a warp overhead (Lunar Surface's black hole, Snow
  Planet's) pulls by the part of its direction along the ground, so a player drifts to stand
  under it.
- A mouth's pull fades to nothing at its throat, so a player drifting in stops at the mouth
  rather than being pulled through it; walking in still goes through.
- All warps' pulls together are capped at the strongest pull. It is a share of walking speed, so
  it scales with each level's walking speed: more than crouching speed (crouching away from a
  warp that close is pulled in), less than walking speed.
- The game server's settings hold it (`gravity` in `server_settings.yaml`, the same built-in
  values in every level when left out, which a level or the game master can change).

| Parameter | Value |
| --- | --- |
| Strongest pull | 0.75 of walking speed (Lunar Surface: 7.5 m/s) (`gravity.max_pull_per_walk_speed`) |
| Full pull | within 3 m of a black hole's horizon or a mouth's throat (`gravity.full_pull_m`) |
| Falling off | as the square of the share of the way left to the reach |
| Reach | 24 m out from the horizon or throat (`gravity.reach_m`) |

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
| `V` | Switch between first person and a third-person view from slightly behind and above the character, which pulls in when a wall is behind it; shots still leave the eye toward the crosshair, and aiming looks from the eye |
| Left click | Shoot, once no movement key has been held for 0.25 s (walking crouched doesn't count, and in the air with no key held is fine), so not while walking upright, sprinting, jumping or deactivating a console (hold it for a weapon that fires while held, such as the rifle) |
| Mouse wheel, `1`, `2` | Switch weapons (see [Weapons](#weapons)) |
| `E` (held) | Deactivate a console of the opposing team's pylons (see [Winning a level](#winning-a-level)) |
| `Q` | Throw your purple wormhole mouth, the next press your green one, then purple again, and so on (see [Thrown wormholes](#thrown-wormholes)) |
| `1`, `2` (in the lobby) | Join the blue or the red team |
| `Tab` (held) | Show the whole level's map (the menu's MAP page) over the game while held; the game goes on and the player can still move and look. Letting go hides it. |
| `Esc` | Open or close the menu (see [Screen](#screen)), which frees the mouse while it is open; its QUIT page leaves the game. The player stands still while it is open, but the game goes on. |
| `Q`, `E` (in the menu) | Turn to the menu's previous or next page |
| `Up`, `Down`, `Return`, `Left`, `Right` (in the menu) | Pick a setting, rebind a key (`Return`, then the new key), change a number |
| `-`, `=` | Lower or raise the render resolution |

## Screen

The screens a player meets, in order:

- **Title** (the Mac app, opened without flags): a list of the online games on the server the app was built for,
  joining a server by the address the player types (its IP and port, remembered for next time), joining a game on
  this Wi-Fi, and Quit, each game on either map. A game on a server asks once for its password (empty for a
  server without one) and keeps it; a server that refuses it, or runs another version, stops the window with a
  pop-up saying why, and the title screen comes back.
- **Loading:** before the first frame, a black screen titled LOADING names what is loading (the surface textures,
  the shaders, each gun) over a bar like the console bar, filling as it goes.
- **Lobby** (the team screen): it says CONNECTING until the server answers, then CONNECTED with how many players
  each team has. `1` joins blue and `2` joins red; it then says JOINING THE GAME until the server lets the player
  in. A player can't shoot from the lobby.
- **Connection banners:** when a window can't play on its server, a banner says why, and the terminal says what to
  do:
  - NO ANSWER FROM THE SERVER: nothing came back for 2 s;
  - SERVER RUNS ANOTHER VERSION: the server's messages don't fit this build, so rebuild both from the same commit;
  - THE GAME IS FULL: the server already has 8 players.
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
- **Map while `Tab` is held:** the MAP page's picture and legend over the dimmed game, which goes on taking the
  player's input, until `Tab` is let go.
- **Menu** (`Esc`, which frees the mouse; `Esc` again closes it and captures the mouse), with five pages under a tab
  bar; it opens on SCORES, and `Q` and `E` turn the pages. Inside the one-window app a sixth page, MATCH, comes
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
    player's screen, W-51, basic visuals), then a row that saves them to
    `~/.config/warped/player_settings.yaml` (or the `--player-settings` file), which later games
    load. A line below says what changed or went wrong.
  - MAP: the whole level from above, with the ground's relief, structures, consoles, wormhole
    mouths, the black hole, the spawns, the zones' edges and the map edge, the player as a disk
    pointing the way they face, a circle of the minimap's reach around them (the rest darker and
    blurred), and every other player in the game, near or far, as a dot in their team's colour.
    A legend line names the marks.
  - CREDITS: where the game's assets come from (their authors, sources and licenses), the font
    and the libraries the game is built on.
  - QUIT: "Do you want to leave the game?"; `Return` or `Y` leaves it (a level run alone quits; the one-window app
    goes back to its game menu), and `Esc` goes back to the game.
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
  ball thrown at the beam's speed would fall (toward the street in City Sphere), the drawn beam follows it, and
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
  under its own gravity. It drops to the floor below whatever it hits, and hovers there at eye
  height, facing back the way it flew.
- **Fizzling:** a throw fizzles if it hasn't landed within its flight time, or if it lands too close
  to a wall or another warp object (their reaches may not overlap).
- **Linking:** once both of a player's mouths are down they link, like a level's wormholes: any
  player or laser that goes into one comes out of the other, bent on the way like light.
- **Looks:**
  - the view through a thrown mouth is tinted its colour;
  - a mouth in flight, or down while its partner isn't, glows as a small ball;
  - every player sees every player's mouths, and anyone can walk or shoot through them.
- **Death:** a player's mouths disappear when they die.
- **Shooting an enemy's mouths:** a shot that goes through an enemy's linked mouth makes both of
  that enemy's mouths flash red; enough hits destroy the pair. A shot that hits an enemy's lone
  mouth's ball (down, its partner not yet thrown) destroys it at once. A shot at a player's own or
  a teammate's mouths, or at a ball still in flight, does nothing.

| Parameter | Value |
| --- | --- |
| Throw speed | 16 m/s (`wormholes.throw_speed_m_per_s`) |
| Throw angle above the crosshair | 0.3 rad (`wormholes.throw_up_rad`) |
| Gravity on a mouth in flight | 20 m/s² (`wormholes.gravity_m_per_s_per_s`) |
| Longest flight before it fizzles | 4 s (`wormholes.max_flight_s`) |
| A mouth's size on Lunar Surface | 1.6 m Schwarzschild radius (`wormholes.schwarzschild_radius_m`); on the house, the house's mouths' size |
| How far its bending reaches on Lunar Surface | 6 m (`wormholes.reach_m`) |
| Hits that destroy an enemy's pair | 2 sniper shots or 5 rifle shots (`wormholes.sniper_hits_to_destroy`, `wormholes.rifle_hits_to_destroy`) |
| Hits that destroy an enemy's lone mouth | 1 shot of either gun |
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
