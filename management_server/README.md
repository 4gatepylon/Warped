# Management server

The management server lets players create, list, join and shut down rooms. It starts and stops
the game server that runs each room and tells clients how to reach it.

What goes here:

- The rooms service that clients call (`common/protocol/`).
- The records of rooms and the game servers that run them.
- Starting, stopping and checking on game servers.
