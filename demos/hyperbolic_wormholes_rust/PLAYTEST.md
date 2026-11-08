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
- To tune play (speeds, jump, both guns, consoles, respawn delay, the hit slowdown, the flashlight, thrown
  wormholes, the curvature), copy `server_settings.yaml`, change its numbers, and start the server with it:
  `make serve-game MAP=lunar-surface SERVER_SETTINGS=my_server.yaml`.
  Everyone who joins plays by the server's settings.
- Your own choices (keys, mouse speed, head bob, motion blur, how a hit looks) are in `player_settings.yaml`. The
  easiest way to change them is in the game: press `Tab` for the menu, `Up`/`Down` to pick a row, `Return` and then
  a key, button or the wheel to rebind an action, `Left`/`Right` to change a number, and `Return` on its last row
  to save. Saved choices go to `~/.config/warped/player_settings.yaml`, which every later game on that computer
  loads. `Esc` or `Tab` closes the menu. You can also copy the file, change it, and add `PLAYER_SETTINGS=mine.yaml`
  to `make join-game`.
- `make join-game MAP=lunar-surface` without `SERVER=` finds the server on the network by itself. If it doesn't
  within a few seconds, use the address the server printed. When several servers announce themselves, it joins the
  one on the usual port (47800) and prints the others.
- If a window can't play on its server, a banner says why, and the terminal says what to do:
  - **NO ANSWER FROM THE SERVER:** nothing came back for 2 s. Check the address and `PORT=`: use the port the server
    printed, not the port in its "player 1 joined ... (their window's address: ...)" lines, which belongs to the
    player's window. Or the server stopped.
  - **SERVER RUNS ANOTHER VERSION:** its snapshots don't fit this game. Pull, and rebuild and restart the server and
    every window from the same commit.
  - **THE GAME IS FULL:** the server already has 8 players.
  - **JOINING THE GAME:** you pressed `1` or `2`, and the server hasn't let you in yet.
- You can't shoot until the server has let you into the game (not from the team screen, nor while joining).
- When macOS asks, allow Local Network access (for Terminal or whichever app runs the game) and allow incoming
  connections. Without that, nobody can reach the game.
- `MAP=house-demo` (the default) plays the house with its three wormholes instead.
- Up to 8 players. A laser that reaches another player removes them: their view darkens, says "YOU DIED" with a
  countdown, and they come back at their spawn 3 seconds later.
- Joining opens a lobby that says CONNECTING until the server answers, then CONNECTED with how many players each
  team has: press `1` to join blue or `2` to join red. You keep your team when you come back after being hit. Lasers
  hit teammates too, but you can only turn off the other team's consoles. Bots join whichever team has fewer
  players.

To win on Lunar Surface, turn off all three of the other team's consoles, the pulsing block beside each of its three
pylons (one near its spawn, one on each flank; a red pylon is past the wormhole on the ridge). Stand within 3 m, look
straight at one and hold `E` for 10 s: a bar below the crosshair fills, the console pulses faster and faster for
everyone (from once a second to a frenzy of 8 times a second right before it turns off), and you can't shoot
meanwhile. Looking away or letting go starts it over, and its pulse slows back down over about a second. A console
that is off goes dark. When a team's consoles are all off, everyone sees which team won, and a new game starts only
when whoever started the server presses `Enter` in its terminal (the `make host-game` terminal, for that command):
every console comes back on and everyone returns to their spawn.

The habitat modules (the red one stands near the red base, on the side away from the ridge) and the covered walkways
have windows: glass in real 1 m openings at eye height. You see out of them, and a laser goes through them both ways,
but you can't climb through.

The crater's wall is Lunar Surface's edge, with no invisible wall any more. Climb it and you slow down without end:
each step outward is cut by how far you have already come, so you never get past halfway up the wall (about 10 to
14 m high). Your view stretches out from the middle of the screen, more and more as you slow. Walking back in is at
full speed. The server's `map_edge` settings set where the slowdown starts and stops; your own
`view.edge_stretch_frac` sets how strongly the view stretches (0 for none).

