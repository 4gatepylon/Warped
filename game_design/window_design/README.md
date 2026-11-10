The pictures of [`../WINDOW_DESIGN.md`](../WINDOW_DESIGN.md).

The mock-up of the windows that aren't built yet lives in the prototype,
[`demos/system_design_visualization_and_prototyping/`](../../demos/system_design_visualization_and_prototyping/README.md),
where its windows are wired to a simulated backend one part at a time: the game app's windows at `/game/` and the
website's pages at `/site/`.

```bash
make -C demos/system_design_visualization_and_prototyping run-prototype
```

```bash
open http://127.0.0.1:8010/game/
```

Screenshots of the mock-ups go in `game_design/.artifacts/window_mockups/<run_id>/`, which git ignores.
