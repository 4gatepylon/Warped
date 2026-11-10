# Warped, the one-window app (the official MVP)

`warped` is the whole game in one window, like a normal game (W-141, one window for the whole
game): a main menu, a game menu, and each round's level in the same window. The match server, the
level servers and the bots are headless programs that the app starts as its own child processes; no
second window opens.

## Play

```bash
export PATH="$(brew --prefix rustup)/bin:$PATH"
pushd demos/warped_mvp_rust
make run-warped                     # builds the match server and the five levels, then opens the app
popd
```

## The Mac app

```bash
make -C demos/warped_mvp_rust package-mac-app   # .build/Warped/Warped.app and .build/Warped.zip
```

`make package-mac-app` (`tools/package_mac_app.sh`) builds `Warped.app`:

| In the app | What |
| --- | --- |
| `Contents/MacOS/Warped` | the one-window app (`warped`) |
| `Contents/MacOS/warped-match` | the match server, the bots, and Lunar Surface (`../warped_match_rust`) |
| `Contents/MacOS/levels/<level>` | every other level of the manifest, by its name (`dust`, `snow_planet`, ...) |
| `Contents/Resources/match_levels.yaml` | the levels manifest, with each level's map picture beside it |
| `Contents/Resources/assets`, `models` | the guns' and maps' models and textures |
| `Contents/Resources/AppIcon.icns`, `Contents/Info.plist` | the match's icon and Info.plist (`../warped_match_rust/app/`) |

The levels are built one after another into one shared target folder (`.build/levels_target/`), so
their common crates build once. The app is signed ad hoc (not by Apple).

**Install** (also in the zip's `How to open Warped.txt`):

1. Unzip, and drag `Warped.app` into Applications.
2. Double-click it. The app isn't signed by Apple, so the first time macOS refuses: right-click
   (Control-click) `Warped.app`, choose Open, then Open again; or System Settings, Privacy &
   Security, Open Anyway; or in Terminal `xattr -dr com.apple.quarantine /Applications/Warped.app`.
3. Allow Local Network access when asked, so games on the Wi-Fi can be found.

**Limitation**: the levels still read their maps, textures and settings from the checkout they were
built in (`CARGO_MANIFEST_DIR`), so the app plays its levels on the Mac that built it (and on a Mac
with the same checkout at the same path). Running elsewhere needs each level to look its files up
through `warped_shared::app_bundle` first; the levels-as-libraries and level-definition work
(W-143) is the place for it. The menus, joining, hosting and the match server work anywhere.

## Screens

- **Main menu**: the games on this network (each match server's beacon, UDP 47851) to join; joining
  a game by its address; the game to start, the five-level match or any one level of the levels
  manifest (Left/Right on "Game"), hosted for others, played alone, or with bots (on your team, 0
  to 4, and on the other, 0 to 5, at most 7 together since a level takes 8 players; turning off
  consoles or fighting; you play blue); while this app hosts a game, going back to it or
  stopping it; player settings; credits; quit.
- **Game menu**: the match's status (the progression page shared with the match window: the level
  being played and its map picture, the wavefronts, the round, who attacks and defends, free play's
  wins, players per team); your team and switching it; rejoining the round being played; the game
  master's choice of what everyone plays (the match, or any one level: Left/Right on its row, or
  click a level on the match's status, and "back to the match" there; no authentication yet, so
  anyone may set it); game settings; player settings; leaving to the main menu.
