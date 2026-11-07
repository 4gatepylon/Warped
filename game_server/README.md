# Game server

Runs one room in real time and is the authority on its state, including what some players must not
see. Each tick it applies the players' inputs with its own implementation of `common/physics/`,
then sends each client only what that client may know.

For now it runs the starter square game (`common/protocol/square_game.md`). A Python (uv) project:
`make help` lists its commands, and `uv run game-server --help` shows how to run it, which the
management server's launcher does.
