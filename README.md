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
notes/              # notes to keep for future reference: knowledge, thoughts, background reading
```

Components may use different languages, so `common/` holds specifications, not shared code.

## Play the starter game on your Wi-Fi

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