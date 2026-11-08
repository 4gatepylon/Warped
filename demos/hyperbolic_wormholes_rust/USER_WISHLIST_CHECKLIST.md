# User wishlist checklist

Every feature and fix the user has asked for in the Rust wormholes demo, so that agents on any platform (Claude Code,
Codex, Devin) can see what is wanted, who is working on what, and what is done. How to add, claim and finish items
is in [`AGENTS.md`](AGENTS.md).

- `[ ]` open: nobody has started it.
- `[~]` claimed: someone is working on it; the line says who, on which branch, since when.
- `[x]` done: merged into `staging`; the line names the PR.

## Claimed

- [~] **W-43 Hold for the flashlight.** The flashlight shouldn't toggle: it stays on only while the right button is
  held. Claimed by Claude Code (session
  `wizardly-hamilton-23a121`), branch `claude/flashlight-hold`, 2026-11-07.

## Open

- [ ] **W-37 Stronger curvature.** The curvature can be set in the YAML settings, higher than now: twice the current
  curvature by default (and at most twice).
- [ ] **W-38 Soft map edge.** Instead of a wall at the map's edge, players get slower and slower as they move off the
  map, so they can never leave it, and things stretch longer and longer: a relativistic barrier rather than a block.
- [ ] **W-39 Key bindings.** Which key crouches, moves, jumps, switches weapons and so on can be set in YAML.
- [ ] **W-40 Graphics caveats.** Fix the Lunar Surface graphics caveats listed in `PLAYTEST.md` ("Graphics on Lunar
  Surface, to fix in later pull requests"); slightly wrong graphics were accepted for playtesting "and we can fix
  them in a following PR". Among them: rays trapped near a throat are black on Lunar Surface (the black band seen
  when turning inside a wormhole, which the house already paints from the pixels around it; "it doesn't need to be
  100% correct physically"), and Lunar Surface should match the WebGL level.
- [ ] **W-41 Play across computers.** Check that people on two or more computers on the same Wi-Fi can play
  together ("the user can play the game with other humans on the same wifi"); so far it has only been tested on one
  Mac over loopback.
- [ ] **W-42 Server in the cloud.** Run the headless game server in the cloud so players can join over the internet
  (the user's "launch from i.e. cloud in the near future"); today the game works on a local network only.

## Done

- [x] **W-01 Fast Rust renderer.** The wormholes demo in Rust and Metal at 60+ fps at full resolution, on the house
  and on Lunar Surface. #73, #76, #81–#86, #90, #91.
- [x] **W-02 Multiplayer on one network.** Host and join a game on the same Wi-Fi, on `house_demo` and
  `lunar_surface`, with lasers that eliminate players. #77, #79, #80.
- [x] **W-03 Playtest guide.** Setup and the commands to play together, with known caveats; `main` official,
  `staging` the latest. #92, #94, #98, #99.
- [x] **W-04 Click to shoot.** #93.
- [x] **W-05 Laser crash and invisible lasers.** No crash when a laser hits a player on Lunar Surface, and everyone
  sees every laser. #95.
- [x] **W-06 No wall climbing.** Walking along a wall doesn't push the player up to its top. #96.
- [x] **W-07 PRs into `staging`.** PRs go to `staging` (agents may merge there); only humans merge into `main`, or an
  agent when a human explicitly asks. #97.
- [x] **W-08 Jump to climb.** Players need `Space` to get onto a block; no sliding up blocks. #100.
- [x] **W-09 Walk on structures.** Players above the lowest ground level walk without jumping; jumping is only for
  going up a level. #101.
- [x] **W-10 Respawn countdown.** A "YOU DIED" message and a visible respawn timer. #102.
- [x] **W-11 Movement feel.** A head bob, walking and running twice as fast, a sprint energy bar as in the levels,
  and three times the gravity. #103.
- [x] **W-12 Gun on screen.** A simple moving gun that jogs when running and points forward when walking; no
  shooting while running. #104.
- [x] **W-13 Only the game's keys.** WASD to move, `Space` to jump, click to shoot; `E` no longer jumps, and the keys
  that gave an edge are gone. #105.
- [x] **W-14 Smaller energy bar, more head bob.** Energy runs out four times faster; a deeper head bob. #106.
- [x] **W-15 No running on an empty bar.** A player can run only with energy left. #107.
- [x] **W-16 No shooting while jumping.** #107.
- [x] **W-17 Snappy jumps.** Gravity that feels three times as strong: quick jumps and fast falls. #107.
- [x] **W-18 Headless server.** A game server without a window that prints its address; every player, the launcher
  included, joins as a client. #108.
- [x] **W-19 Win by consoles.** Hold `E` on the other team's consoles to turn them off. Consoles pulse with bright
  light while on, twice as fast while being turned off, and go dark once off. When a team's consoles are all off the
  game ends and says who won, and the server's launcher starts a new game. #110, #115.
- [x] **W-20 3D wand.** A prettier gun: a rectangular prism with a coloured tip that turns like a real object. #111.
- [x] **W-21 Motion blur.** A tiny bit. #112.
- [x] **W-22 Second gun.** The design's rifle as gun 2, looking different from the wand, switched with the mouse
  wheel or `1`/`2`. The game design says how a player with two guns switches and which keys do what. #113.
- [x] **W-23 Laser from the tip.** The laser leaves from the wand's tip. #114.
- [x] **W-24 Consoles turn off.** Fix: finishing a hold on a console did nothing (a server running another map).
  #115.
- [x] **W-25 Rifle recoil.** #116.
- [x] **W-26 Settings in YAML.** Speeds, head bob, shots per second and the other game numbers in a well-organised,
  easy-to-understand YAML file that the server takes with a flag and sends to every player. #117.
- [x] **W-27 Flying beams.** The laser disappears from the tip onward and flies forward like a bullet, lasting longer
  for the sniper than the rifle; speed and trail per gun in the YAML. #118.
- [x] **W-28 Many players, apart.** Several players at once, never spawning one on top of another. #119.
- [x] **W-29 Health bar.** A discrete health bar so players see they were hit; the ammo and energy bars in segments
  too. #120.
- [x] **W-30 Pink sky noise.** Fix the flickering pink noise flooding the sky when shooting into the sky or at the
  black hole's edge. #122, #124.
- [x] **W-31 Hit effects.** A player who is hit is slowed (set in the YAML), sees red and a blur come in from the
  screen's edges, and the screen shakes. #123.
- [x] **W-32 Crouch.** `Ctrl` crouches; walking crouched is slower; players can shoot crouched but not while walking
  upright. #125.
- [x] **W-33 No respawn by choice.** #126.
- [x] **W-34 Crosshair.** #127.
- [x] **W-35 Flashlight.** Right-click turns on a flashlight that shines in a neat cone. #129.
- [x] **W-36 Lobby.** A lobby page where players see how many players are on each team, choose a team, and then
  play as that team; friendly fire stays on, and only the enemy's consoles can be turned off. #131.
