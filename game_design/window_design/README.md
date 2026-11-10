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
