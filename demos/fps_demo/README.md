# FPS demo

`warped-fps-demo` is a single-player first-person shooter in Rust: SDL3 for the window, input
and audio, and a renderer that talks to Metal directly (`objc2-metal`, shaders in MSL). It is a
look-and-feel test for a "AAA-looking" FPS before curved space comes in. macOS on Apple silicon
only.

- One indoor/outdoor arena: a yard of asphalt, cover walls and props, and a warehouse you can
  walk into.
- Walk, sprint, crouch and jump, with box collision.
- A hitscan energy revolver (or the CC0 rifle, see Assets): recoil, aim-down-sights, reload,
  muzzle glow, an HDR beam, impact decals and sparks, synthesised sound.
- Robot bots that hunt you, keep a fighting distance, strafe and shoot back, less accurately
  from far away. Health, regeneration, death and respawn for you and the bots.
- Renderer: PBR materials with image-based lighting from the HDRI sky, cascaded shadow maps,
  HDR with ACES tone mapping and a color grade (white balance, log contrast, split toning),
  energy-conserving bloom, SSAO with multi-bounce occlusion, multi-scatter GGX specular, height
  fog that scatters blue more than red, ray-marched sun shafts through the shadow cascades (with
  dust in sheltered air), FXAA, procedural weapon sway, bob,
  recoil and reload motion. The internal render scale lowers the 3D resolution; the HUD stays
  at full resolution.

## Running

```sh
pushd demos/fps_demo
make download-assets
make run-fps-demo
popd
```

`make download-assets` needs Rust (rustup) and CMake (`brew install cmake`; SDL3 builds from
source). It downloads about 126 files into `.assets/` once; a second run downloads nothing.
`make help` lists every target, e.g. `make run-fps-demo RENDER_SCALE=0.7 BOT_COUNT=6`.
`warped-fps-demo --help` lists every flag (`--fullscreen`, `--no-vsync`, `--autopilot`,
`--show-frame-time`, `--rifle`, ...). `--camera-view yard|indoor|firing` holds the camera at
a fixed pose (firing holds the trigger), and `--disable-effect <name>` starts with an effect off,
for screenshots and per-effect GPU costs.

`make bench-fps-demo BENCH_SECONDS=30` lets the autopilot play and writes the frame-time
summary to `.artifacts/bench_fps_demo/<run_id>/`. Its choices are in
[`DECISIONS.md`](DECISIONS.md).

## Controls

| Input | Action |
| --- | --- |
| Mouse | look (click the window to capture the mouse) |
| `WASD` | move |
| `Shift` / `Ctrl` or `C` / `Space` | sprint / crouch / jump |
| Left / right mouse | fire / aim down sights |
| `R` | reload |
| `Esc` | release the mouse; a second `Esc` quits |
| `F1` / `F2` / `F3` | SSAO / bloom / FXAA on or off |
| `F4` / `F5` | frame-time readout / autopilot on or off |
| `F6` / `F7` / `F8` | color grade / volumetric haze / multi-scatter materials on or off |
| `[` / `]` | render scale down / up by 0.1 |

## Assets

No binaries are committed. `download-assets` writes everything into `.assets/` (git-ignored,
kept by `make clean`). The list is `src/asset_manifest.rs`.

| Asset | Source | License |
| --- | --- | --- |
| HDRI sky `kloofendal_48d_partly_cloudy_puresky` (4K) | https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky | CC0 |
| Textures `asphalt_02`, `concrete_floor_worn_001`, `brick_wall_02`, `concrete_wall_008`, `corrugated_iron_02`, `metal_plate`, `rusty_metal_02`, `concrete_pavement_02` (2K) | https://polyhaven.com/a/<name> | CC0 |
| Rifle `bolt_action_rifle_7_62` (2K) | https://polyhaven.com/a/bolt_action_rifle_7_62 | CC0 |
| Props `concrete_road_barrier`, `Barrel_01`, `barrel_03`, `old_military_crate`, `wooden_crate_02`, `metal_trash_can`, `utility_box_01`, `fire_hydrant`, `street_lamp_01`, `exterior_aircon_unit`, `old_tyre`, `portable_generator`, `propane_tank`, `cardboard_box_01`, `mounted_fluorescent_lights` (1K) | https://polyhaven.com/a/<name> | CC0 |
| Bot `RobotExpressive.glb` by Tomás Laulhé, modified by Don McCurdy | https://github.com/mrdoob/three.js/tree/r170/examples/models/gltf/RobotExpressive | CC0 |
| "Sci-fi revolver" by Ramhat (optional) | https://sketchfab.com/3d-models/none-f6bd1cbad4174048bcf02f8e52771adb | Sketchfab Standard |
| "FPS- Arms" by 3DHaupt (optional) | https://sketchfab.com/3d-models/none-22274e479c794377b58671f1652f61fc | CC BY-NC 4.0 (credit the author, non-commercial only) |

The two Sketchfab models need a Sketchfab account. `download-assets` fetches them when
`SKETCHFAB_API_TOKEN` is set (from https://sketchfab.com/settings/password). Otherwise download
their glTF zips by hand and unzip them so that these files exist:

- `.assets/sketchfab/sci_fi_revolver/scene.gltf`
- `.assets/sketchfab/fps_arms/scene.gltf`

Without the revolver, or with `--rifle`, the viewmodel is the Poly Haven rifle and the beam
stays. The arms model has six static poses and no rig; the demo uses one pose (`Arms_Obj.001`,
right arm only) and animates the whole viewmodel procedurally.

## Code layout

- `src/render/`: the `Renderer` trait (meshes, materials, textures, environment, one
  `render_frame` call per frame) and `metal/`, its only backend, with the MSL shaders. A Vulkan
  backend would implement the same trait.
- `src/assets/`: glTF (materials, skinning, animation), texture and HDRI loading.
- `src/game/`: arena and collision, player, weapon, bots, audio, HUD, and `Session`, which ties
  one frame of gameplay together.
- `src/scene.rs`: loads the assets and places the arena's draws and the viewmodel.
- `src/bin/download_assets.rs`: the asset downloader.

`cargo` writes to `.build/`. `make bench-fps-demo` writes to `.artifacts/`.

## Frame time

Autopilot with 4 bots, render scale 1.0, vsync on:

| Machine | Pixels | Mean | p99 | GPU |
| --- | --- | --- | --- | --- |
| MacBook Pro (Mac15,6, M3 Pro 11-core, 36 GB), Liquid Retina XDR, macOS 26.5.2, Rust 1.98.0, 12 s | 3024×1834 | 8.44 ms (vsync at 120 Hz) | 12.9 ms | 5.9 ms |
| Development VM (Apple M4 Pro virtual machine), macOS 26.6.2, Xcode 26.6, Rust 1.99.1, 40 s | 1600×1000 | 16.9 ms (vsync at 60 Hz) | 21.5 ms | 5.9 ms |

A virtual GPU isn't representative of a real MacBook Pro.

## Limitations

- Anti-aliasing is FXAA only; TAA is left for later.
- Collision is axis-aligned boxes; solid props have box colliders, small ones none.
- Bots steer straight toward you; there is no navigation mesh.
- The arms are one static pose; there is no left hand or finger animation.
- If there is no audio device, the demo runs silently.