- **Game settings** (the game master's, W-158, per-map game settings), from the game menu: the match
  server's settings in sections, every level's (a key at least two levels' settings files have,
  "(levels differ)" where their defaults do) and each level's own (Lunar Surface's gravity, City
  Sphere's glass bounces, range and reflectivity, Dust's wind, Snow Planet's blizzard size; Dark
  Forest's section is empty). Left/Right steps a number, typing sets one, R goes back to the
  default; each change goes to the match server at once, and each round's level plays by the
  settings as they are when it starts. `*` marks a value off its default.
- **Level**: a round's level, played in this window by the level's own library
  (`src/level_library.rs`: Lunar Surface, Dust, Snow Planet, City Sphere, Dark Forest, found by
  the manifest's `folder`), joined to the round's level server on your team with your settings
  file. Escape opens the level's menu (its MATCH page shows the match's status; its QUIT page
  leaves for the game menu); holding Tab shows the map. When the round ends the level returns to
  the game menu, which shows the match's progress; leaving keeps you out of that round until
  Rejoin; closing the window quits. A level without a library would show a stand-in screen.
- **`warped --tour-levels`** checks the levels inside the app: it plays alone, asks for each level
  in turn, and in each one settles, times its frames, holds Tab, opens the menu and leaves; it
  writes `frame_times.md` and the game menu after each level to `.artifacts/tour_levels/<run_id>/`.
  Run it with `WARPED_LOOPBACK_ONLY=1` and your own ports (`--match-port`, `--level-port`,
  `--discovery-port`).
- **Player settings**, from every menu: every key binding and the in-level menu's numbers, saved to
  `~/.config/warped/player_settings.yaml`, the file every level reads (`--player-settings` for
  another file).

Escape never quits: in a level it opens the in-level menu, in the in-level menu it resumes, in the
game menu it goes to the main menu (the user's rule), and on the main menu it does nothing. A game
the app hosts keeps running on the main menu until "Stop hosting" or quitting.

## Flags

| Flag | Default | What |
| --- | --- | --- |
| `--width-px`, `--height-px` | 1280, 800 | the window's starting size in points (the screen pictures' in pixels) |
| `--player-settings <path>` | `~/.config/warped/player_settings.yaml` | the player's settings file |
| `--match-port`, `--level-port` | 47850, 47852 | UDP ports of a game this app hosts |
| `--discovery-port` | 47851 | UDP port it listens on for match beacons |
| `--save-screen-pictures` | | draw every screen without a window, and exit |
| `--label <label>` | | a suffix for the run folder's name |

## How it fits together

- **Levels manifest**: the app reads the levels from one file, today
  `../warped_match_rust/match_levels.yaml`, through `src/levels_manifest.rs` alone, so it never
  names a level; the declarative level definitions (W-143, `common/maps/LEVELS.md`) replace it there.
- **The level interface** (`warped_shared::app`): a level crate's `lib.rs` exposes one
  `play_level_fn_t`; the app calls it with a `LevelWindow` (SDL, the window, its event pump, its
  `CAMetalLayer` and Metal device, and a `MatchLink`) and a `LevelJoin` (the manifest entry, the
  level server's address, the team, the player's settings file). It returns a `LevelEnd`: the
  round is over, `LeaveLevel` or `CloseWindow` (the in-level menu's `PlayOutcome`). Each frame the
  level calls `match_link.is_round_over()`, where the app keeps the match client alive, and feeds
  `match_link.get_match_lines()` to its menu's MATCH page.
- **Children** (`src/children.rs`): hosting runs `hyperbolic-wormholes --serve-match` from
  `../warped_match_rust/.build/release/` (built by `make build-children`); each bot is a match client
  with `--bot`. Playing alone adds one bot on the other team that sits out every round, since a
  round starts only when each team has a player.
- **The game master's choice**: the game menu sends the match server `play <level>` or
  `play match`; the server sets the match aside for free play on that level (round after round,
  counting each team's wins) or goes back to it, ends the round under way, and every client follows
  into the next round (`warped_shared::match_protocol`, `../warped_match_rust/src/match_flow.rs`).
- **Drawing**: menus are pages of shapes in SDL's 8x8 debug font (the match window's look),
  drawn on the CPU and copied into the window's Metal layer only when they change.

## Tests

`make test` covers the screens' navigation (every Escape and exit rule), the main menu (every
manifest level can be chosen, each game kind and its bots), the game menu (teams, rejoining, the
game master choosing every level, the in-level menu), the game master's settings page (its
sections, every change and reset sent, a refused fraction, following the server's answer; and against a
stand-in server on UDP 48392 and 48393), the app following a match server's views into
and out of a round's level and sending the game master's choice until it is taken (against a stand-in
server on UDP 48304 to 48307), the children's command lines, the player settings page (saving only
changes, every setting has a row) and the menu keys. The shared crate tests the beacons (bytes,
expiry, a listener hearing an announcer on UDP 48301), the play messages and free play; the match
crate tests that a match server switches levels on the game master's word and everyone follows (a
fake level program, UDP 48302 and 48303).

Tests listen for beacons on 127.0.0.1 only. `warped --loopback-only` (or `WARPED_LOOPBACK_ONLY=1`) keeps the app,
the match server it hosts and every level and bot on this computer, so testing it never makes macOS ask for network
permission; without it the app finds and hosts games on the local network.

## Folders

`cargo` writes to `.build/` (`make clean`). `.artifacts/warped/<run_id>/` holds the logs of the
games this app hosts (`match_server.log`, `bot_<n>_<team>.log`; in `Warped.app`,
`~/Library/Logs/Warped/<run_id>/`), and `.artifacts/screen_pictures/<run_id>/` the screen pictures;
`make reset-artifacts YES_TO_RESET_ARTIFACTS=1` deletes them.

Decisions are in [`DECISIONS.md`](DECISIONS.md).
