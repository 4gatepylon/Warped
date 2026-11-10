# Level demos

One playable demo for each level of the default map in [`game_design/INSIDE_GAME_DESIGN.md`](../../game_design/INSIDE_GAME_DESIGN.md),
plus City Sphere with wormholes, which isn't in the design yet. Each is a first try at the README checklist's first item:
Minecraft-like visuals on one level, with real obstacles and good physics, without multiplayer. Each level was built
independently from the design doc, so the layouts are proposals to argue with, not decisions.

| Level | Team, depth | Space | Warp objects | Folder |
| --- | --- | --- | --- | --- |
| Dust | Blue, 2 | Hyperbolic | Black holes, wormholes | [`dust/`](dust/README.md) |
| Snow Planet | Blue, 1 | T² × ℝ | Black holes, wormholes | [`snow_planet/`](snow_planet/README.md) |
| City Sphere | Purple, 0 | Inside a sphere | None | [`city_sphere/`](city_sphere/README.md) |
| City Sphere with wormholes | Purple, 0 | Inside a sphere | Wormholes | [`city_sphere_wormholes/`](city_sphere_wormholes/README.md) |
| Dark Forest | Red, 1 | T² × ℝ | Black holes, wormholes | [`dark_forest/`](dark_forest/README.md) |
| Lunar Surface | Red, 2 | Hyperbolic | Black holes, wormholes | [`lunar_surface/`](lunar_surface/README.md) |

Each folder has the demo (`index.html`), a `README.md` (what to try, controls, how it works) and a `DECISIONS.md` (the
choices the design doc leaves open, such as the layout, sizes and rendering method, with the ones that need your input
flagged).

## Controls (the same in every level)

| Key | Action |
| --- | --- |
| Click / drag | capture the pointer / look |
| `W` `A` `S` `D` | move |
| `Shift` | sprint (uses energy: drains in 30 s, refills in 60 s) |
| `Space` | jump; in fly mode, up |
| `C` | in fly mode, down |
| `H` | toggle fly mode (in Dust and Lunar Surface, up to 10 curvature radii from the centre) |
| `E` (hold) | deactivate a console of the other team's pylon (10 s each) |
| Click (captured) or `R` | fire the laser |
| `T` | play as the other team |
| `1`–`9` | jump to the named viewpoints |

`#view=<name>` in the URL starts at a named viewpoint (each README lists them), and `#quality=low|medium|high|full`
fixes the render quality of the ray-traced levels.

## Known issues

- **Shots through wormholes can miss the crosshair** in Snow Planet, Dark Forest and City Sphere with wormholes. Those
  three trace the shot from the gun (0.22 m right of and 0.3 m ahead of the eye), and a throat magnifies that offset. City
  Sphere, Dust and Lunar Surface trace it from the eye and only draw the beam from the gun.
- The layouts, sizes and speeds are first guesses; each `DECISIONS.md` flags the ones that most need playtesting.
- Nobody plays the other team: the enemy figures, where there are any, don't move or shoot.

## Running

```bash
python3 -m http.server --directory demos
```

Then open, for example, `http://localhost:8000/levels/snow_planet/index.html`. The demos load three.js from a CDN, so
they need an internet connection.
