# Making Warped testable by agents

What engineers who lean heavily on coding agents have written about letting agents test their own
work, applied to Warped: graphics and gameplay, the client-server system, and latency. Researched
2026-11-06.

Most sources are blog posts or summaries rather than controlled studies. OpenAI's harness
engineering post refused automated reading, so its details come from search excerpts.

## The common idea

An agent works only as well as its ability to check its own work, and the check shouldn't be the
agent's own opinion.

- **Boris Cherny (who created Claude Code)** calls verification his top tip. He estimates it makes
  results 2–3x better.
- **Mitchell Hashimoto's rule:** every time an agent makes a mistake, add a tool or an instruction
  so it can't make that mistake again.
- **Anthropic** found agents marked features done after unit tests or a `curl`, even when the
  feature failed end to end. They did better once told to test as a user would, through a real
  browser. Anthropic also found agents "reliably skew positive when grading their own work", so a
  separate checker helps.

For a game, this means turning "play it and see" into checks a program can run.

## 1. Graphics and gameplay: check below the pixels

- **Run the simulation headless.** Write it as `step(state, inputs, dt) → state`, deterministic
  given a seed. Rendering just shows the state. This is the biggest lever.
  - [`common/physics/`](../common/physics/README.md) already plans test cases as a start state,
    inputs and an expected end state. Use the same format for gameplay rules.
  - [`USER_THOUGHTS.md`](../game_design/brainstorm/USER_THOUGHTS.md) already has good test
    numbers: a console takes 10 s to deactivate, the sniper overheats after about 8 shots, the
    sniper kills in one shot.
- **Record and replay inputs.** Replay a recorded human session headless, then check the end state
  or compare a per-tick state log. Factorio has relied on replay and integration tests to catch
  determinism bugs for years. Turn every bug into a recorded scenario.
- **Use scripted bot clients.** Simple bots (walk to the pylon, hold E, shoot at X) that connect
  over the real protocol. Ubisoft built bots like this for The Division so testers could run
  multiplayer missions alone. The same bots can also generate load.
- **For rendering, check numbers, not looks.** Add a command that renders a known state from a
  fixed camera to a PNG, offscreen.
  - Curved space helps here because many answers are known exactly. In a small 3-sphere you see
    the back of your own head, and a laser in a flat room is straight. Check pixels or projected
    positions numerically.
  - Compare against saved reference images with some tolerance, because GPUs differ.
  - Use the model's vision only to spot obviously broken frames.
  - Debug overlays (hitboxes, light paths, the tick number drawn on the frame) make screenshots
    much more useful. One Godot developer's breakthrough came from turning on collision boxes and
    sending the agent a screenshot.
- **Don't hand the agent a video.** Give it the state log plus a grid of frames from key ticks
  (ffmpeg can make one).
- **The macOS client is the riskiest spot.** In mid-2025 Hashimoto turned off agent self-testing
  for Ghostty's Mac app, because the agent couldn't build or run it or read screenshots. To avoid
  that:
  - Make the client build and run from the command line, behind `make`.
  - Give it a headless mode that takes an input file and a tick count and writes frames to a
    folder.
  - Keep game logic out of the client.
  - The same applies to the starter Rust + SDL3 client in `clients/starter/`.
  - For checks that need a window, Peter Steinberger's Peekaboo gives agents macOS screenshots. It
    needs the Screen Recording permission.
  - The three.js demos in [`demos/`](../demos/README.md) are already easy: an agent can drive them
    in a browser with Playwright.
- **Whether it's fun stays a human call.** Agents can check that a game works, not that it's fun.
  In Anthropic's game-maker experiment, the editors looked fine but nothing responded to input. A
  single "press a key, the state changes" test would have caught it.

## 2. Client and server: runnable specs, one command

- **Turn `common/` into test suites any implementation must pass.** The specs are there because
  components use different languages; the next step is making them runnable.
  - The physics test cases become data files every implementation loads.
  - [`game_server_contract.md`](../common/protocol/game_server_contract.md) becomes a suite run
    against any game server binary: flags, `/health`, admission, and exiting within 5 s of
    SIGTERM.
  - [`management_server/tests/stand_ins.py`](../management_server/tests/stand_ins.py) already has
    a stand-in game server. The suite is the same idea, pointed at the real binary.
- **Start the whole system with one command.** `make dev` would start the management server, N
  game servers and M bots, and write each process's logs to files the agent can read.
  - It should refuse to start twice. Armin Ronacher added a lock file because agents kept starting
    a second server on the same port.
  - His rule for tools: a crash is fine, a hang isn't.
- **Smoke-test at the start of every session.** Anthropic's long-running agents started the app and
  ran one end-to-end check before new work. That caught breakage left by the previous session.
