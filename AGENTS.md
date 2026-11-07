These rules apply to the whole repository. Codex and Devin read them here, in `AGENTS.md`;
Claude Code reads them through `CLAUDE.md`, which imports this file. Folders can add their own
rules in their own `AGENTS.md`, with a `CLAUDE.md` beside it that imports it the same way.

Skills live in `.agents/skills/`, where Codex and Devin find them. `.claude/skills` is a symlink
to that folder for Claude Code, so add new skills under `.agents/skills/`.

Keep this file under 16 KiB, because Devin reads only the first 16 KiB of an `AGENTS.md`. A rule
that applies only to some kinds of work goes in a skill, and this file points to it.

## Pull requests

- Keep each PR small enough to review: at most 200 non-blank lines of hand-written code (source,
  scripts, build files and configs, comments included), counting added and changed lines. Prose
  such as READMEs doesn't count, but keep it minimal. Split larger changes into stacked PRs with
  the `stacked-prs` skill (`.agents/skills/stacked-prs/`).
- Lockfiles and verbatim copies of existing files don't count.
- Generated code (output of a script or compiler) doesn't count either, but commit it only if the
  user agrees. A PR may go well over 200 lines this way: the limit is on what reviewers read.
- State the reviewable line count in the PR description.
- Write every PR description with the `pr-description` skill (`.agents/skills/pr-description/`).
  Every PR uses its template unchanged.

## Issues

Open, format, label and split GitHub issues with the `github-issue` skill
(`.agents/skills/github-issue/`). Every issue uses its title format, template and labels.

## Experiments and benchmarks