Space on Lunar Surface is 1.5 times as curved as before: its curvature radius is 26.7 m instead of 40 m. Buildings
and players keep their size, but far things shrink faster with distance, the crater's wall sinks toward the
horizon, and the structures away from the middle line are further apart. The server's
`space.curvature_per_map_curvature` sets it, from 1 (the old curvature) to 2, with 1.5 the default; a window that
joins builds the map at the server's value. Known rough: at 2, surfaces more than about 90 m from the middle of the
map (both spawns, the red pylon) speckle, because the renderer's single precision runs out that many curvature
radii out; at 1.5 there is almost none.

The bars in the bottom left (health, then the gun's ammo, then sprint energy) lean like `/` and glow in their
colours. While one changes, soft swells of light run over its filled part, stronger and faster the faster it
changes: after a hit, reddish-orange ones run down the health bar from your last hit point; while you sprint, pale
blue ones run down the energy bar from its last filled slot; while it refills, fainter ones run up toward it. The
ammo bar doesn't pulse.

Walk through a wormhole (on Lunar Surface, the one on the ridge leads to the red pylon) and, for the moment you
are inside it, within about 3.6 m of its centre on either side (about 0.6 s at walking speed), your view's colours
shift as if you moved near light speed: a light cyan, almost white, walking forward, a dark red walking backward
(option 8 and R2, which you picked), crisp, with dark staying dark. It is strongest at the throat and with your
speed, goes away the moment you stand still, and is gone 3.6 m out. Only you see your own shift. In the menu,
`wormhole tint 0-1` sets how strong it is (0 turns it off). A saved `player_settings.yaml` that still has the old
`wormhole_tint_s` line loads as before; that line is ignored.

The minimap in the top right shows the ground within 60 m of you, turned so you face up. It is a Poincaré disk, so
the faint 10 m grid and straight paths bend into arcs toward its rim. Other players are dots and shots are lines,
in their team's colours. The menu's MAP page rings the same 60 m around you and darkens the rest, and shows every
other player in the game, both teams, near or far, as a dot in their team's colour ringed in dark grey. To move,
resize or hide the minimap, change `minimap` in your `player_settings.yaml` (`corner: top_left`, `top_right`,
`bottom_left` or `bottom_right`).

Look up on Lunar Surface: spaceships fly slowly across the sky, purely for looks. A ring ship with a turning ring and
three delta-wing fighters in a V each cross from horizon to horizon in 2 to 5 minutes, with engine glow, red and green running
lights and a flashing strobe. They follow the UTC clock, so everyone whose clock is right
sees the same ships at the same moment. They are far away in the sky: lasers and players never touch them.

Far behind them hang two giants. From the blue spawn, look ahead and a little left: a moon-sized battle station
hovers low over the horizon, half lit by the sun, with a dish in its upper half, a dark trench round its middle with
small lights, a red beacon flashing on top and lit windows on its dark side. It barely moves and never sets. A long
wedge-shaped capital ship with a bridge tower, blue engines and rows of lit windows crosses the sky slowly: 30
minutes from horizon to horizon, then 6 below it. From the blue spawn it rises on the left and passes highest a
little right of ahead, 30° up. Both show through the wormholes and around the black hole, like the stars.

Controls (the defaults; to change them, edit the `keys` in a copy of `player_settings.yaml` and add
`PLAYER_SETTINGS=mine.yaml` to `make join-game`): click to capture the mouse (`Esc` releases it, `Esc` again asks "Do you want to exit?": `Return` or `Y` exits, `Esc` or `N` goes back to the game). `W`/`A`/`S`/`D` walk, `Shift` sprints (on Lunar Surface it uses the energy bar in the bottom left: 7.5 s of sprinting; once you let go of `Shift` it refills in 7.5 s crouched (`Ctrl`) or 15 s walking or standing),
`Space` jumps (on Lunar Surface, jump to get onto a block; hold it on the house for the jetpack), holding the right button aims (zooms in, 2.5 times with the sniper and 1.5 times with the rifle, and turns more slowly), holding `F` shines your flashlight (a cone of light that only you see for now), `V` switches to a third-person view from 3 m behind and a little above your character and back (it pulls in when a wall or the ground is behind you; shots still leave your head toward the crosshair, and aiming looks from your eye again), `Ctrl` (held) crouches
(a lower view, half speed), click to shoot (only standing still or crouched: not while walking upright, running, when
the wand in your hand swings low, nor in the air on Lunar Surface), the mouse wheel or
`1`/`2` switch between gun 1, the laser wand (a quarter-second shot per click, then 2 s to reload, shown by one block
above the energy bar that empties when you fire; one hit eliminates), and gun 2, the
rifle (a grey block with a thin barrel; hold the button to fire 8 pulses a second, each kicking your view up a little; 2 hits to the head or 12 to the
body eliminate; it overheats after 20 shots, shown in a bar above the energy bar, and cools in 2 s). Each shot strays
a little from the crosshair: the sniper's by a random few milliradians, the rifle's more and more as it heats up
during a burst (`DECISIONS.md` D89). The sniper's beam is a white lightsaber blade, the rifle's a laser. Each shot's
beam also flickers: each frame random pixels of it turn black, white or drop out to show what is behind (1 : 1 : 8),
a random share for each sniper shot (0.2 on average, at most half) and more and more as the rifle heats up, from none
to half the beam (D99). A shot into the ground, from either gun and anyone, throws up dust and a puff
of mist that fade within 2 s. `E` (held)
turns off one of the other team's consoles, `-` and `=` change the render resolution, `Tab` opens the menu on SCORES: how
many players there are and each one's team, kills and deaths in this game (`Q`/`E` turn to your settings, a map of the
whole level with you on it, and CREDITS: where the game's assets come from). There is no key to respawn:
players come back only after being eliminated.

## Known caveats

Multiplayer:

- Tested on one Mac only: a host, two bots and a window joined over loopback, on both maps (lasers hit, players
  rejoin). Not yet tried across two computers on Wi-Fi.
- A player whose `MAP` differs from the server's can't join: their game stops with an error naming the server's
  map.
- Each computer is trusted with its own moves and hits, so there is no protection against cheating.
- Lasers hit teammates too. On the SCORES page, eliminating a teammate counts their death but no kill.
- Lasers pass through glass; players don't (`DECISIONS.md` D90). The house's windows are glass laid on solid
  plank walls and drawn opaque, so there a shot at a window still stops on the wall behind it.
- Glass is drawn as a faint tint, so from outside a habitat module's windows look mostly like open holes.
- Health, shown in the bottom left, resets when a player respawns.
- The menu doesn't pause the game: a player in it stands still and can still be hit.
- A file saved from the menu has no comments. If a later version adds a setting, that file stops loading and the
  error names it: delete it (`~/.config/warped/player_settings.yaml`) or add the new setting.
- Consoles are found by name: a map's materials named "blue console …" and "red console …". The house has none,
  so nobody can win there.
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
- On Lunar Surface a shot's beam flies out from the gun's tip as a streak (D61), but a hit counts the moment you
  fire, so a target can drop before the streak reaches them.
- The window blends each frame with the last one a little (motion blur, D55), so fast turns smear for about 10 ms.
- Frames that take more than about 15.5 ms of GPU time are drawn at a lower resolution and upscaled (D19, D41), so
  very close approaches to a mouth look softer.
- Rays that never come out (trapped near a throat or out of iterations) are black on Lunar Surface; the house paints
  them from the pixels around them.
- The laser is traced on the CPU with the level's steps; where the picture uses longer steps (D40) the drawn beam
  can sit a few pixels off in the most lensed rings.
- Near the wormhole's throat the picture is about 4 pixels from a converged reference (a known cost of the level's
  steps, D34).
