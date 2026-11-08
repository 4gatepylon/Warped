# Playtest: play together on one Wi-Fi

How to get a few people into one game of the Rust wormholes demo, and what is known to be rough. Report anything
else you notice.

## Set up each Mac (once)

You need a Mac with Apple silicon (the game draws with Metal and upscales with MetalFX).

```bash
xcode-select --install
brew install cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/4gatepylon/Warped.git
```

- `xcode-select --install` gives the C compiler, and CMake builds SDL3 (the window library) from source. Skip
  whichever you already have.
- The repository is private: each player needs read access on GitHub to clone it.
- `main` is the official version; `staging` is the latest, with fixes that reach `main` when a person merges
  them. Everyone in a game should be on the same one.
- For the official version: `git -C Warped switch main && git -C Warped pull`. For the latest:
  `git -C Warped switch staging && git -C Warped pull`.
- From a fork, GitHub's "Sync fork" updates the fork's `main`. For the latest, take `staging` from this
  repository: `git -C Warped remote add upstream https://github.com/4gatepylon/Warped.git`, then
  `git -C Warped fetch upstream staging && git -C Warped switch -c staging upstream/staging`, and
  `git -C Warped pull upstream staging` to update later.
- The first build takes a few minutes (SDL3 compiles from source); later ones take seconds.

## Play

Everyone joins the same Wi-Fi and uses the same `MAP`. One person starts a game server; everyone, that person
included, joins it with a window:

```bash
cd Warped/demos/hyperbolic_wormholes_rust
make serve-game MAP=lunar-surface
make join-game MAP=lunar-surface SERVER=192.168.1.20
```

- `make serve-game` runs only the server, without a window, and prints this computer's address and the exact
  `make join-game` commands to give the others (`SERVER=127.0.0.1` on the same computer). Leave it running in its
  terminal; `Ctrl-C` ends the game.
- `make host-game MAP=lunar-surface` does both in one terminal: it starts the same server as its own process, opens
  your window in it, and stops the server when your window closes.
- `make join-game MAP=lunar-surface` without `SERVER=` finds the server on the network by itself. If it doesn't
  within a few seconds, use the address the server printed.
- When macOS asks, allow Local Network access (for Terminal or whichever app runs the game) and allow incoming
  connections. Without that, nobody can reach the game.
- `MAP=house-demo` (the default) plays the house with its three wormholes instead.
- Up to 8 players. A laser that reaches another player removes them: their view darkens, says "YOU DIED" with a
  countdown, and they come back at their spawn 3 seconds later.

Controls: click to capture the mouse (`Esc` releases it, `Esc` again quits). `W`/`A`/`S`/`D` walk, `Shift` sprints (on Lunar Surface it uses the energy bar in the bottom left: 7.5 s of sprinting, 15 s to refill once you let go of `Shift`),
`Space` jumps (on Lunar Surface, jump to get onto a block; hold it on the house for the jetpack), click to shoot (a
quarter-second laser shot; not while running, when the gun in the bottom right jogs, nor in the air on Lunar Surface), `R` respawns, `-` and `=`
change the render resolution.

## Known caveats

Multiplayer:

- Tested on one Mac only: a host, two bots and a window joined over loopback, on both maps (lasers hit, players
  rejoin). Not yet tried across two computers on Wi-Fi.
- The game doesn't check that everyone uses the same map; a player on another map sees the others in the wrong
  places.
- Each computer is trusted with its own moves and hits, so there is no protection against cheating.
- Local network only: no play over the internet. Networks that keep devices apart (many guest and office Wi-Fi
  networks) stop players from reaching each other; a home network or a phone's hotspot works.
- The game uses UDP port 47800 and announces itself on UDP port 47801 (`PORT=` changes the first).

Graphics on Lunar Surface, to fix in later pull requests:

- Rays still going after 90 iterations take steps three times longer (`DECISIONS.md` D40), so the thinnest rings
  just inside a wormhole mouth differ slightly from the WebGL level's.
- The sky seen through the lenses differs slightly from the WebGL level's: escaping rays read it where their bending
  ends, not where their last step ended (D39).
- Neighbouring pixels on one surface are interpolated rather than traced (D37); textures smaller than a pixel
  shimmer, as they do when every pixel is traced.
- Frames that take more than about 15.5 ms of GPU time are drawn at a lower resolution and upscaled (D19, D41), so
  very close approaches to a mouth look softer.
- Rays that never come out (trapped near a throat or out of iterations) are black on Lunar Surface; the house paints
  them from the pixels around them.
- The laser is traced on the CPU with the level's steps; where the picture uses longer steps (D40) the drawn beam
  can sit a few pixels off in the most lensed rings.
- Near the wormhole's throat the picture is about 4 pixels from a converged reference (a known cost of the level's
  steps, D34).