- **Give each worktree its own ports.** Agents run in parallel worktrees, so fixed ports will
  collide. OpenAI's Codex team ran a separate copy of the app per worktree, with its own logs and
  metrics. Their monitoring stack isn't needed: JSON log lines with tick numbers and player IDs in
  `.artifacts/<script>/<run_id>/` (already the repo's convention) work.
- **Simulate the network in-process first.** Put the clock, randomness and network behind
  interfaces, and add a fake network with seeded latency, jitter, loss and reordering. Prediction
  and reconciliation then get fast, repeatable tests, and any failure replays from its seed. This
  is a cheap version of deterministic simulation testing (FoundationDB, TigerBeetle, Antithesis).
- **Then test on a real network.**
  - The system tools (`tc netem` on Linux; Network Link Conditioner or `dnctl`/`pfctl` on macOS)
    need admin rights, so an agent can't run them unattended.
  - A small relay program that delays and drops UDP packets works without admin rights. Toxiproxy
    only handles TCP.
  - One pattern from open-source games: 4–8 bots through a latency proxy, failing the run if client
    and server positions drift apart past a threshold.

## 3. Latency: numbers agents can't fake

- **Write budgets as numbers in the specs.** For example:
  - tick rate
  - 99th-percentile tick time at N players and B bullets
  - snapshot bytes per tick
  - input-to-server latency

  Riot cut Valorant's server frame time from 50 ms to under 2 ms so it could run at 128 ticks per
  second. Numbers like that turn "fast enough" into a test.
- **Split exact measurements from noisy ones.**
  - Bytes per snapshot, messages per tick and allocations per tick give the same answer every run,
    so they can go in every test run.
  - Milliseconds need a fixed machine, warmup, repeats and percentiles, compared to a baseline.
  - A July 2026 study (arXiv 2607.01211) found most reported speedups in agent performance
    benchmarks didn't hold up across machines. Another paper found agents tune for the benchmark
    instead of improving the code.
- **Humans own the yardstick.** Agents can run benchmarks and propose changes, but not edit the
  harness or thresholds. Anthropic's agents could only flip a "passes" field in their feature list.
  Simon Willison says agents do best on problems with clear success criteria that need trial and
  error, and benchmark work like this fits. The
  [`experiment-decisions`](../.agents/skills/experiment-decisions/SKILL.md) skill already fits
  this. Real Wi-Fi vs. wired runs still need two machines and a person.

## 4. Repo-level gaps

As of 2026-11-06:

- **There's no CI.** `.github/` doesn't exist, so agent PRs merge with no automatic check. Add a
  `make check` per component plus one at the root, and run the same command locally, in a Claude
  Code Stop hook, and in CI.
- **[`AGENTS.md`](../AGENTS.md) is 230 lines and loads into every session** (Claude Code reads it
  through `CLAUDE.md`). An ETH Zurich study
  (ICLR 2026) found:
  - Agents follow these files closely.
  - The files raise cost by over 20%.
  - Overviews of the repo don't help.
  - Files written by an LLM slightly lower success.

  Anthropic reportedly suggests keeping these files under about 200 lines. Consider moving the
  naming and docstring rules into a skill that loads when writing code, and enforcing what can be
  enforced with tools (ruff's docstring rules, a small checker for the no-comments rule). OpenAI's
  team made lint error messages explain the fix.
- **Grow instructions from real failures.** Hashimoto adds each instruction line because an agent
  got something wrong. Remove lines that no longer prevent anything: one team found a skill still
  telling agents to run tools deleted months earlier.
- **Use a separate reviewer agent.** A second agent with a skeptical prompt, or `/code-review`,
  catches more than asking the author to double-check.
- **Keep build and test loops fast.** Ronacher favors boring, well-known languages, and found that
  Python's slow startup slowed the agent loop and that agents got confused running Rust's tests.
  The Python game server and the Rust starter client each have a `make test`; keep those quick,
  and weigh this when choosing the macOS client's toolchain.

## Sources

- [Anthropic: Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents)
  (2025-11-26)
- [Anthropic: Harness design for long-running apps](https://www.anthropic.com/engineering/harness-design-long-running-apps)
  (2026-03-24)
- [Mitchell Hashimoto: My AI Adoption Journey](https://mitchellh.com/writing/my-ai-adoption-journey)
  (2026-02-05)
- [Summary of Hashimoto's Ghostty workflow](https://teamday.pages.dev/ai/hashimoto-new-way-of-writing-code)
- [Simon Willison: Designing agentic loops](https://simonwillison.net/2025/Sep/30/designing-agentic-loops/)
  (2025-09-30)
- [Armin Ronacher: Agentic Coding Recommendations](https://lucumr.pocoo.org/2025/6/12/agentic-coding/)
  (2025-06-12)
- [OpenAI: Harness engineering](https://openai.com/index/harness-engineering/) (2026-02-11; read
  through search excerpts only)
- [InfoQ on Boris Cherny's workflow](https://www.infoq.com/news/2026/01/claude-code-creator-workflow)
- [Evaluating AGENTS.md (ETH Zurich)](https://arxiv.org/abs/2602.11988)
- [Performance benchmark reliability audit](https://www.emergentmind.com/papers/2607.01211)
- [PERFOPT-Bench write-up](https://codex.danielvaughan.com/2026/08/01/perfopt-bench-performance-optimisation-coding-agents-codex-cli-profiling-speedup-verification/)
- [Godot with AI coding agents](https://dev.to/abagames/godot-is-well-suited-for-game-development-with-ai-coding-agents-2lcm)
- [AI playtesting for Godot](https://dev.to/orellanaed/ai-playtesting-for-godot-an-agent-can-check-that-your-game-works-not-that-it-is-fun-58gd)
- [GDC: AI-controlled players to test The Division](https://gdcvault.com/play/1026382/Automated-Testing-Using-AI-Controlled)
- Factorio Friday Facts [#60](https://factorio.com/blog/post/fff-60) and
  [#415](https://direct.factorio.com/blog/post/fff-415)
- [Riot: VALORANT's 128-tick servers](https://technology.riotgames.com/node/112)
- [Headless netcode chaos harness example](https://github.com/Jbudone/bitquest/issues/34)
- [A deterministic simulation testing primer](https://www.amplifypartners.com/blog-posts/a-dst-primer-for-unit-test-maxxers)
- [Peekaboo](https://steipete.me/posts/2025/peekaboo-mcp-lightning-fast-macos-screenshots-for-ai-agents)
- [MLOps Community: an AGENTS.md-ready stack](https://mlops.community/blog/the-mlops-adventure-continues-an-agents-md-ready-stack-for-ai-ml)
