# Physics

The physics is specified once, here, and implemented separately by the game server and by each
client that needs it, in their own languages.

What goes here:

- **Specification:** the spaces a room can take place in (topologies and geometries, such as
  spherical or hyperbolic space), how objects move through them, and how objects such as black
  holes curve them; the units, coordinates and time step; and the tolerance within which two
  implementations count as agreeing.
- **Test cases:** language-neutral files, each a starting state, inputs and the expected state
  after a number of steps, that every implementation runs in its own tests.

Implementations don't go here: they live in `game_server/` and `clients/<platform>/`.
