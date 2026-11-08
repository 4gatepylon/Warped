# Protocol

The messages between the game window (`warped`) and the game server (`warped-server`), specified once.

- [`authentication.md`](authentication.md): how a server lets in only the players who know its password. It's
  decided, so the demo, the game, the Mac app and the cloud deployment all build the same bytes.
- **Every other message:** single UDP datagrams in little-endian layouts, written and read by
  `demos/hyperbolic_wormholes_rust/src/net.rs`, whose docstrings are the specification until the game's copy is
  written up here.
- **Plan:** [`game/REFACTOR_PLAN.md`](../../game/REFACTOR_PLAN.md) moves that code into `game/core/`. Its phase C
  writes the layouts up in this folder (`messages.md`), then adds a smaller pose and loss-proof eliminations and hits.
