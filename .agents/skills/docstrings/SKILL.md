---
name: docstrings
description: Write docstrings in this repo's templates - Python (Google style), C++ and Metal Shading Language (Doxygen's Javadoc style), and the `#` header block of Makefiles, shell scripts, TOML and .env files. Use whenever you add or change a file, class or function, or write or edit any docstring or file header.
---

# Docstrings

Every file, class and function has one docstring (see "Comments" in the root `AGENTS.md`). This
skill says which sections it has and which template it follows.

- **Sections:** a docstring's sections come in the template's order. A section with nothing to say
  is left out, except Example, which every docstring has except a test's.
- **Interface:** a function's interface is its arguments, return value and errors. A file's is its
  command-line arguments (scripts and `main`s) or its public names (libraries). A Python class's is
  its constructor arguments and attributes; a C++ class documents its interface on its public
  methods and static constructors.
- **Must know:** an optional list grouped under these categories, in this order: Limitations,
  Gotchas, Side effects, Performance. Each item is one bullet of one sentence that fits on one
  line. A new category needs the user's agreement.
- **Files without docstrings:** Makefiles, shell scripts, TOML and `.env` files start with a `#`
  block shaped like the Python file docstring.

Python docstrings use the Google style:

```python
"""`serve.py` runs the management server until it gets SIGINT or SIGTERM.

Interface:
    --port: The management server's HTTP port (default 8000).
    --advertised-host: The host clients reach game servers at (default 127.0.0.1).

Example:
    $ uv run management-server --advertised-host 10.0.0.5

Must know:
    Side effects:
        - When it exits, `serve.py` stops every game server it started.
"""


class Room:
    """`Room` holds the players and bullets of one game server's map.

    Args:
        max_player_count: How many players the `Room` holds at once.

    Attributes:
        player_count: Number of players in the `Room` now.

    Example:
        >>> room = Room.FromConfig(config)

    Must know:
        Limitations:
            - A `Room` is not thread-safe.
    """


def step_bullet(bullet: Bullet, black_holes: list[BlackHole], dt: float) -> Bullet:
    """`step_bullet` moves a bullet one time step along its path, bent by black holes.

    Args:
        bullet: The bullet to move.
        black_holes: The black holes that pull on the bullet.
        dt: The time step, in seconds.

    Returns:
        A new bullet, `dt` seconds later.

    Raises:
        ValueError: `dt` isn't positive.

    Example:
        >>> bullet = step_bullet(bullet, black_holes, 1 / 60)

    Must know:
        Limitations:
            - `step_bullet` loses accuracy when a bullet passes very close to a black hole.
    """
```

C++ docstrings (and those in Metal Shading Language, which is based on C++) use Doxygen's Javadoc
style:

```cpp
/**
 * @file
 * @brief `replay.cc` replays a recorded match and prints one CSV line per tick.
 *
 * @par Interface
 * - Flags: `--recording-file <path>`, `--tick-count <count>`.
 * - Output: one `tick,player_count` line per tick on stdout.
 *
 * @par Example
 * @code
 * .build/replay --recording-file match.bin --tick-count 600
 * @endcode
 *
 * @par Must know
 * - Gotchas:
 *   - `replay` stops early when the recording has fewer than `--tick-count` ticks.
 */

/**
 * @brief `Room` holds the players and bullets of one game server's map.
 *
 * @par Example
 * @code
 * Room room = Room::FromConfig(config);
 * @endcode
 *
 * @par Must know
 * - Limitations:
 *   - A `Room` is not thread-safe.
 */

/**
 * @brief `step_bullets` moves N bullets one time step along their paths, bent by black holes.
 *
 * @param[in,out] bullets The bullets to move.
 * @param[in] black_holes The black holes that pull on the bullets.
 * @param[in] dt The time step, in seconds.
 * @param[in] N Number of bullets.
 *
 * @par Example
 * @code
 * step_bullets(bullets, black_holes, dt, N);
 * @endcode
 *
 * @par Must know
 * - Performance:
 *   - The first call builds a table of the black holes' pull, so it is slower than later calls.
 */
```

A C++ function that returns a value adds `@return <meaning>.` after its `@param` lines, and one that
throws adds `@throws <type> <when>.`.
