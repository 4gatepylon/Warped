# Warped game design

This document formalizes the notes in [`brainstorm/USER_THOUGHTS.md`](brainstorm/USER_THOUGHTS.md).
Every level and every weapon is described by the same parameters: the rows of its table. To add a
level or a weapon, add a column and fill in every row. To add a parameter, add a row and fill it in
for every column.

- **≈** marks a first guess, to be tuned in playtests.
- **TBD** marks something not designed yet.

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

Each team's wavefront only moves deeper, and every level but City Sphere can be played many times.
For example:

| Round | Level | Attacker | Winner | Next round |
| --- | --- | --- | --- | --- |
| 1 | City Sphere | None | Blue | Blue attacks its wavefront, Dark Forest. |
| 2 | Dark Forest | Blue | Blue | Blue's wavefront moves deeper, to Lunar Surface. |
| 3 | Lunar Surface | Blue | Red | Red attacks its wavefront, Snow Planet. |
| 4 | Snow Planet | Red | Blue | Blue attacks again, at Lunar Surface, where it left off. |
| 5 | Lunar Surface | Blue | Blue | None: blue won red's deepest level, so blue wins the match. |

## Levels

| Parameter | Dust | Snow Planet | City Sphere | Dark Forest | Lunar Surface |
| --- | --- | --- | --- | --- | --- |
| Team | Blue | Blue | Purple | Red | Red |
| Depth | 2 | 1 | 0 | 1 | 2 |
| Theme | Dust (desert) planet | Snow planet | City, or space station | Dark forest planet | Lunar surface |
| Space | Hyperbolic | T² × ℝ | Inside a sphere | T² × ℝ | Hyperbolic |
| Gravity | TBD | Down | Outward, onto the sphere | Down | TBD |
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
- **Hyperbolic:** hyperbolic space with warp objects. The pylons are far apart. The topology and
  the direction of gravity are TBD.

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

- **Match length:** whether a match has a time or round limit, since it goes back and forth for as
  long as defenders keep winning.
- **Consoles:** whether progress on a console resets when a player lets go of E or is hit, whether
  several players can deactivate consoles at once, and whether deactivated consoles come back on.
- **Respawning:** how long a player waits to respawn, and where.
- **Hyperbolic levels:** their topology and the direction of gravity.
- **Warp objects:** how many black holes and wormholes each level has, and where.
- **Weapons:** exact fire rates, how fast an overheated weapon cools down, and how much and for how
  long a hit slows a player.

These may change later: more than one pylon per team per level, players placing warp objects, and
more than two levels per team.
