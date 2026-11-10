# System design visualization and prototyping

A playable simulation of the system around a Warped match, to try the design of
[`game_design/OUTSIDE_GAME_DESIGN.md`](../../game_design/OUTSIDE_GAME_DESIGN.md) and
[`game_design/WINDOW_DESIGN.md`](../../game_design/WINDOW_DESIGN.md) before building it for real, and to watch how
the backend of [`game_design/cloud/`](../../game_design/cloud/README.md) would behave. It runs on one computer, with
no cloud account. Nothing in it is the real game, and every player is made up.

```bash
make -C demos/system_design_visualization_and_prototyping run-prototype
```

```bash
open http://127.0.0.1:8010/
```

## What is in it

| Page | What it is |
| --- | --- |
| `/game/` | The game app mock-up: the windows of the Mac app, to click through. |
| `/site/` | The website mock-up: download, rankings, status and the superadmin console. |
| `/system/` | The system view: components, the messages between them, machines, game servers, the database. |
| `/docs` | The backend's API with its schemas, drawn by FastAPI. |

The yellow band at the top of every page is the **simulation controls**. The real game will not have them: they
skip the clock, set how many simulated players are online, and (in the system view) kill machines and game servers.

## What is simulated so far

Each component of the real design is played by a part of `src/system_prototype/world.py`, and every call from one
component to another is written to the `message` table, which the system view draws.

| Component | In the prototype | In the real design |
| --- | --- | --- |
| Backend | admits matches under the caps, keeps the wait queue, takes results | the backend service |
| Database | SQLite, one file per run | PostgreSQL |
| Fleet manager | places game servers on machines, keeps spare slots, turns empty machines off | the same |
| Cloud | boots pretend machines after a delay | a cloud provider |
| Machines and game servers | rows with a state; a game server's match is a pretend one that lasts a while | real machines running the game server |
| Simulated players | come online up to a target and ask for matches in tens | real players |

Still to come, one part at a time: accounts with sign-in and payment, a stand-in 2D game with recordings and
spectating, custom games, matchmaking, rating, fair play, friends and chat.

## Layout

- `src/system_prototype/`: `world.py` (the simulated system), `rules.py` (the rules as plain functions), `store.py`
  (the tables), `clock.py` (the clock that can skip), `http_api.py` (the routes), `serve.py` (the command).
- `web/`: the pages. `shared/` has the look, the window navigation, the API helper and the simulation controls.
- `tests/`: unit tests, with the input partitionings they cover in each file's docstring.

`make help` lists the commands. Each run of `make run-prototype` writes `.artifacts/serve/<run_id>/`, with its
`command.txt` and its database, a new world each time; `uv run system-prototype --database-file <path>` carries on
from a kept one. Screenshots of a flow go in `.artifacts/flow_screenshots/<run_id>/`.

## Changing the pages

The pages are plain HTML, CSS and JavaScript with no build step: edit a file and reload.

- **Change a window:** find its `<section class="window" id="w-<name>">` in `web/game/index.html` or
  `web/site/index.html` and edit the HTML.
- **Add a window:** add a `section` with a new `id="w-<name>"` and a `data-title`. The toolbar lists it by itself.
  Add `data-side` to show the friends panel beside it, and `data-online` if it needs the purchase.
- **Link windows:** `data-go="<name>"` on any element opens that window when clicked.
- **Add tabs:** wrap them in an element with `data-tabs="<first pane>"`, with `data-tab="<pane>"` buttons and
  `data-pane="<pane>"` blocks inside.
- **Show something to some viewers only:** `data-needs="admin"`, `root`, `paid`, `unpaid`, `custom` or `matchmade`
  (several words mean all of them).
- **Add a window over the others:** a `<div class="modal" id="m-<name>">`, opened with `data-open="<name>"` and
  closed with `data-close`.
- **Make a window live:** call the backend with `call_api` (`web/shared/api.js`) from the page's own script, as
  `web/site/site.js` does for the status page.
- **Change the look:** the colours are variables at the top of `web/shared/look.css`.
