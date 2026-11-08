# Decisions (warped match)

Decisions made instead of the user while building the match (W-123). **Weigh in** marks the ones
the user should check.

- **Each level is its own program, started per round** (rather than one program with every level
  in it). The levels are being built in parallel on separate branches; a process boundary with a
  tiny interface (README, "Level interface") lets each be plugged in unchanged and replaced
  later. The match window and the level window are separate windows. **Weigh in**: for the full
  game the level will likely be one program with the match page as a menu; this boundary is where
  to cut.
- **A level reports its winner in a file** (`--level-result-file`, written once, atomically:
  `.partial` then rename) rather than over the network: nothing to parse in the level's protocol,
  and a crashed level leaves no half-written result.
- **The match server is authoritative**; clients only say `hello <team>` every 0.5 s and draw the
  state it sends (JSON over UDP, port 47850). A client heard from in the last 3 s counts as on its
  team. 47851 is reserved for match discovery but unused: join by address.
- **A round starts when each team has at least one player**, 10 s after (intermission), and the
  page stays on the result 5 s after a round (round over). Both are flags
  (`--intermission-s`, `--round-over-s`).
- **The match timer runs from the first round's start**, 30 minutes (`--match-s`), and the match
  is a tie when it runs out, even mid-round (DESIGN.md: "If the match times out, it's a tie").
- **Wavefronts never move back**: when the defender wins, roles swap and the new attacker plays
  at its own wavefront, where it was. This is how the worked example in DESIGN.md reads (rounds 3
  to 5); `src/match_state.rs` tests that table row by row.
- **Ownership on the page**: defensive ownership of a level is its side's team (purple has none);
  offensive ownership is the team whose wavefront it is, or that captured it (won it attacking).
  **Weigh in** if "ownership" was meant differently (e.g. a captured level changing hands).
- **Stand-ins**: Lunar Surface (from the copied hyperbolic demo) can play a level not plugged in
  (`stand_in_for` in `match_levels.yaml`); the page marks it `(stand-in)`. All four levels are
  plugged in now, so none is used. The house map has no consoles, so it can't be won and isn't
  used.
- **Levels' flags differ a little**: Dark Forest's bots take no `--label`, so a level says so in
  `match_levels.yaml` (`takes_bot_label: false`) rather than every level having to accept every
  flag. Dark Forest takes no `--map` name either (its `--map` is a map file), so its entry has no
  `map`. The fast bot match shortens consoles to 1 s through each level's `--server-settings`
  where it has one (Dark Forest has none, so it keeps its own time).
- **Plugging in a level links its branch** (`make levels`: a git worktree of the level's branch
  in `.levels/`, built there) rather than copying its folder, so the match always plays the
  level's latest pushed version and this branch stays small. `match_levels.yaml` names each
  level's binary.
- **Console bots** (`--bot-objective consoles`) hold `E` on the other team's consoles in turn
  and don't shoot while doing so; they don't walk to the console, because the copied server
  doesn't check distance. Who wins a bot round is a race, which exercises both outcomes.
- **Fix in the copied level code**: when both teams' last consoles went off in the same server
  update, nobody won and the round never ended (seen in the end-to-end match). Now the first one
  turned off wins. The same bug is in `demos/hyperbolic_wormholes_rust/` (not changed here).
- **Ports**: this copy's levels moved to 47852 (game) and 47853 (discovery) so they don't clash
  with the hyperbolic demo's 47800/47801.
- **Balance note, rules unchanged** (**weigh in**): in bot matches the defender usually wins.
  Rounds 2 to 6 of the five-level bot match were all held by the defender, so roles swapped every
  round and neither wavefront moved; that match took 14 rounds. The bots are symmetric, so this
  reads as a home-ground edge (shorter way to one's own consoles). With people it may differ. If it
  holds, options are a time limit per round that the attacker wins, fewer consoles on the
  defender's side, or a respawn delay for defenders; none is implemented.
