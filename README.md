# Warped

A multiplayer first-person shooter set in curved space: rooms can be spherical or hyperbolic, and
players can place objects such as black holes that bend the space around them.

## Layout

```
clients/mac/        # the game client, one folder per platform (macOS first)
management_server/  # starts and stops game servers, and tracks who may join each
game_server/        # runs one room in real time; the authority on its state
common/protocol/    # every message and API between components
common/physics/     # the physics spec and the test cases every implementation passes
demos/              # experiments that de-risk mechanics, weapons and physics before we build them
infra/              # the Rust demo's cloud deploy on Google Cloud (Terraform) and its download page
notes/              # notes to keep for future reference: knowledge, thoughts, background reading
```

Components may use different languages, so `common/` holds specifications, not shared code.

## The official game

> NOTE: If this section is outdated (the official game moved, a folder below was replaced, or a command
> no longer works), ask the user to help you update it, so that the next agent isn't misled.

The official game so far is [`demos/warped_mvp_rust/`](demos/warped_mvp_rust/README.md): the whole game in
one window, with a main menu (games on the network, hosting, playing alone or against bots, the five-level
match or any one level), a game menu, and each round's level. It lives under `demos/` for historical
reasons; it is the game, not an experiment.

To play it, on a Mac with Rust installed through Homebrew:

```bash
export PATH="$(brew --prefix rustup)/bin:$PATH"
make -C demos/warped_mvp_rust run-warped
```

The first run builds the match server and the five levels, which takes a while, then opens the game. To
play with others, host a game from the main menu or pick one on your Wi-Fi. `make -C demos/warped_mvp_rust
package-mac-app` builds `Warped.app`; the folder's README has the rest.

### The other folders

| Folder | Status | What it is |
| --- | --- | --- |
| `demos/warped_mvp_rust/` | official | the one-window game |
| `demos/warped_match_rust/`, `demos/warped_shared_rust/` | part of the official game | the match server and bots, and the library the levels share |
| `demos/hyperbolic_wormholes_rust/` (Lunar Surface), `demos/dust_rust/`, `demos/snow_planet_rust/`, `demos/city_sphere_rust/`, `demos/dark_forest_rust/` | part of the official game | the five levels it plays; each also runs alone as a demo |
| `infra/`, `common/maps/` | supports the official game | the Rust game's cloud deploy and download page, and the portable map format its levels' layouts follow |
| `clients/starter/`, `game_server/`, `management_server/`, `common/protocol/` | historical, to be replaced soon | the starter 2D square game, below, and the specs behind it |
| `common/physics/`, `clients/mac/` | planned, unused so far | folders that hold only a README (a physics spec and a macOS client plan); nothing in the official game refers to them yet |
| `demos/levels/`, `demos/fps_demo/`, `demos/hyperbolic_wormholes/`, `demos/three_js_vibecode_demo/`, `demos/warp_pull_js/`, `demos/laser_gravity_demo/`, `demos/gluebox/`, `demos/mirror_sphere/`, `demos/mirror_sphere_v2/` | historical | earlier experiments that decided what to build ([`demos/README.md`](demos/README.md)) |

## Play the old starter game on your Wi-Fi (historical)

On one Mac, start the servers; the first line it prints is the management server's URL:

```bash
make -C management_server run-management-server
```

On each player's Mac, with Rust installed (or with a copy of the built client):

```bash
make -C clients/starter run-client MANAGEMENT_SERVER_URL=http://<that URL's IP>:8000
```

## Rough checklist to full game
1. [ ] Get working minecraft-like visuals game on a single level with good physics without multiplayer. This will involve defining one actual level with real obstacles, etc...
2. [ ] Get working minecraft-like visuals game on all levels for the map without multiplayer. This will involve actually finalizing the design for _all_ the levels on the map.
3. [ ] Get (2) working with multiplayer.
4. [ ] Get (3) working with proper (realistic, ideally AAA-looking) graphics.
5. [ ] Make sure the CRUD systems work correctly to enable players to spin up game servers. Integrate with AWS (if needed).
6. [ ] Set up website (etc...) so anyone can easily download and join. Make sure it works on a few different systems.