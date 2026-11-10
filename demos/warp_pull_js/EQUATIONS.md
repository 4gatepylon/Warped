# Pull toward wormholes and sprinting: the equations (W-202)

These are the equations proposed for W-202 (pull toward wormholes and black holes in the game settings), for the user to
confirm before the levels get them. `pull_dynamics.js` implements exactly these; the page (`index.html`) plays them.

## What exists today

- **Pull:** a speed added to the walk, not an acceleration: up to 0.75 of walking speed toward a warp, full within 3 m of
  its throat or horizon, fading to nothing 24 m out, and for a mouth fading to nothing at the throat. There is no
  momentum, so nothing carries through a mouth.
- **Sprint:** the speed jumps from walking speed to sprint speed the moment Shift is held, and back when it is let go.

## Symbols

| Symbol | Meaning | Proposed default |
| --- | --- | --- |
| `R` | pull radius, from the warp's centre | 5 m (2.5 bodies of 2 m) |
| `A` | pull strength | 11.9 m/s² (see "The default strength") |
| `rho` | radius of the mouth's throat (or the black hole's horizon) | the warp's own (1.6 m for a thrown mouth) |
| `v_walk`, `v_sprint` | walking and sprint top speed | the level's (Snow Planet: 8 and 15.5 m/s) |
| `a_up`, `a_down` | how fast sprinting speeds up and slows down | 15 m/s² each |
| `b` | braking of the drift outside every pull radius | 15 m/s² |
| `dt` | time step | 1/120 s |

## State of a player on the ground

- `x`: position along the ground.
- `s`: the speed the legs give (between `v_walk` and `v_sprint`).
- `u`: the drift, the velocity the warps have given the player (a vector along the ground).
- `h`: the unit direction of the move keys, or 0 with no key held.

## Sprint

```
target = v_sprint   if Shift is held, a move key is held, and energy > 0
         v_walk     otherwise

ds/dt  = +a_up      if s < target   (never past target)
         -a_down    if s > target   (never below target)
```

Walking itself still starts and stops at once, as today; only the part of the speed above walking ramps. Letting go of
Shift, or running out of energy, starts the slowing down at once. Energy drains and refills as today.

## Pull

For each warp `i` with centre `c_i`, at distance `d_i = |c_i - x|` and with `n_i` the unit direction from the player to
`c_i` along the ground:

```
g_i = A * n_i       if rho_i < d_i <= R
      0             otherwise

g   = sum of g_i over all warps
```

The pull is the same size everywhere inside the radius and points straight at the centre (isotropic).

## Drift

```
du/dt = g                              while inside some warp's radius
d|u|/dt = -b  (until u = 0)            outside every radius
```

Inside a radius nothing brakes the drift, so a player who touches nothing keeps swinging. Outside, the drift dies away
quickly, so a player thrown out of a mouth doesn't slide forever.

## Motion

```
v     = s * h + u
dx/dt = v
```

Each step: update `s`, add `g * dt` to `u` (or brake it), then move by `v * dt` (semi-implicit Euler).

## Through a mouth

When the player crosses mouth 1's throat going inward at the point `c_1 + e * rho` (`e` a unit vector), the player
comes out of mouth 2 at the opposite point `c_2 - e * rho`. `s` and `u` are unchanged, so the player leaves mouth 2
exactly as fast as it entered mouth 1, and mouth 2's pull then points back at mouth 2.

## What follows

- **Falling in from rest** at distance `d_0`: speed at the throat `v_t = sqrt(2 A (d_0 - rho))`.
- **Swinging:** the player comes out of mouth 2 at `v_t`, is slowed by `A`, stops `d_0` from mouth 2, falls back in, and
  so on, with period `T = 4 sqrt(2 (d_0 - rho) / A)`. With the defaults from the radius: `v_t` = 9 m/s, `T` = 3 s.
- **Walking out:** a player at rest at distance `d`, walking straight away, gets out if `R - d < v_walk^2 / (2 A)`.
  With the defaults that is 2.7 m, so walking out works from more than 2.3 m from the centre (0.7 m outside the throat);
  closer than that, walking is not enough and the player goes through.
- **Sprinting out:** `A` is below `a_up`, so a sprinting player always pulls away.

## The default strength

Today's pull is a speed, so "1.5 times today's" needs a reading. Proposed: a player who falls from the radius to the
throat arrives 1.5 times as fast as today's strongest pull speed:

```
sqrt(2 A (R - rho)) = 1.5 * 0.75 * v_walk     =>     A = (1.125 v_walk)^2 / (2 (R - rho))
```

With `v_walk` = 8 m/s, `R` = 5 m and `rho` = 1.6 m: `A` = 11.9 m/s².

## Open points for the user

1. **Shape of the pull.** Constant inside the radius (above) swings, but its period grows with the swing's size; a true
   harmonic oscillator needs a pull that grows with the distance from the throat (weakest right at the mouth), which
   fits "get too close and you get plopped through" less well. The page has both, and a third that is strongest at the
   mouth, to compare.
2. **Braking outside the radius** (`b`): proposed, not asked for.
3. **Black holes** get the same pull with their horizon as `rho`; what happens at the horizon stays as today.
4. **Jumping and flying** players are not pulled, as today.
