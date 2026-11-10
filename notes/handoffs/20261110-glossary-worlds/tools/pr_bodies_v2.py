import os, pathlib, subprocess, sys

BASE_BRANCH = "staging/user-4gatepylon"
REPO = "4gatepylon/Warped"
FOOTER = "🤖 Generated with [Claude Code](https://claude.com/claude-code)"
SKILL = ".agents/skills/pr-description"
SESSION = os.environ["CLAUDE_CODE_SESSION_ID"]
WORKSPACE = "~/git/Warped/.claude/worktrees/wishlist-commit-consistency-fc3638"
TRANSCRIPT = f"~/.claude/projects/-Users-$USER-git-Warped--claude-worktrees-wishlist-commit-consistency-fc3638/{SESSION}.jsonl"
READY = "Ready for review, at the user's request; auto-merge stays off."

def cats(kind, component, domain, area):
    return [f"Kind: {kind}", f"Component: {component}", f"Domain: {domain}", f"Area: {area}", "Wishlist: None."]

def agents(branch, contributed):
    return ("```text\n"
            f"* Claude Code 2.1.321, trace {SESSION}, desktop\n"
            f"  contributed: {contributed}\n"
            f"  workspace: {WORKSPACE}, branch {branch}\n"
            f"  transcript: {TRANSCRIPT}\n"
            f"  read it: jq -rf {SKILL}/chat.jq {TRANSCRIPT} | less\n"
            "```")

