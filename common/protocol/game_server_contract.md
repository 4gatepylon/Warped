# Game server contract

What every game server implementation must do, so launchers and game servers can change
independently:

- **Inputs:** when started, it is given its game server ID, configuration, maximum number of
  players, the management server's URL and the port to listen on.
- **Health:** it answers health checks on that port, so a launcher can tell when it is running.
- **Admission:** when a client connects with a player ID, it lets the client in only if
  `GET /game-servers/{game_server_id}/players/{player_id}` on the management server finds that
  player.
- **Stopping:** it exits when its launcher stops it.
