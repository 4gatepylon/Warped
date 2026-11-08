# Protocol

The messages between the game window (`warped`) and the game server (`warped-server`), specified once.

- **Today:** single UDP datagrams in little-endian layouts, written and read by
  `demos/hyperbolic_wormholes_rust/src/net.rs`, whose docstrings are the specification until the game's copy is
  written up here.
- **Plan:** [`game/REFACTOR_PLAN.md`](../../game/REFACTOR_PLAN.md) moves that code into `game/core/` and, in its
  phase C, adds a protocol version, a session token, a smaller pose and loss-proof eliminations and hits. Phase C's
  first layer writes the layouts up in this folder.
