# Warped

A multiplayer first-person shooter set in curved space: rooms can be spherical or hyperbolic, and
players can place objects such as black holes that bend the space around them.

## Layout

```
game/               # the Warped game: the window (`warped`), the game server (`warped-server`), shared code
game_design/        # the game's design: levels, rules, weapons
common/maps/        # the levels' layouts in one portable format, with a validator
common/protocol/    # the messages between the game window and the game server
common/physics/     # the physics spec and the test cases every implementation passes
demos/              # experiments that de-risk mechanics, weapons and physics before we build them
infra/              # the Rust demo's cloud deploy on Google Cloud (Terraform) and its download page
notes/              # notes to keep for future reference: knowledge, thoughts, background reading
```

`common/` holds specifications and data, not shared code.

## Play

The game is moving from [`demos/hyperbolic_wormholes_rust/`](demos/hyperbolic_wormholes_rust/README.md) into
[`game/`](game/REFACTOR_PLAN.md). Until the move is done, play the demo, as its README says:

```bash
make -C demos/hyperbolic_wormholes_rust serve-game
make -C demos/hyperbolic_wormholes_rust join-game SERVER=127.0.0.1
```

## Rough checklist to full game
1. [ ] Get working minecraft-like visuals game on a single level with good physics without multiplayer. This will involve defining one actual level with real obstacles, etc...
2. [ ] Get working minecraft-like visuals game on all levels for the map without multiplayer. This will involve actually finalizing the design for _all_ the levels on the map.
3. [ ] Get (2) working with multiplayer.
4. [ ] Get (3) working with proper (realistic, ideally AAA-looking) graphics.
5. [ ] Make sure the CRUD systems work correctly to enable players to spin up game servers. Integrate with AWS (if needed).
6. [ ] Set up website (etc...) so anyone can easily download and join. Make sure it works on a few different systems.