The pictures of [`../WINDOW_DESIGN.md`](../WINDOW_DESIGN.md): a mock-up of the windows that aren't built yet, as a
small website to click through and comment on. Nothing in it is real: the players, games and numbers are made up,
and it talks to no server.

```bash
open game_design/window_design/index.html
```

- `index.html`: the mocked windows, one `section` each.
- `mockup.css`: their look, after the game's own menus.
- `mockup.js`: moving between windows, the role and account switches, a pretend search for a match, and the
  feedback bar.

The bar at the bottom keeps a like or dislike and a note for each window, in the browser only. "All feedback" lists
them to copy into a chat. Screenshots of the mock-ups go in `game_design/.artifacts/window_mockups/<run_id>/`, which
git ignores.

## Changing it

Everything is plain HTML and CSS: no build step, no packages. Edit a file and reload the page.

- **Change a window:** find its `<section class="window" id="w-<name>">` in `index.html` and edit the HTML.
- **Add a window:** add a `section` with a new `id="w-<name>"` and a `data-title`. The toolbar lists it by itself.
  Add `data-side` to show the friends panel beside it, and `data-online` if it needs the purchase.
- **Link windows:** `data-go="<name>"` on any element opens that window when clicked.
- **Add tabs:** wrap them in an element with `data-tabs="<first pane>"`, with `data-tab="<pane>"` buttons and
  `data-pane="<pane>"` blocks inside.
- **Show something to some viewers only:** `data-needs="admin"`, `root`, `paid`, `unpaid`, `custom` or `matchmade`
  (several words mean all of them).
- **Add a window over the others:** a `<div class="modal" id="m-<name>">`, opened with `data-open="<name>"` and
  closed with `data-close`.
- **Add behaviour:** add a function to `ACTIONS` in `mockup.js` and call it with `data-act="<its name>"`.
- **Change the look:** the colours are variables at the top of `mockup.css`.

Keep the made-up players made up, and keep the design in `../WINDOW_DESIGN.md` in step with what the mock-up shows.
