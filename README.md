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
```

Components may use different languages, so `common/` holds specifications, not shared code.
