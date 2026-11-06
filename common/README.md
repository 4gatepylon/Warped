# Common

`common/` specifies, in one place, what the clients and the servers must agree on: the messages
they exchange (`protocol/`) and the physics of the game (`physics/`).

It holds specifications, not shared code. The components may be written in different languages,
and they don't run the same state machine: a game server holds a room's full state, including
what some players must not see, while a client knows only what the game server sends it. Each
component implements these specifications in its own folder and language.
