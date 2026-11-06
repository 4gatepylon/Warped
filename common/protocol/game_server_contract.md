# Game server contract

What every game server implementation must do, so launchers and game servers can change
independently:

- **Inputs:** it takes these command-line flags:
  - `--game-server-id`: its UUID.
  - `--config`: its `GameServerConfig`, as JSON.
  - `--max-player-count`: how many players it may have at once.
  - `--management-server-url`: where to check players, e.g. `http://10.0.0.5:8000`.
  - `--port`: the port to listen on, on every network interface (`0.0.0.0`), so other machines
    can connect.
- **Health:** once it accepts players, it answers HTTP `GET /health` on TCP port `--port` with
  `200`, so a launcher can tell when it is running.
- **Admission:** when a client connects with a player ID, it lets the client in only if
  `GET /game-servers/{game_server_id}/players/{player_id}` on the management server finds that
  player.
- **Stopping:** it exits within 5 seconds of `SIGTERM`.
