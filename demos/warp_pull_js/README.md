# Wormhole pull and sprint tuning demo

A page to tune the pull toward wormhole mouths and the sprint before the levels get them (W-202, pull toward wormholes
and black holes in the game settings). A player seen from above moves between one wormhole's two mouths under the
proposed equations ([EQUATIONS.md](EQUATIONS.md)), with a slider for every setting.

- **Run it:** `make serve-demo`, then open <http://127.0.0.1:48510/>. WASD or the arrows move, Shift sprints, R puts
  the player back.
- **What it saves:** `serve_demo.py` writes to `.artifacts/serve_demo/<run_id>/`: every settings change
  (`settings_latest.json`, `settings_log.jsonl`), each run sent with "Save this run for Claude" (`recording_<n>.json`:
  the settings, the starting state, every key change by tick, and the path), and the settings sent with "It's good now"
  (`approved.json`, with the run so far).
- **Why recordings:** the dynamics step 120 times a second and depend only on the keys, so replaying a recording's
  keys through the levels' Rust code must give the recording's path. That is the check that the levels have the same
  dynamics as the page.
  `node replay_recording.mjs <recording.json>` replays one through `pull_dynamics.js` and prints how far the replay
  strays from the recorded path (0 m for a sound recording).
- **Files:** `pull_dynamics.js` (the dynamics, no page needed), `index.html` (the page), `serve_demo.py` (the server),
  `replay_recording.mjs`, `test_pull_dynamics.mjs` (`make test`).
