# Common

What clients and servers must agree on, specified once: the messages (`protocol/`), the physics
(`physics/`) and the maps (`maps/`: the levels' layouts in one portable format, with a validator). These are specifications, not shared code: components may use different languages,
and a client never holds the server's full state.
