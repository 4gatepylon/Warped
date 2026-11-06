# Warped

Warped is a multiplayer first-person shooter whose main mechanic is curved space. Matches take
place in spaces with different topologies and geometries, such as spherical or hyperbolic space,
and players can place objects such as black holes that curve the space around them.

## Layout

```
clients/              # one game client per platform
└── mac/              # macOS on Apple silicon (M1 and later)
management_server/    # creates, lists, joins and shuts down rooms, and starts their game servers
game_server/          # runs one room in real time; the authority on its state
common/               # what the clients and servers must agree on, specified once
├── protocol/         # every message they exchange, as Protocol Buffers
└── physics/          # the physics of curved space, and test cases every implementation passes
```

Each component picks its own languages and tools. For example, the macOS client may use Metal
Shading Language (MSL) on the GPU, while a client for another platform would use that platform's
graphics API. So `common/` holds specifications, not shared code, and each folder's README says
what goes in it.
