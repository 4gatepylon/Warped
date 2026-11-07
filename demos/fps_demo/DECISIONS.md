# Decisions: FPS demo frame time

- **Question:** How long does one frame of the FPS demo take on a Mac, CPU and GPU, so that we
  know how far it is from the 120 fps target on a MacBook Pro?
- **Given:** Rust + SDL3 + direct Metal, native Retina resolution with an internal render scale,
  target 120 fps on Apple-silicon MacBook Pros, and report the frame time measured on the VM.
- **Outcomes:** mean, median and p99 frame time and mean GPU time per frame. On a real MacBook
  Pro the GPU time could fit in 8.3 ms (120 fps) or need a lower render scale.

## Workload

### D1. Autopilot plays the default arena with 4 bots

- **Choice:** `--autopilot` walks between waypoints, aims at and shoots visible bots, and
  reloads, so fights, beams, particles and bot deaths are on screen.
- **Alternatives:** a fixed camera flythrough; recorded human input.
- **Rationale:** it exercises the whole frame (gameplay, effects, every render pass) without a
  person, but its path depends on the bots, so two runs aren't identical.
- **Revisit if:** runs vary too much to compare a change; then add a fixed flythrough.

### D4. Fixed camera views for lighting comparisons

- **Choice:** `--camera-view yard|indoor|firing` holds the player at one pose each (outdoor yard,
  inside the warehouse under the roof gap, and the yard with the trigger held), with
  `--bot-count 0`, for before/after screenshots and per-effect GPU costs.
- **Alternatives:** the autopilot route (D1), which mixes all three kinds of view.
- **Rationale:** a fixed view makes screenshots comparable and removes the route's variance from
  a per-effect difference.

## Procedure

### D2. 30 s by default, vsync on

- **Choice:** `make bench-fps-demo` runs 30 s with vsync on, at render scale 1.0.
- **Alternatives:** `--no-vsync` to measure the uncapped frame time.
- **Rationale:** vsync matches how it is played; the GPU time shows the headroom under the cap.
  The duration is arbitrary; anything from 20 to 60 s gives stable means.

## Metrics

### D3. Frame time from the CPU clock, GPU time from Metal

- **Choice:** frame time is the time between frames; GPU time is the command buffer's
  `GPUEndTime - GPUStartTime`. Reported every 5 s and for the whole run: mean, median, p99.
- **Alternatives:** Xcode's Metal frame capture per pass.
- **Rationale:** both numbers come from the demo itself, so any machine can reproduce them with
  one command; per-pass profiling is for when we optimise.

### D5. Per-effect cost is all-on minus one-off, no vsync, 3 interleaved repeats

- **Choice:** each new effect's GPU cost is the mean GPU time with every effect on minus the mean
  with only that effect off (`--disable-effect`), 8 s per run with `--no-vsync`, for each fixed
  view, repeated 3 times with the configurations interleaved; the median of the repeats is
  reported.
- **Alternatives:** per-pass timing in an Xcode Metal capture; vsync on (D2).
- **Rationale:** on the development VM, vsync-on GPU times varied between 3 and 16 ms for the same
  view, so the difference between effects was hidden; without vsync the GPU stays busy and runs
  agree to about 0.5 ms.
- **Revisit if:** the numbers are measured on a real MacBook Pro, where per-pass Metal capture
  gives each effect's cost directly.
