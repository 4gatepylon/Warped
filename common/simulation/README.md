# Simulation

The simulation is the game's state machine: given a room's state and the players' inputs, it
computes the next state, including movement through curved space and the effect of objects such
as black holes. The game server runs it to decide what happens, and clients run it to predict
what happens before the server's answer arrives.
