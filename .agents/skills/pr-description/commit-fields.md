# Commit fields

A commit message tells the user, in `git log`, what changed, where, what kind of change it is, what to read
for it, and how to verify it. The subject carries the kind and the area, and a block of trailers at the end
carries the rest. Git reads the trailers itself, so `git log --format='%(trailers:key=Kind,valueonly)'` and
`git log --grep='^Domain:.*physics'` find commits by field. The wishlist commit rules are in
[`wishlist-transitions.md`](wishlist-transitions.md) and [`wishlist-history.md`](wishlist-history.md), and the
other commit rules are in the "Commits" section of [`SKILL.md`](SKILL.md).

## The subject

```text
<kind>(<area>): <summary>
```

- `<kind>` is the first value of the `Kind` trailer, and `<area>` is the `Area` trailer.
- `<summary>` is short and specific. When it is about a wishlist item, it names the item's id with its short
  name (`W-85 (stuck leaving a bunker)`), never a bare id.
- A wishlist commit is this form with kind `wishlist` and the wishlist's folder as the area, such as
  `wishlist(demos/hyperbolic_wormholes_rust): add W-203 (pull toward black holes)`.

## The trailers

One block at the end of the message, after the body, the history and the agent traces
([`commit-agent-traces.md`](commit-agent-traces.md)), in this order. The agent's own trailer (the
attribution table in `SKILL.md`) goes last in the same block.

| Trailer | Holds | Values |
| --- | --- | --- |
| `Kind` | what sort of change it is | one or more kinds, the main one first |
| `Component` | which part of the product it changes | one or more components, the main one first |
| `Domain` | which technical field it is in | one or more domains, the main one first |
| `Area` | the demo or folder it mainly changes | one path, relative to the repository root |
| `Wishlist` | the items it is about, one trailer per item | `<wishlist>/<id> (<short name>)`, then `partial` or `complete` |
| `Read` | what to read to understand it | repository paths in reading order, separated by commas |
| `Verify` | how the work was checked | each command or step and its result, or `none (<reason>)` |

- `Wishlist` is left out when the commit is about no item. `complete` means the commits up to this one
  deliver the whole item, and a wishlist commit then moves it to pending review. A wishlist commit repeats
  its own item here, with `complete` when its transition moves the item to `[?]` or `[x]` and `partial`
  otherwise.
- `Verify` is honest: `none (<reason>)` when nothing was run. A wishlist-only commit writes
  `none (checklist only)`.

A whole message, with its subject, line, history and trailers:

```text
wishlist(demos/hyperbolic_wormholes_rust): claim W-203 (pull toward black holes)

* [demos/hyperbolic_wormholes_rust/W-203 (pull toward black holes)] ([ ] -> [~]) CLAIM ON WISHLIST: Claimed by <agent> (session <id>), branch <branch>, 2026-11-10.

[demos/hyperbolic_wormholes_rust/W-203 (pull toward black holes)] HISTORY

* 1a2b3c4d5e6f: (null -> [ ]) ADD TO WISHLIST: W-203 (pull toward black holes): "<the user's words>"

AGENT TRACES

* Claude Code 2.1.321, trace 00000000-0000-4000-8000-000000000000, desktop
  contributed: claimed the item
  workspace: ~/git/Warped/.claude/worktrees/example, branch claude/example
  transcript: ~/.claude/projects/-Users-$USER-git-Warped--claude-worktrees-example/00000000-0000-4000-8000-000000000000.jsonl
  read it: claude --resume 00000000-0000-4000-8000-000000000000

Kind: wishlist, feature
Component: game-design
Domain: physics
Area: demos/hyperbolic_wormholes_rust
Wishlist: demos/hyperbolic_wormholes_rust/W-203 (pull toward black holes) partial
Read: demos/hyperbolic_wormholes_rust/USER_WISHLIST_CHECKLIST.md
Verify: none (checklist only)
Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
```

## The values

A value outside these tables needs the user's agreement. Add its row in the same change.

**Kind**

| Value | Use it when the commit |
| --- | --- |
| `feature` | adds behavior the player or developer can see |
| `bug` | fixes behavior that was wrong |
| `performance` | makes the same behavior faster or smaller |
| `wishlist` | changes a wishlist checklist; add another kind when the item is one (`wishlist, bug`) |
| `docs` | changes only prose, such as a README or a decision note |
| `design` | changes the game design: a spec, rules or a level's intent |
| `refactor` | changes structure and keeps behavior |
| `test` | changes only tests |
| `build` | changes Makefiles, CI, dependencies or tooling |
| `experiment` | compares options or measures something (see "Experiments and benchmarks" in `AGENTS.md`) |

**Component**

| Value | The commit changes |
| --- | --- |
| `client` | the game window or app, on any platform |
| `game-server` | the game server |
| `management-server` | the management server |
| `cloud` | infrastructure and deployment |
| `common` | code, maps or formats that several components share |
| `game-design` | the broader game design, including the wishlist |
| `docs` | documentation that belongs to no other component |
| `tooling` | scripts, skills, hooks and other developer tools |

**Domain**

| Value | The commit is about |
| --- | --- |
| `graphics` | how the 3D world is drawn: rendering, shaders, models, lights, skies |
| `windows` | the game's windows and what they show outside the 3D world: menus, the HUD, screens, and input in them |
| `engine` | the game engine: the game loop, rules, players, weapons and bots |
| `level-layout` | a map's, game's or level's layout: ground, structures, spawns, pylons, game settings |
| `physics` | movement, collisions, bullet paths, curved space |
| `audio` | sounds and music |
| `networking` | what client and server send each other |
| `none` | no technical field, such as a docs-only commit |

> NOTE: if something is two kinds pick the main kind for a title. Wishlist always takes precedence. All kinds can be listed out in the kind listing below.
> NOTE: to an agent---if the domain, component, or kind is not available, ask the user for help.
