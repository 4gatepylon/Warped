# Starter client

A bare-bones client for the starter square game (`common/protocol/square_game.md`), in Rust with
SDL3. It is not the final client. `make help` lists its commands; the first build compiles SDL3
from source, which takes about a minute.

- **Lobby:** N creates a game, 1-9 join one, Esc quits.
- **Game:** WASD or the arrow keys move, a left click shoots toward the mouse (once every 2 s),
  and Esc goes back to the lobby. Joining again makes a new player.
