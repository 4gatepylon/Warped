# Clients

Each subfolder is the game client for one platform, in that platform's own languages and
graphics API. macOS (`mac/`) is the first platform supported.

What goes in each client:

- Rendering the curved space and reading the player's input.
- Finding, creating and joining rooms through the management server.
- The real-time connection to the room's game server (`common/protocol/`).
- Whatever parts of `common/physics/` the client needs, implemented in its own language: for
  example, to show the player's own movement before the game server confirms it.

A client knows only what the game server sends it, so it never holds the room's full state.
