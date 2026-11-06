# Warped

Warped is a multiplayer first-person shooter whose main mechanic is curved space. Matches take
place in spaces with different topologies and geometries, such as spherical or hyperbolic space,
and players can place objects such as black holes that curve the space around them.

## Layout

```
clients/              # one game client per platform
└── mac/              # macOS on Apple silicon (M1 and later)
management_server/    # creates, lists, joins and shuts down rooms, and starts their game servers
game_server/          # runs one room in real time and streams its state to the clients
common/               # code shared by the clients and the servers
├── simulation/       # the game's state machine: a state and inputs in, the next state out
└── protocol/         # the messages that clients and servers exchange
```

Each component picks its own languages and tools. For example, the macOS client may use Metal
Shading Language (MSL) on the GPU, while a client for another platform would use that platform's
graphics API.
