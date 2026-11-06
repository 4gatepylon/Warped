# Game server

A game server runs one room in real time. Each tick, it takes the players' inputs, steps the
shared simulation (`common/simulation/`), and sends the resulting state changes to every client in
the room. It is the authority on the room's state.