LAYERS = [
 dict(n=569, title="pr-description: split into PR and commit sections; one line per wishlist commit",
  summary="The user wants `git log` to show exactly which wishlist change each commit makes, because subjects like `wishlist: add W-NN` read as if they might implement the item. This layer gives the skill a Commits section and defines the one line every wishlist commit carries: the four transitions the user specified, plus the others the checklist needs.",
  changes=[
   f"`{SKILL}/SKILL.md`: split into \"Pull requests\" and \"Commits\" sections, with a file map and a critical pointer for wishlist commits; the Pull requests section says a PR's commits follow the Commits section; the frontmatter description now mentions commits.",
   f"`{SKILL}/SKILL.md`: a `> NOTE` for agents who modify the skill at the user's request: keep every file under 400 lines, split a longer one by topic for progressive disclosure, and have each file name the files to read for what it leaves out.",
   f"`{SKILL}/wishlist-transitions.md` (new): the line `* [<wishlist>/<id> (<short name>)] (<from> -> <to>) <KEYWORD>: <text>`, 12 transitions (ADD TO WISHLIST, CLAIM ON WISHLIST, MOVE TO PENDING ON WISHLIST, MARK CONFIRMED DONE ON WISHLIST as the user wrote them, plus unclaim, take over, update work, link PR, add user's words, reopen, fold, renumber), each with its text and commit subject, and the rules.",
  ],
  decisions=[
   "Every line and subject names the wishlist (`<wishlist>` is the checklist's folder), because the user asked that a commit say which wishlist it changes, and ids repeat across checklists (W-35 exists in two). This slightly extends the user's `[<id and tiny desc>]` bracket.",
   "One commit changes exactly one item and nothing else, not even code. The user asked for no batch commits; \"nothing else\" is stricter, so that reverting a commit reverts exactly one transition. `RENUMBER` is the one exception, since it rewrites references to the old id.",
   "Beyond the user's four transitions, seven come from the 348 existing wishlist commits (unclaim, take over, update work, link PR, add user's words, fold, renumber). One is new policy: `REOPEN` (`[x] -> [ ]`) for a done item that broke.",
   "The word `add` in a `wishlist(...)` subject only ever means ADD TO WISHLIST; a PR number is \"linked\" (history showed `add #NNN to W-126` meaning a PR link).",
   "The lines are for commits only, as the user said; the PR template is unchanged in this layer.",
   "The skill keeps the name `pr-description`: renaming would break references in the root `AGENTS.md` and other skills.",
   READY,
  ],
  testing=["Checked with a script: the transitions table has 12 rows of 4 columns, the example line matches the line's pattern, and every relative link in the skill resolves.", "Not run: no code in this layer."],
  size="0 (Markdown only)"),
 dict(n=570, title="pr-description: append-only wishlist history in commit messages, with a search command",
  summary="So an item's whole life, and any reversion, can be read from one commit message, every wishlist commit now carries a HISTORY section for its item, and a command writes it so no one has to do it by hand.",
  changes=[
   f"`{SKILL}/wishlist-history.md` (new): the section `[<wishlist>/<id> (<short name>)] HISTORY` with one `* <hash>: (<from> -> <to>) <KEYWORD>: <text>` bullet per earlier commit, the append-only rule, how to revert, and the limits.",
   f"`{SKILL}/wishlist_history.py` (new): a click command, `WISHLIST ITEM_ID SHORT_NAME [--rev] [--json]`, that finds the newest earlier commit about the item and prints the section the next commit must carry; it validates its arguments and fails naming the commit it can't read.",
   f"`{SKILL}/wishlist-transitions.md`: adds `REVERT ON WISHLIST` (now 13 transitions) and a pointer to the history file.",
   f"`{SKILL}/SKILL.md`: the file map and the Commits bullet mention the history.",
  ],
  decisions=[
   "The header has no leading `#` (the user's sketch had one), because git strips `#` lines when it opens an editor for the message, for example on `git commit --amend`.",
   "A section lists the commits before its own commit, since a commit can't contain its own hash; the commit's own change is its transition line, and the next commit adds its bullet.",
   "A bullet is the transition line without the item bracket, with the text cut to 72 characters, and the hash is 12 characters long. Both are constants in the script.",
   "A revert is a new commit with the `REVERT ON WISHLIST` keyword and text `reverts <hash>`, so reversions show up in the history and in `git log --grep`.",
   "Rebasing or amending changes hashes, so old bullets can name commits that no longer exist; the file says so under Limits rather than letting anyone rewrite a bullet.",
   "The script's tests are in the next layer: together they were 236 lines, over the 200-line cap.",
   READY,
  ],
  testing=["`python3 .agents/skills/pr-description/wishlist_history.py demos/hyperbolic_wormholes_rust W-203 \"pull toward black holes\"` on this repo: prints the header only (no earlier commit uses the format), exit 0; `--json` prints `previous_commit: null`.", "The same command with `demos/nope`: exit 2 with an error naming WISHLIST; `--help` ends with the example.", "Unit tests are in #571, where all 9 pass."],
  size="146 (`wishlist_history.py`; the rest is Markdown)"),
 dict(n=571, title="pr-description: test wishlist_history.py",
  summary="`wishlist_history.py` (#570) decides what every wishlist commit's history says, so its behavior is pinned by tests, one per part of each way its input can vary.",
  changes=[f"`{SKILL}/test_wishlist_history.py` (new): 9 pytest tests that run the command in a temporary git repository. The docstring lists each partitioning and the tests that cover its parts: earlier commits for the item (none, one without history, one with, several), text length (cut at 72), ids that share a prefix (W-20 against W-203), bad input (id, unknown wishlist, commit without a transition line) and output format (text, JSON)."],
  decisions=["The tests drive the command through click's `CliRunner`, because the command line is the file's only interface; its other functions are private.", "Tests are their own layer so each layer stays under the 200-line cap (the script is 146 lines and the tests 92).", READY],
  testing=["`python3 -m pytest -q .agents/skills/pr-description/test_wishlist_history.py`: 9 passed (rerun after the rebase onto current staging)."],
  size="92 (`test_wishlist_history.py`)"),
 dict(n=572, title=f"docs({SKILL}): kind, component, domain and area on every commit and PR",
  summary="The user wants `git log` to say at a glance what a commit changes, where, what kind of change it is, what to read for it and how it was verified. This layer proposes the fields and their values as defaults for review.",
  categories=cats("docs", "tooling", "none", SKILL),
  changes=[
   f"`{SKILL}/commit-fields.md` (new): the subject `<kind>(<area>): <summary>`, a trailer block (`Kind`, `Component`, `Domain`, `Area`, `Wishlist`, `Read`, `Verify`) that git reads itself, a whole example message, and the allowed values.",
   f"`{SKILL}/SKILL.md`: a Commits bullet for every commit, a `## Categories` section in the PR template with a Filling-it-in bullet, and PR titles in the commit-subject form.",
   f"`{SKILL}/wishlist-history.md`, `{SKILL}/wishlist-transitions.md`: pointers to the new file.",
  ],
  decisions=[
   "The domains use the user's words: graphics (how the 3D world is drawn), windows (the game's windows and what they show outside the 3D world), engine, level-layout and physics; I added audio, networking and none.",
   "Kinds: feature, bug, performance, wishlist, docs, design, refactor, test, build, experiment. Components: client, game-server, management-server, cloud, common, game-design, docs, tooling. A value outside the tables needs the user's agreement.",
   "Beyond the categories the user listed, I added three trailers: `Wishlist` (items touched, `partial` or `complete`), `Read` (what to read, in order) and `Verify` (what was checked, or `none (<reason>)`), because the user's goal includes knowing what is done, what to read and how to verify.",
   "Trailers are git trailers, so `git log --format='%(trailers:key=Kind,valueonly)'` and `--grep` work on them, and a commit can hold several kinds (`wishlist, bug`).",
   "The PR template changes in this layer (a `## Categories` section, and titles in the commit form), which the template's note allows in its own PR. This PR and the ones above it already use that form.",
   READY,
  ],
  testing=["Checked with a script: every relative link in the skill's Markdown files resolves. `git log -1 --format='%(trailers:key=Kind,valueonly)'` reads this layer's own trailers.", "Not run: no code in this layer."],
  size="0 (Markdown only)"),
 dict(n=573, title=f"docs({SKILL}): AGENT TRACES section names each agent and its transcript",
  summary="The user wants to read how a change came about from the commit alone, so each commit lists the agents that produced it, with trace ids, where they ran, what each contributed, and a command that reads their transcript or fails at once.",
  categories=cats("docs, feature", "tooling", "none", SKILL),
  agents=agents("claude/commit-agent-traces", "designed and wrote the section and chat.jq, then verified the reading command"),
  changes=[
   f"`{SKILL}/commit-agent-traces.md` (new): the `AGENT TRACES` section (agent and version, trace id, desktop or cloud, contribution, workspace and branch, transcript, command to read it), where each agent keeps its trace, a command that prints a desktop Claude Code agent's fields, how to read a transcript, and the fail-fast rule.",
   f"`{SKILL}/chat.jq` (new): a `jq` filter that prints a Claude Code transcript as plain text, each user and assistant message and one line per tool call.",
   f"`{SKILL}/SKILL.md`: a Commits bullet, file-map entries, and an `## Agents` section in the PR template with a Filling-it-in bullet.",
   f"`{SKILL}/commit-fields.md`: the whole-message example and the trailer position mention the section.",
  ],
  decisions=[
   "`read it` is `jq -rf .agents/skills/pr-description/chat.jq <transcript> | less`. It replaces `claude --resume <id>`, which an earlier commit of this layer named without running it, and which continues the session instead of only reading it.",
   "The command fails at once, with exit code 2, when the transcript is not on this computer. It needs `jq`.",
   "Paths never carry user-specific parts: the home folder is `~` and the user name is `$USER`, including inside the encoded project folder name Claude Code uses.",
   "Only the Claude Code desktop row is checked. The Codex, Devin and cloud rows come from the agents' documented behavior, are marked unchecked, and the first agent of each type to commit fixes its row.",
   "An agent that can't fill every field does not commit and asks the user, instead of writing a placeholder.",
   "`chat.jq` has no unit tests; it was run by hand on a real transcript and a missing file.",
   READY,
  ],
  testing=["Ran the pasted `read it` command, with `~` and `$USER` in the path, on this session's transcript: 504 lines, 180 of them tool calls.", "Ran it on a missing file: `Could not open file`, exit code 2.", "Ran the fields command from the file on this session: it found the transcript and printed masked paths."],
  size="23 (`chat.jq`; the rest is Markdown)"),
 dict(n=575, title="docs(README.md): name the official game, how to run it, and what the other folders are",
  summary="It should be obvious to everyone where the real game is and how to run it. The root README now opens with the official game, a tiny run guide, and the status of every other folder, with a note telling agents to ask the user to help update it when it goes stale.",
  categories=cats("docs", "docs", "none", "README.md"),
  agents=agents("claude/readme-official-game", "wrote the official-game section and the folder table, and moved it to the top"),
  changes=[
   "`README.md`: the \"official MVP\" section becomes \"The official game\", now the first section: a NOTE (ask the user to help update it if outdated), the folder, the two-line run guide, and a table, \"The other folders\", that gives each folder a status (official, part of the official game, supports it, historical and to be replaced soon, planned but unused, historical).",
   "`README.md`: the old starter game's section is retitled \"Play the old starter game on your Wi-Fi (historical)\".",
  ],
  decisions=[
   "The official game is `demos/warped_mvp_rust/`, and the \"main 2D game that will be replaced soon\" is the starter square game (`clients/starter/`, `game_server/`, `management_server/`, `common/protocol/`). Both confirmed by the user, 2026-11-10.",
   "`common/physics/` and `clients/mac/` are marked \"planned, unused so far\": each holds only a README, and no code or doc of the official game refers to `common/physics` (checked with grep).",
   "`common/maps/` and `infra/` are marked \"supports the official game\": `demos/city_sphere_rust` reads `common/maps`, and `infra/` deploys the Rust game.",
   "The run guide is copied from the app's own README and Makefile, so it says what they say.",
   READY,
  ],
  testing=["`make -n -C demos/warped_mvp_rust run-warped`: printed the build and run commands. The game itself was not launched, because the user was at the computer.", "Checked with a script: every folder the README names exists, and the table's rows have a consistent shape."],
  size="0 (Markdown only)"),
 dict(n=576, title="docs(game_design): glossary of the game's terms, and a rule that every agent uses it",
  summary="Agents and commits have used some game words inconsistently (game and match, map and level, the HUD, the screens). This adds one glossary, with a single meaning per term, and a rule that every agent uses it and asks the player a precise question when a term is missing or unclear.",
  categories=cats("docs, design", "game-design", "none", "game_design"),
  agents=agents("claude/game-glossary", "researched the terms in DESIGN.md, the READMEs, the wishlist and every commit subject, and wrote the glossary and the AGENTS.md rule"),
  changes=[
   "`game_design/GLOSSARY.md` (new, about 230 lines): the terms for the structure of play, people and bodies, programs and places, screens and windows, the HUD, world objects and effects, moving and looking, space and physics, looks of the world, building and shipping, and tracking work; a table of retired words; and eight questions for the player.",
   "`AGENTS.md`: a Glossary section (every agent MUST use the terms exactly; ask the player a precise question for a new or unclear term). To stay under 16 KiB, the git commands of \"Changes from others\" are dropped (the `compatible-changes` skill has them) and three passages are tightened.",
   "`game_design/DESIGN.md`: its Terms table, which defined \"match\" differently, becomes a pointer to the glossary.",
  ],
  decisions=[
   "The rule is in the root `AGENTS.md`, not `CLAUDE.md`: `CLAUDE.md` imports `AGENTS.md` and says to edit that file, so Claude Code, Codex and Devin all read the rule there.",
   "A game is one that hasn't started and a match is an ongoing game, as the user defined them. A map is the line of levels, a level one playable space, and a level's layout data is a \"level layout\" (a proposal: `AGENTS.md` and `common/maps/` called the layout a \"map\").",
   "The HUD is the information seen while walking normally: the health, energy and ammo bars and the minimap, not the map itself (the user, 2026-11-10). The crosshair, console bar, readouts and pylon squares are in it as a proposal. Whether it shows behind the open menu, under the `Tab` map, while dead, or in the lobby is a question, not a guess.",
   "Coverage: I counted every word and word pair in the 1,026 commit subjects and added the frequent game terms the first version lacked. Rare terms, and terms that only appear in commit bodies, may still be missing.",
   "Where the project used two words, I picked one and listed the other as retired. Picks marked *proposed* are mine: level edge, game server, level layout, boundary plane, pull, deactivate, sprint and walk, haze.",
   "`AGENTS.md` is now 16,346 bytes, 38 under the 16 KiB limit, so the next rule added there needs another trim.",
   READY,
  ],
  testing=["Checked with a script: every relative link in the three files resolves, and each glossary table has a consistent column count.", "Checked `AGENTS.md`'s size: 16,346 bytes.", "Not run: no code in this layer."],
  size="0 (Markdown only)"),
 dict(n=579, title="docs(AGENTS.md): point the root and demo AGENTS.md files at the new commit rules",
  summary="The new commit rules (#569 to #573) only help if the files agents read first send them there. The root `AGENTS.md` now says commit messages follow the `pr-description` skill, and the demos' `AGENTS.md` files describe wishlist titles in the new `wishlist(<wishlist>): ...` form.",
  categories=cats("docs", "tooling", "none", "AGENTS.md"),
  agents=agents("claude/agents-md-commit-rules", "pointed the root and demo AGENTS.md files at the new commit rules"),
  changes=[
   "`AGENTS.md`: \"Write every PR description and commit message with the `pr-description` skill\" (the root file is now 16,365 bytes, under the 16 KiB limit).",
   "`demos/hyperbolic_wormholes_rust/AGENTS.md`, `demos/dust_rust/AGENTS.md`, `demos/city_sphere_rust/AGENTS.md`, `demos/snow_planet_rust/AGENTS.md`: a bullet that sends wishlist commits to `wishlist-transitions.md` and `wishlist-history.md`; titles `wishlist: add|claim|unclaim|... W-NN` become `wishlist(<wishlist>): ...`; the pending step is its own commit (`MOVE TO PENDING ON WISHLIST`); the example that put two ids in one title now just says to name each id.",
  ],
  decisions=[
   "The four demo files are verbatim copies, so each got the same text, with `<wishlist>` left generic (\"the folder that holds the checklist\").",
   "Found, not fixed: `demos/dust_rust/AGENTS.md` links to a `USER_WISHLIST_CHECKLIST.md` that doesn't exist in that folder; `demos/snow_planet_rust/` has its own checklist, a verbatim copy from 2026-11-08 with 115 items and one commit ever, while the hyperbolic one has 203 items and 547 commits. The live wishlist is the hyperbolic one; which to keep is the user's call.",
   "The root `AGENTS.md` has 19 bytes to spare, so the next rule added there needs another trim.",
   READY,
  ],
  testing=["No old `wishlist: ` title form is left in any `AGENTS.md` (grep).", "Each substitution matched exactly once in each of the four files (asserted by the edit script).", "Not run: no code in this layer."],
  size="0 (Markdown only)"),
 dict(n=580, title="docs(game_design): the scene window graph (SWG), a conceptual model of what the player sees outside the 3D world",
  summary="The user wants the windows and visuals to be, conceptually, a DOM, so visuals are easy to explain and later to implement. This adds the scene window graph (SWG, pronounced \"swig\") as terminology and a mental model, in the user's words. It does not create an SWG.",
  categories=cats("docs, design", "game-design", "windows", "game_design"),
  agents=agents("claude/ui-tree", "wrote the scene window graph as a conceptual model in the player's words, and its glossary terms"),
  changes=[
   "`game_design/SCENE_WINDOW_GRAPH.md` (new, 60 lines): the model in eight points (what an SWG holds; a tree; only leaves are rendered; a leaf is a render or a UIE; x, y and z offsets; a group's leaves share a z by design; hidden parts; different windows are different SWGs), the regular game as a small example, and three questions.",
   "`game_design/GLOSSARY.md`: rows for the SWG, render (noun and verb), scene render, minimap render, UIE and main map; \"level view\" is retired for the scene render; question 9 on the word window.",
   "`game_design/README.md`: lists the glossary and the SWG file.",
  ],
  decisions=[
   "No SWG file, language or code is created, as the user asked. A NOTE says one may come later and tells agents not to create one until the user asks.",
   "An earlier version of this layer listed every node of four SWGs with states and a z order. I removed that: it was an actual SWG in all but name, mostly made of my own proposals, and the user asked for terminology and a mental model only.",
   "The example is the user's own: the game/match SWG has the scene render and the HUD, the HUD has the minimap render and each of the bars, and the main map is a hidden part.",
   "*Group* for a node that isn't a leaf is my word (question 2).",
   READY,
  ],
  testing=["Checked with a script: every relative link resolves, and every glossary table has a consistent column count.", "Not run: no code in this layer."],
  size="0 (Markdown only)"),
]

