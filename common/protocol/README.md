# Protocol

The protocol defines every message exchanged between the clients, the management server and the
game servers. Messages are Protocol Buffers, written once here as `.proto` files, and each
component generates code for them in its own language.

What goes here, by channel:

- **Rooms** (client ↔ management server): gRPC services to create, list, join and shut down rooms.
- **Game server control** (management server ↔ game server): gRPC services to start, stop and
  check on the game server that runs a room.
- **Real-time play** (client ↔ game server): player inputs and state updates. These are protobuf
  messages too; how they are carried is chosen separately from the gRPC services above.

Generated code doesn't go here: each component generates its own into its git-ignored `.build/`.
Physical quantities in messages (positions, velocities, times) use the units and coordinates that
`common/physics/` specifies.
