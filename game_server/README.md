# Game server

Runs one room in real time and is the authority on its state, including what some players must not
see. Each tick it applies the players' inputs with its own implementation of `common/physics/`,
then sends each client only what that client may know.