def stack_section(i: int) -> str:
    m = len(LAYERS)
    below = f"`{BASE_BRANCH}`" if i == 0 else f"#{LAYERS[i-1]['n']}"
    above = "" if i == m - 1 else f"; #{LAYERS[i+1]['n']} builds on this"
    return f"Layer {i+1}/{m}. Based on {below}{above}."

def body(i: int) -> str:
    L = LAYERS[i]
    lines = ["## Summary", "", L["summary"], ""]
    if "categories" in L:
        lines += ["## Categories", ""] + [f"- {c}" for c in L["categories"]] + [""]
    lines += ["## Changes", ""] + [f"- {c}" for c in L["changes"]]
    lines += ["", "## Decisions", ""] + [f"- {d}" for d in L["decisions"]]
    lines += ["", "## Testing", ""] + [f"- {t}" for t in L["testing"]]
    lines += ["", "## Pictures", "", "None: nothing visible changes (Markdown, a script and a filter for developers).", ""]
    if "agents" in L:
        lines += ["## Agents", "", L["agents"], ""]
    lines += ["## Stack", "", stack_section(i), "", "## Size", "", f"Reviewable lines: {L['size']}", "", FOOTER, ""]
    return "\n".join(lines)

if __name__ == "__main__":
    scratch = pathlib.Path(sys.argv[1])
    for i, L in enumerate(LAYERS):
        path = scratch / f"body_{L['n']}.md"
        path.write_text(body(i))
        subprocess.run(["gh", "api", "-X", "PATCH", f"repos/{REPO}/pulls/{L['n']}", "-f", f"title={L['title']}", "-F", f"body=@{path}", "--silent"], check=True)
        print("updated", L["n"])
