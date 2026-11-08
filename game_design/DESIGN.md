# Warped game design

This document formalizes the notes in [`brainstorm/USER_THOUGHTS.md`](brainstorm/USER_THOUGHTS.md).
Every level and every weapon is described by the same parameters: the rows of its table. To add a
level or a weapon, add a column and fill in every row. To add a parameter, add a row and fill it in
for every column.

- **≈** marks a first guess, to be tuned in playtests.
- **TBD** marks something not designed yet.

> NOTE: Almost every number in this document is configurable, such as consoles per pylon, the time
> to deactivate a console and pylons per team per level, and may change after playtests. The
> default map won't be the only map either: maps will be configurable too, though not yet.

## Terms

| Term | Meaning |
| --- | --- |
| Team | Blue or red, ≈5 players each. |
| Match | One game between the two teams, played across the levels of one map. |
| Map | A line of levels: the blue levels, one purple level in the middle, then the red levels. |
| Level | One playable space. A blue or red level belongs to that team; the purple level to neither. |
| Depth | How far a level is from the purple level: 0 for purple, then 1, 2, … outward. |
| Round | One play of one level, which one team wins. |
| Wavefront | A team's next level to win on the other team's side. It only moves deeper. |
| Attacker | The team whose wavefront is being played. |
| Defender | The team that owns the level being played. |
| Spawn | Where a team's players enter a level. |
| Pylon | A team's objective in a level, near its spawn. |
| Console | One part of a pylon. The opposing team deactivates consoles to win the level. |
| Zone | One of a level's three regions: the blue zone, the middle zone and the red zone. |
| Route | One way across the middle zone, from one side to the other. |
| Floor | One walkable layer inside a level. |
| Warp object | A black hole or a wormhole. Level designers place them; players can't. |
| Energy | What sprinting uses up. It refills over time, and fully on a kill. |

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

### Winning a level

