# Starter client

A bare-bones client for the starter square game (`common/protocol/square_game.md`), in Rust with
SDL3. It is not the final client. `make help` lists its commands; the first build compiles SDL3
from source, which takes about a minute.

- **Lobby:** N creates a game, 1-9 join one, Esc quits.
- **Game:** WASD or the arrow keys move, a left click shoots toward the mouse (once every 2 s),
  and Esc goes back to the lobby. Joining again makes a new player.

## Known issue: a wrong URL freezes the window

If `--management-server-url` points at a host that doesn't answer, such as a wrong IP or a machine
on another network, the window stops responding and has to be force quit (⌘⌥Esc, or
`pkill warped-starter-client`). The lobby asks the management server for its list on the
window's own thread once a second, and each request can wait 30 s to connect (ureq's default), so
the window never gets to handle Esc or closing. A management server that isn't running fails at
once instead, with a message in the lobby. Check the URL the management server printed when it
started.