For experiments and benchmarks (anything that compares options or measures how well something
works, such as two physics integrators or the game server's time per tick), let the user make the
important decisions: ask rather than choose. Whatever you do decide, list in the PR with a short
rationale. Follow the `experiment-decisions` skill (`.agents/skills/experiment-decisions/`) for
when to ask, the template, and where the list goes (PR description or a per-experiment
`DECISIONS.md`).

## Uncommitted files

Everything that must not be committed (compiler output, generated code, logs and other run
outputs) goes in one of these git-ignored folders at the root of its component folder (e.g.
`game_server/.build/`), never at the repository root or next to the sources. Any other git-ignored
folder a component needs is dot-prefixed too. Each component's `.gitignore` lists its folders, and
its README says which scripts write to them.

| Folder | Holds | Recreated by | Removed by |
| --- | --- | --- | --- |
| `.build/` | compiler output and generated code: binaries, bundles, code generated from `.proto` files | rebuilding | `make clean` |
| `.artifacts/` | run outputs: logs, benchmark results, profiler reports, plots | rerunning | `make reset-artifacts` |

- **Runs:** a script writes each run to `.artifacts/<script>/<run_id>/`. `<script>` is the
  script's file stem (`bench_physics`), and `<run_id>` is the UTC start time, `YYYYMMDD-HHMMSS`,
  optionally followed by `-<label>`, so the newest run sorts last.
- **Provenance:** each run folder has a `command.txt` with the exact command line and the git
  commit (`git describe --always --dirty`), so a past result can be found, read and reproduced.
- **Paths:** scripts build these paths from the component folder (`Path(__file__)`), not from the
  current directory.
- **Which folders:** a component has only the folders it needs. Before creating a git-ignored
  folder that isn't in the table, ask the user.
- **Reset:** `make reset-artifacts` deletes `.artifacts/`. Like every destructive command, it fails
  with a warning unless it gets `YES_TO_RESET_ARTIFACTS=1`, or `INTERACTIVE=1` and the user types
  `yes`.

## Makefiles

Write or change a component's Makefile with the `makefiles` skill (`.agents/skills/makefiles/`).

## Command-line interfaces

Write every script's and main's command-line interface with the `clis` skill
(`.agents/skills/clis/`): click in Python, `getopt_long` in C and C++, non-interactive and
idempotent by default, `--yes-to-<action>` and `--interactive` for destructive actions, and one
vocabulary of flags across the repo.

## Code practices

These rules are for new code in every language; don't rewrite existing code only to follow them.

### Names and declarations

A name should say what a value is without making the reader look at the code around it. Match a
name's length to its scope: `i`, `j` and `x` are fine for a few lines, but a name that lives across
a function or a file, or that a decision depends on, spells out its meaning.

- **Intent:** say what the value is for, not what type it is: `connected_player_count`, not
  `count`.
- **Number:** collections are plural, scalars singular: `for player in players`.
- **Units:** every quantity that has a unit carries a suffix from the table below: `timeout_ms`,
  `message_bytes`.
- **Ratios:** a value that is one quantity per another, such as a conversion factor, is named
  `<numerator>_per_<denominator>`: `bytes_per_float32` and `meters_per_mile`, not
  `float32_bytes`, `bytes_float32`, `mile_meters` or `meters_in_mile`.
- **Source:** a measured value, or one taken from a spec or other document, says where it comes
  from with `_according_to_<source>`, and `<source>` makes clear whether it is a spec or a
  measurement: `tick_ms_according_to_perf_counter`, `MAX_REFRESH_HZ_ACCORDING_TO_APPLE_SPEC`, not
  `_according_to_apple`.
- **Maker:** when it matters what made a thing, say so with `_created_by_<maker>`:
  `player_id_created_by_management_server`.
- **Range bounds:** an index, pointer or port that starts or ends a range (other than a loop
  variable) carries `_incl` or `_excl`: `end_excl`, `last_port_incl`.
- **Booleans:** read as a yes/no question, starting with a verb such as `is_`, `has_`, `can_` or
  `should_`: `is_full`, not `full`.
- **Temporaries:** a throwaway value starts with `temp_`: `temp_sum`.
- **Functions:** `<verb>_<noun>`, the action and what it acts on, optionally followed by context:
  `add_player`, `move_box_outside`. Interchangeable implementations of one operation may be
  `<operation>_<variant>`: `integrate_rk4`.
- **Dangerous functions:** a function that is dangerous to call ends in
  `_DANGEROUS_DANGEROUS_DANGEROUS_BECAUSE_<REASON>`:
  `forget_game_server_DANGEROUS_DANGEROUS_DANGEROUS_BECAUSE_IT_KEEPS_RUNNING`.
- **Types:** classes and structs are nouns for the object, not the action: `Launcher`, not
  `Launch`. Type aliases end in `_t`: `player_id_t`, `tick_fn_t`.
- **Math letters:** where code follows a formula or a standard algorithm (physics, geometry,
  linear algebra), use the formula's letters: `x`, `v` and `a` for position, velocity and
  acceleration, and `dt` for the time step. Matrices are capitals (`A`, `R`); vectors are
  lowercase (`x`, `b`); sizes and lengths are capitals (`N`); loop indices are `i`, `j`, `k`, or
  the lowercase of the size they run over (`n` over `N`); spatial coordinates and quaternion parts
  are `x`, `y`, `z`, `w`. There these letters override the length, intent and unit rules, so a
  time step stays `dt` across a whole file. Elsewhere, name arrays and sizes by what they hold:
  `players`, `bullet_count`.
- **Location:** a GPU buffer, or a pointer to one, ends in `_device`, and its copy in CPU memory
  in `_host`: `positions_device`, `positions_host`. Plain scalars, and code that never touches a
  GPU, don't take it.
- **Order:** when a name takes several suffixes, they go `<what>_<unit>`, then `_incl`/`_excl`,
  then `_according_to_<source>` or `_created_by_<maker>`, then the location:
  `tick_ms_according_to_perf_counter`, `end_offset_bytes_excl_device`.
- **Case:** snake_case for variables and free functions, in every language. Classes are the
  exception, and their rules take precedence: class and struct names are UpperCamelCase
  (`Launcher`), methods are lowerCamelCase (`moveBoxOutside`), and static constructors are
  UpperCamelCase (`Launcher::FromConfig`). Globals, global constants, macros and environment
  variables are UPPER_SNAKE_CASE (`DEFAULT_PORT`, `MAX_PLAYER_COUNT`). The math letters above keep
  their case. A Python script's work goes in a `main()` function, so its working variables are
  locals and only constants sit at module level.
- **No magic numbers:** every literal with a meaning gets a named constant: `constexpr` in C++, a
  module-level constant in Python, `#define` only where a constant can't work. Bare `0` and `1` as
  start values, identities or increments are fine.
- **Macros:** a `#define` goes at the top of its file, after the includes, unless there is a good
  reason to put it elsewhere. A macro anywhere else gets a one-line comment right above it that
  says why it is there and where it is used:
  `// MAX_BULLET_COUNT is here because only spawn_bullet uses it; no other file includes it.`
- **Privacy:** everything that is not part of the interface of a module, file or class is private,
  so that only the interface can be imported. Keep as much private as possible.
  - Python: private module names, attributes and methods start with `_`: `_pick_port`.
  - C++: names used by only one `.cc` file go in an anonymous namespace there, headers declare
    only the interface, and class members outside the interface are `private:`, with data members
    ending in `_` (Google style): `player_count_`.
- **Declared types:** write the type when declaring a variable, in Python too
  (`timeout_ms: float = ...`). Use C++ `auto` only where the type can't be written (lambdas).

| Quantity | Suffixes | Meaning |
| --- | --- | --- |
| Time | `_ns`, `_us`, `_ms`, `_s` | |
| Frequency | `_hz` | per second, e.g. a tick rate |
| Data | `_bits`, `_bytes` | |
| Data, decimal | `_kbytes`, `_mbytes`, `_gbytes` | 10^3, 10^6, 10^9 bytes |
| Data, decimal bits | `_kbits`, `_mbits` | 10^3, 10^6 bits |
| Data, binary | `_kibytes`, `_mibytes`, `_gibytes` | 2^10, 2^20, 2^30 bytes |
| Bandwidth | `_kbits_per_s`, `_mbits_per_s`, `_mbytes_per_s` | not `_mbps`: bits or bytes? |
| Count | `_count` | a number of items: `player_count` |
| Fraction | `_frac`, `_pct` | 0 to 1, 0 to 100 |

For a unit the table lacks, build the suffix the same way, from lowercase unit names joined with
`_per_` (`_rad`, `_rad_per_s`), and add its row here in the same change.

### Comments

Code has no comments except docstrings, DANGEROUS comments and macro comments (see Macros). Tool
directives such as `// NOLINT(...)`, `# noqa`, `#pragma` and shebangs aren't comments for this rule.

- **Docstrings:** every file, class and function has one docstring, in the template for its
  language from the `docstrings` skill (`.agents/skills/docstrings/`). It gives the purpose, the
  interface, an example and, optionally, what a reader must know.
- **DANGEROUS comments:** a line that someone MUST know something about before changing it gets a
  one-line comment right above it:
  `// DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN <CONSEQUENCE> WILL HAPPEN` (`#` in
  Python). In a dangerous section, every line gets one. A DANGEROUS comment longer than one line
  needs the user's approval. Nothing else gets a comment.
- **Phrasing:** every sentence names its subject and uses the active voice: "`add_player` lets one
  more player join a game server.", not "Lets one more player join a game server.". Write
  directly, clearly and briefly. Separate points are bullets; a line of reasoning is a paragraph.

### Failing fast

Code is written for its correct inputs only, and fails at once, explicitly and obviously, on any
input that breaks its preconditions. It doesn't guess a default, skip a bad value or carry on with
partial data.

- **Layers:** each function handles the correct inputs of its own layer of abstraction. It doesn't
  defend against every way a caller or a lower layer might go wrong; anything else fails.
- **Dictionaries:** Python reads a key with `d[key]`, never `d.get(key)`, so a missing key raises
  `KeyError` where it is read. Where a key or a value may legitimately be missing, the code checks
  for that case explicitly (`if key in d`, `if value is None`) and handles it on purpose.
- **Fallbacks:** `x or {}`, `x or []` and broad `except` clauses don't stand in for a value that
  should be there. A value that may be null is checked with `is None`.
- **Errors:** a failed precondition raises an error that names the input and what was expected,
  rather than letting a wrong value travel on and fail somewhere else.

### Why

These practices aim for fewer bugs, code that reads quickly, and short code. They get there by
keeping code standard, informative and correct. When something you write isn't covered by a rule
here, ask the user whether to set one, so that conventions don't drift apart.

## Unit tests

Choose unit tests by partitioning the input space, and list the partitionings and the tests
that cover each part in the test file's docstring. Follow the `unit-tests` skill
(`.agents/skills/unit-tests/`) for the steps.

## Playtesting

When the user tests a game or other visual work you made, follow the `playtesting` skill
(`.agents/skills/playtesting/`).

## Machine-specific workarounds

This repo is used from several machines, so don't write a workaround for one machine (e.g. a
broken `gh` command) as if it held everywhere. In `AGENTS.md` files and skills, write it as a
condition, a one-line note on where it was seen, and the fix. Keep it to a line or two:

> If `gh pr edit` fails with a Projects (classic) error (seen once with gh 2.55 on a Linux machine,
> 2026-10-30), set the body with `gh api -X PATCH repos/<owner>/<repo>/pulls/<n> -f body=...`.

## Documenting the system

When you record the system a result came from (machine, OS, network, toolchain versions), follow
the `system-docs` skill (`.agents/skills/system-docs/`).

> NOTE: if any of this is outdated notify the user.
