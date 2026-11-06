# Game server

A game server runs one room in real time and is the authority on its state. The management server
starts and stops it.

What goes here:

- The room's full state, including what some players must not see.
- The tick loop: take the players' inputs, check them, and advance the state with this server's
  implementation of `common/physics/`.
- Deciding what each client may know, and sending each client only that part of the state.
- The real-time connection to the clients, and the control service the management server calls
  (`common/protocol/`).
