# Capturing screenshots and videos of the Rust demo

How to get pictures out of `demos/hyperbolic_wormholes_rust/` for the `graphical-changes` skill: offscreen views,
temporary hooks, live games, and running alongside other agents and the user's own game.

## Offscreen views

- **Bench views:** run, from the demo's folder:
  - the command: `./.build/release/hyperbolic-wormholes --map lunar-surface --bench-views --view <name> --frame-count 1
    --width-px 2560 --height-px 1600 --label <label>`;
  - it writes `.artifacts/bench_views/<run>-<label>/<view>.png`;
  - the views (`middle`, `red_pylon`, `blue_spawn`, `wormhole`, `black_hole`, ...) are listed where `main.rs` builds
    them with `view(...)`.
- **Labels** may use only letters, digits, `-` and `_`.

## Temporary hooks

When a picture needs something the views can't show (fake players, a sky time, a new camera, a menu page), add a
hook for the run only:

- **Set aside and restore.** Copy the file aside to your scratch folder first, and copy it back afterwards. Never
  `git checkout` a file: that throws away other uncommitted work in it.
- **Key every hook** to an environment variable named `TEMP_<ITEM>`, so it does nothing without it.
- **Where hooks go:**
  - **Scene and camera:** in `bench_voxel_views`, change the world or camera just before the frame is built. For
    example, add bodies with `world.with_avatars(...)`, placed with `VoxelPlayer::AtView(...).get_feet_frame(...)`.
  - **Overlay** (HUD, minimap, blueshift): in `render_to_pixels` in `render_metal.rs`.
  - **Menu and other text:** a temporary test that builds the page's `OverlayText` and calls `make_text_mask`. The
    mask is the game's own font and layout; turn it into a PNG with `save_png`.
  - **Data the shader draws** (minimap lines, paths): a temporary test that writes the data to a file, then a Pillow
    plot. Label it "Not a game screenshot".
  - **Motion with real physics:** a temporary test or hook that steps `VoxelPlayer::step` with the real input, every
    1/60 s, and renders or records each step.
- **Clean up:**
  - copy every file back;
  - check that `grep -rn TEMP_ src/` finds nothing;
  - delete your run folders under `.artifacts/`, unless they hold the pictures you keep;
  - rebuild, so the binary in `.build/` no longer contains the hook.

## Live games

- **Windows need a server.** A window must join a server (#212):
  1. start one with `--serve --port <yours>`;
  2. join it with `--join 127.0.0.1:<yours> --exit-after-s 8`;
  3. use several `--join` windows for multiplayer pictures.
- **First frame:** it takes 4 to 7 seconds, so time scripted actions from the first frame, not from launch.
- **Keys and clicks can't be scripted** in a real window. Drive a test game through a temporary hook instead: the
  same functions the keys call (`throw_wormhole()`, firing, picking a team), and a frame read back from the
  `CAMetalLayer` in `render_to_layer`.
- **Settings:** run windows with `--player-settings player_settings.yaml`, so nothing is read from the home folder.

## Sharing the machine

- **Your own ports.** Use only the ports you were given. Never join, stop or measure the user's own game (port 47800
  by default), and stop every server and window you started before you finish.
- **Busy GPU, rough timings.** While other agents render or the user plays, frame times can vary by 2× from run to
  run. To compare options:
  - alternate their runs (A, B, A, B, ...);
  - take each option's fastest round;
  - say how busy the machine was (load average, other game processes);
  - don't judge an absolute frame rate until a quiet run.
- **Record the system** with the `system-docs` skill when you report a measured time.

## Sheets and GIFs

- **Pillow:** assemble sheets and GIFs with Pillow, run as `python3 -I <script>`, with scripts kept in your scratch
  folder or worktree, never in a folder of downloads.
- **Labels:** use `ImageFont.load_default()`; don't read fonts from the system or home folders.
- **GIFs:** keep each under about 5 MB (crop, scale or drop frames). Label the playback speed when it isn't real
  time.
- **Where the outputs go:** keep the final pictures in the run folder, `.artifacts/<script>/<run_id>/`, with its
  `command.txt` (root `AGENTS.md`).
