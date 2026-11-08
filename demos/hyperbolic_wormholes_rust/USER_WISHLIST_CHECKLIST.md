# User wishlist checklist

Every feature and fix the user has asked for in the Rust wormholes demo, so that agents on any platform (Claude Code,
Codex, Devin) can see what is wanted, who is working on what, and what is done. How to add, claim and finish items
is in [`AGENTS.md`](AGENTS.md).

- `[ ]` open: nobody has started it.
- `[~]` claimed: someone is working on it; the line says who, on which branch, since when.
- `[x]` done: merged into `staging`; the line names the PR.

## Claimed

- [~] **W-60 Throw wormholes.** Each player has a purple and a green wormhole. `Q` throws one, then the next press
  throws the other. These are each player's linked wormholes. Claimed by Claude Code (session
  `bc330d62-7d04-4172-98e2-16546720d58b`), branch `claude/throw-wormholes`, 2026-11-07.

## Open

- [ ] **W-37 Stronger curvature.** The curvature can be set in the YAML settings, higher than now: twice the current
  curvature by default (and at most twice).
- [ ] **W-40 Graphics caveats.** Fix the Lunar Surface graphics caveats listed in `PLAYTEST.md` ("Graphics on Lunar
  Surface, to fix in later pull requests"); slightly wrong graphics were accepted for playtesting "and we can fix
  them in a following PR". Among them: rays trapped near a throat are black on Lunar Surface (the black band seen
  when turning inside a wormhole, which the house already paints from the pixels around it; "it doesn't need to be
  100% correct physically"), and Lunar Surface should match the WebGL level.
- [ ] **W-41 Play across computers.** Check that people on two or more computers on the same Wi-Fi can play
  together ("the user can play the game with other humans on the same wifi"); so far it has only been tested on one
  Mac over loopback.
- [ ] **W-44 Smooth wormhole interior.** Inside a wormhole, looking sideways shows a grainy texture; it should look
  pretty, and a trivial fix such as blurring is fine. Looking at a wormhole from outside must stay correct.
- [ ] **W-47 Admin dashboard.** Change the server's settings in real time: someone connects as an "admin", who can't
  play but has a dashboard where they can change or reset those settings.
- [ ] **W-51 Basic visuals.** Players can change some basic visuals, such as saturation, so the game isn't always so
  dark.
- [ ] **W-52 Cyan through a wormhole.** Going through a wormhole turns the whole screen somewhat cyan, simulating a
  relativistic effect on colour if possible.
- [ ] **W-53 Spaceships overhead.** A few spaceships flying overhead in the sky, purely background world-building, to
  look cool and pretty.
- [ ] **W-54 Professional HUD bars.** Make the health, ammo and other bars look more professional: at the very least
  slightly translucent, slightly shiny and rounded. Show the user a few options to pick the best from before
  implementing.
- [ ] **W-55 Aiming.** Players can aim (right click by default), so the flashlight moves off right click to a
  different key.
- [ ] **W-56 Dust from ground shots.** Shooting the ground throws some mist and dust particles up into the air.
- [ ] **W-57 Laser stuck in a room.** In one of the rooms on the red side, the one with windows one block wide,
  shooting out from close to a window hits some sort of hitbox (or looks like it) and the laser doesn't leave the
  room.
- [ ] **W-42 Server in the cloud.** Run the headless game server in the cloud so players can join over the internet
  (the user's "launch from i.e. cloud in the near future"); today the game works on a local network only.
- [ ] **W-58 Installable bundle.** Define the map in some kind of declarative form so that it can be put into an app
  bundle that other people can install or port without needing the GitHub repository; more generally, refactor
  things so there is an easy "bundle, then give the bundle to another person, and they can run it easily". Before
  starting, talk with the user about how to do it, rather than doing it in some random way.

  The user asked to fold in this plan, which another agent wrote, as part of this item: a **downloadable Mac app**.
  The game is played among friends: a friend downloads a zip, unzips it and runs the game on an Apple silicon Mac
  (macOS 13+), then hosts or joins a game on the same Wi-Fi, without a terminal, Rust or a checkout of the repo. Not
  for the App Store, and no paid Apple developer account.
  1. **Built-in maps:** build the maps (`common/maps/house_demo` and `lunar_surface`, about 136 KB) into the
     program. Today `get_map_folder` finds them through the path to the checkout, which is fixed at build time, so
     they aren't found on other Macs.
  2. **Title screen:** a double-clicked app gets no flags, so it opens on a screen with Host, Join (finds the game on
     the local network) and a map choice. Build it on the W-46 settings menu.
  3. **Logs and errors:** log to `~/Library/Logs/` instead of the terminal, and show fatal errors in a pop-up.
  4. **Version check:** put a version number in the join messages, and refuse a player whose version doesn't match
     the server's, with a clear message, since friends will download at different times.
  5. **App and zip:** a `make app` target that builds the .app (`Info.plist` with name, icon, minimum macOS 13, and a
     line explaining the local network use), signs it ad hoc (free), and zips it with `ditto -c -k --keepParent` so
     the app survives unzipping.
  6. **Local network permission:** as its own app, macOS asks the player whether the game may find devices on the
     local network. If the player refuses, the game says so clearly instead of failing silently.
  7. **First-open instructions:** without notarization, macOS blocks a downloaded app the first time. `PLAYTEST.md`
     and a short note in the zip tell players to open the app once, then click System Settings → Privacy & Security
     → Open Anyway.
  8. **Hosting the zip:** the repo is private, so friends can't download from its GitHub Releases unless added to
     the repo. Ask the user where the zip goes (for example a shared Drive or Dropbox link).

  Not part of this item: Apple notarization (needs the paid account), the App Store, Windows and Linux (the Metal
  renderer would need a wgpu port), Intel Macs, and play over the internet (W-42).
- [ ] **W-61 Third-person view.** A way to switch to a third-person view: the camera zooms out to slightly behind the
  character, and the player controls it from there, seeing their own character.
- [ ] **W-63 Missing textures.** Get the proper textures, and/or fail fast when textures are missing: "the aquarium's
  fork merge into this forgot to git add the textures and I think they'll be added soon but they are still
  missing".
- [ ] **W-64 Zone planes.** The painted lines that mark the edges of the blue and red zones become planes jutting out
  of the ground, just barely translucent, to mark out the real zones.
- [ ] **W-65 Confirm before exiting.** Pressing `Escape` doesn't exit straight away: the game asks "Do you want to
  exit?", exits if the player presses OK, and otherwise doesn't.
- [ ] **W-67 Consoles pulse faster and faster.** While a console is being turned off, its pulsing gets faster
  continuously, reaching a fervor right before the console turns off. If the turning off stops (a "disconnect"), the
  pulsing slows down smoothly instead of jumping back.

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
- [x] **W-43 Hold for the flashlight.** The flashlight stays on only while the right button is held. #133.
- [x] **W-39 Key bindings.** Which key crouches, moves, jumps, switches weapons and so on can be set in YAML
  (now the `keys` of `player_settings.yaml`, each player's own). #135.
- [x] **W-45 Player and server settings.** The YAML settings are split into `player_settings.yaml`, each
  player's own, and `server_settings.yaml`, the server's. #140.
- [x] **W-46 Settings menu.** A screen to exit to a menu where a player changes their own choices (hotkeys and the
  rest of the per-player settings): `Tab` opens it. #143.
- [x] **W-49 Menu with pages.** The menu where players change their hotkeys and the rest is a general menu with
  pages: one page for the hotkeys, another showing a layout of the entire map (`Q`/`E` turn pages). #149.
- [x] **W-59 Smooth crater instead of voxels.** Get rid of voxels and replace them with a proper 3D world: an uneven
  lunar surface texture from an online source, the Lunar Surface level inside a crater whose walls are the level's
  walls, a smooth noise-based environment with no voxels at all, smooth shapes for the objects, and the ridge, peak
  and boulders as hills and mounds in the terrain. #147.
- [x] **W-62 Sniper ammo bar.** The sniper gets an ammo bar too, but of only one block instead of many: it empties
  when the sniper fires and comes back when the timer resets, letting the player shoot again (a 2 s reload,
  `sniper.reload_s`). #156.
- [x] **W-38 Soft map edge.** Instead of a wall at the map's edge, players get slower and slower as they move off the
  map, so they can never leave it, and things stretch longer and longer: a relativistic barrier rather than a block.
  Since the map update (W-59), the hills at the edge may block players from leaving: the "Zeno's paradox" slowdown
  should start when a player climbs the hill, to stop them from leaving. As they slow down, their view of the world
  should warp so it looks as if they are being stretched ever further. #159.
- [x] **W-50 Scoreboard page.** A menu page that shows how many players there are, and each player's kills, deaths
  and team. #164.
- [x] **W-48 Minimap.** A minimap that shows off the map's hyperbolic nature nicely. Its place can be picked at
  least from top or bottom and left or right. Players show on it, and so do laser shots that fall in its range; it
  doesn't show the entire map. It is a circle that shows just where the player is and what is around them (the
  menu's map page, W-49, shows the whole map). #PR.
- [x] **W-66 Your range on the map page.** The map in the menu shows a circle around where the player is, with a
  radius (the rest is slightly darker and blurred), and shows where they are on the map. #PR.