- Each team has a spawn and, near it, a pylon. The two spawns are on opposite sides of the level.
- A team wins a level by deactivating every console on the opposing team's pylon, as in capture
  the flag. The [wavefront](#wavefront) then decides which level is played next.
- A pylon with more consoles takes longer to deactivate.

| Parameter | Value |
| --- | --- |
| Pylons per team per level | 1 |
| Consoles per pylon | Set per level (see [Levels](#levels)) |
| Control to deactivate a console | Hold E |
| Time to deactivate one console | 10 s |
| While deactivating | The player must face the pylon and can't use a weapon. |
| Letting go of E | Resets that console's progress to zero. |

### Wavefront

Each team defends its own levels and attacks the other team's. A match is a sequence of rounds:

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

### Respawning

A player who dies waits out a respawn time before coming back. The respawn time grows in steps as
a round goes on, so dying costs more late in a round than early on.

| Parameter | Value |
| --- | --- |
| Respawn time at the start of a round | 20 s |
| Increase per step | 10 s |
| Time between steps | 5 min |

Respawn time = 20 s + 10 s × ⌊t / 5 min⌋, where t is the time since the round started. For
example, a player who dies 12 minutes into a round waits 40 s. A match lasts at most 30 minutes,
so the respawn time never goes above 70 s. These values are configurable starting points and may
change after playtests.

## Levels

| Parameter | Dust | Snow Planet | City Sphere | Dark Forest | Lunar Surface |
| --- | --- | --- | --- | --- | --- |
| Team | Blue | Blue | Purple | Red | Red |
| Depth | 2 | 1 | 0 | 1 | 2 |
| Theme | Dust (desert) planet | Snow planet | City, or space station | Dark forest planet | Lunar surface |
| Space | Hyperbolic | T² × ℝ | Inside a sphere | T² × ℝ | Hyperbolic |
| Gravity | Down, onto the ground | Down | Outward, onto the sphere | Down | Down, onto the ground |
| Warp objects | Yes | Yes | No | Yes | Yes |
| Size | Very large | Large | Smallest | Large | Very large |
| Consoles per pylon | 5 | 4 | 4 | 4 | 5 |
| Combat focus | Long range | Mid range | Close quarters | Mid range | Long range |
| Zone layout | Open | Paired bands | Bands | Paired bands | Open |
| Detailed layout | TBD | TBD | See [City Sphere](#city-sphere) | TBD | TBD |

### Spaces

- **Inside a sphere:** flat space, but players walk on the inside of a hollow sphere and gravity
  pulls them outward onto its surface. With no warp objects, light travels in straight lines.
- **T² × ℝ:** flat space whose two horizontal axes loop back around (a torus) and whose vertical
  axis goes on forever. The loops are large. The warp objects curve space around them.
- **Hyperbolic:** hyperbolic space with warp objects. The pylons are far apart. Gravity pulls
  players down onto the ground, along the ground's normal under them; players never get far above
  the ground, so that simple rule is enough. The topology is TBD.

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
| Open | The zones have no set shape yet. They cover the space, with the spawns far apart, and walls, obstacles and mountains shape the routes. |

## Movement

### Sprinting

Players can sprint, with head bob, as in other first-person shooters. Sprinting uses up energy,
which has nothing to do with the guns' heat.

- Energy drains at a steady rate while a player sprints, and refills at a steady rate while they
  don't.
- A kill refills the killer's energy to full at once.
- Energy never goes above full. A player who runs out of energy can't sprint.

| Parameter | Value |
| --- | --- |
| Time to drain full energy while sprinting | 30 s (≈3.3% per second) |
| Time to refill from empty while not sprinting | 60 s (≈1.7% per second) |
| Energy after a kill | Full |
| Sprint speed | TBD |

These values are configurable and may change after playtests.

## Controls

| Key | Action |
| --- | --- |
| Mouse | Look around |
| `W`, `A`, `S`, `D` | Walk forward, left, back, right |
| `Shift` (held) | Sprint, while there is energy (see [Sprinting](#sprinting)) |
| `Space` | Jump |
| `Ctrl` (held) | Crouch: a lower view and half the walking speed |
| Right button (held) | Flashlight: a cone of light from the eye while held |
| Left click | Shoot, standing still or crouched, not while walking upright, sprinting or in the air (hold it for a weapon that fires while held, such as the rifle) |
| Mouse wheel, `1`, `2` | Switch weapons (see [Weapons](#weapons)) |
| `E` (held) | Deactivate a console of the opposing team's pylon (see [Winning a level](#winning-a-level)) |
| `Tab` | Open or close the menu of the player's own settings: key bindings, mouse speed and view effects |

## Bullets, lasers and light

- Warp objects bend light, so players see the world bent around them.
- A shot fires a bullet (an energy pulse) with minimal drop, and shows a laser along its path with
  no drop.
- Warp objects bend bullets and lasers as they bend light, and both pass through wormholes.

## Weapons

Every weapon follows these rules:

- Ammo is infinite, but firing too often overheats the weapon.
- Every shot shows a laser in the shooter's team color. Both teams see it, so it gives away where
  the shooter is.
- A player who is hit is slowed.
- The rules are a simplified version of those of tactical shooters such as CS:GO.
- A player who carries two weapons switches between them with the mouse wheel (either way swaps
  them), or with `1` for the first and `2` for the second.

| Parameter | Sniper rifle | Rifle |
| --- | --- | --- |
| Fire rate, at most | ≈1 shot every 2 s | 2–10 shots/s |
| Overheats after | ≈8 shots | 15–25 shots |
| Hits to kill, head | 1 | 2 |
| Hits to kill, body | 1 | 10–20 |
| Time to kill, sustained fire | One shot | At most ≈1–2 s |
| Laser | Shows once per shot, then fades slowly | Pulses on and off while firing |

The rifle's fire rate and body hits to kill are tuned together, so that body shots kill within the
time to kill.

## Open questions

- **Consoles:** whether being hit interrupts a deactivation, whether several players can deactivate
  consoles at once, and whether deactivated consoles come back on.
- **Sprinting:** how fast sprinting is, whether players spawn with full energy, and whether a
  player can shoot while sprinting.
- **Hyperbolic levels:** their topology.
- **Warp objects:** how many black holes and wormholes each level has, and where.
- **Weapons:** exact fire rates, how fast an overheated weapon cools down, and how much and for how
  long a hit slows a player.

These may change later: more than one pylon per team per level, players placing warp objects, and
more than two levels per team.
