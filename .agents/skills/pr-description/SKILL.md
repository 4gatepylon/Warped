---
name: pr-description
description: Write or update a pull request description using this repo's standard template, write every commit message in this repo's standard format, and open every PR against the user's `staging/user-<username>` branch (agents merge into `staging/global` or `main` only with explicit permission). Use whenever you open a PR (gh pr create, gh stack submit), edit a PR's body, the user asks for a PR description, title or summary, you attach screenshots or videos to a PR, you write a commit message, or a commit changes a wishlist item's status (critical: `wishlist-transitions.md`). Every PR in this repo uses the same template.
---

# PR descriptions and commit messages

Every PR and every commit in this repo follows the formats in this skill. "Pull requests" covers the PR
description (and title), and "Commits" covers the commit message. The PR size rule is in the root `AGENTS.md`,
and how to split and stack PRs is in the `stacked-prs` skill (`.agents/skills/stacked-prs/`).

Files in this skill:

- `SKILL.md` (this file): the PR template and the commit rules.
- [`wishlist-transitions.md`](wishlist-transitions.md): the line every wishlist commit carries. Read it before
  any commit that changes a wishlist checklist.
- [`wishlist-history.md`](wishlist-history.md): the append-only history section below that line, and how to
  revert. Read it with the file above.
- [`wishlist_history.py`](wishlist_history.py): the command that writes the history section.
- [`commit-fields.md`](commit-fields.md): the subject and the trailers (kind, component, domain, area,
  wishlist, read, verify) of every commit and PR, and their allowed values. Read it before any commit.

> NOTE: If you modify this skill (at the user's request), keep every file in it under 400 lines. Split a longer
> file by topic, for progressive disclosure, and have each file name the files to read for what it leaves out,
> so that an agent that finds a gap knows where to look next.

## Pull requests

A PR's commits follow the "Commits" section below. That is critical for a PR with a wishlist commit.

> NOTE: The template is a living document: it may change, but at any point in time every PR uses
> the current version. If a PR doesn't fit it (a section makes no sense, or something important
> has no place), stop and ask the user for guidance rather than improvising a variant. If the
> template needs to change, change it in this skill in its own PR.

### Base branch

- Open every PR against your user's `staging/user-<username>`, never straight against
  `staging/global` or `main`. In a stack, layer 1's base is `staging/user-<username>`.
- Agents may merge PRs into their user's `staging/user-<username>` themselves, once the PR's own
  tests pass.
- An agent merges into `staging/global` or `main` only after the user explicitly says yes to that
  merge. How the branches fit together is in the `merging-to-staging` skill.

### Template

```markdown
## Summary

<1-3 sentences: what the PR changes and why.>

## Categories

- Kind: <kind>[, <kind>...]
- Component: <component>[, <component>...]
- Domain: <domain>[, <domain>...]
- Area: <the demo or folder the PR mainly changes>
- Wishlist: <None.> or <wishlist>/<id> (<short name>), one per item

## Changes

- <one bullet per concrete change, naming the files or folders touched>

## Decisions

<None.> or <the decisions you made rather than the user, each with a short rationale>

## Testing

- <each command run and its result, e.g. `make test`: 12 tests pass>
- <or: Not run: <reason>>

## Pictures

<None: <why nothing visible changes>.> or <screenshots, videos, GIFs or plots of the change, each under a caption>

## Stack

<Not stacked.> or <Layer n/m. Based on #<PR below>; #<PR above> builds on this.>

## Size

Reviewable lines: <N> (<what they are, e.g. "Markdown only" or "src/ + tests/">)

🤖 Generated with <agent>
```

### Filling it in

- **Title**: short and specific, `<kind>(<area>): <summary>` (e.g.
  `feature(demos/hyperbolic_wormholes_rust): curved-geodesic movement`), the form of a commit
  subject in `commit-fields.md`. For a stack layer, no "1/2" numbering in the title; the Stack
  section carries it.
- **Summary**: the why matters more than the what; the diff already shows the what.
- **Categories**: the PR's own values for the fields of `commit-fields.md`, the main value first. They
  cover all of the PR's commits, so each value of a commit's `Kind`, `Component` and `Domain` trailers
  appears here.
- **Changes**: concrete and complete, but no line-by-line narration of the diff.
- **Decisions**: the choices you made without asking the user, each with a short rationale;
  otherwise `None.`.
- **Testing**: report what was actually run and what happened, including failures. Don't claim a
  test passed unless you ran it in this change.
- **Pictures**: every PR with a graphical change that is best understood by seeing it (a game,
  demo, app, menu, HUD, web page, plot or other graphical tool) shows that change here, in the
  images and videos that fit that PR, so a reviewer sees it without running anything:
  - a screenshot of a still look, from the viewpoints the change matters in;
  - a video (MP4) of anything that moves, animates, changes over time or responds to input, since
    stills can't show motion; a GIF is fine for a short loop;
  - before and after, from the PR's base branch and the PR;
  - each option, by its name, when the PR leaves a choice to the user;
  - a plot labelled "Not a screenshot" when only data changes.

  Each caption says what the image or video shows (and its playback speed, if not real time). How
  to make them is in the `graphical-changes` skill. Write `None: <why>.` only when nothing anyone
  sees changes.
- **Attaching images and videos**: upload them with `--attach`, never by committing them. Attached
  videos play inline; committed ones don't.
  1. Put `body.md` in the pictures' `.artifacts/` run folder, and in its Pictures section put each
     file where it belongs as `![<caption>](./<file>)`, images and videos alike.
  2. Run, from that folder, `gh pr create --base staging/user-<username> --title "<title>"
     --body-file body.md --attach ./<image>.png --attach ./<video>.mp4` (repeat `--attach` per
     file), or `gh pr edit <n> --body-file body.md --attach ...` for an open PR or a stack layer
     after `gh stack submit`.
  3. `gh` uploads each file and replaces its reference with a `user-attachments` URL, which GitHub
     shows as an image or a video player. A file the body doesn't reference is added at the end.
  4. Check the PR in the browser, or check that `gh api repos/<owner>/<repo>/pulls/<n> -H 'Accept:
     application/vnd.github.full+json' -q .body_html` has an `<img>` or `<video>` for each file.

  `gh pr comment <n> --body-file ... --attach ...` works the same way. All three were tested on
  #372 and #373 (gh 2.103.3, 2026-11-08). Attaching needs gh 2.99.0 or later, push access and a
  token type `gh` accepts (not a GitHub App token, `ghu_`). Keep each file under 10 MB. If `gh` is
  older (seen with gh 2.96.0 on the user's Mac, 2026-11-08), ask the user to let you upgrade it
  (`brew upgrade gh`) or to drag the files into the PR in the browser.
- **Stack**: link the PR below (and above, once it exists) by number. Layer 1's base is
  `staging/user-<username>`.
- **Size**: the count of reviewable, hand-written code lines under the PR size rule in the root
  `AGENTS.md` (generated code, lockfiles, verbatim copies and prose don't count).
- **Attribution**: keep the `🤖 Generated with <agent>` line last, with `<agent>` from the table
  below for the agent that wrote the PR. If the session gives a different attribution line, use
  that one.

### Updating

When a PR changes after review, update the description so it matches the final diff (Changes,
Testing, Pictures, Size, Decisions), with `gh pr edit <n> --body-file body.md`. If that fails with a
Projects (classic) error (seen once with gh 2.55 on a Linux machine, 2026-10-30), use
`gh api -X PATCH repos/<owner>/<repo>/pulls/<n> -f body="$(cat body.md)"` instead.

## Commits

- **Every commit.** The subject is `<kind>(<area>): <summary>`, and a block of trailers at the end of the
  message gives `Kind`, `Component`, `Domain`, `Area`, `Wishlist`, `Read` and `Verify`
  ([`commit-fields.md`](commit-fields.md)). `git log` then shows what a commit changes, where, what
  to read for it and how it was verified.
- **Wishlist commits (critical).** A commit that changes a wishlist checklist (`USER_WISHLIST_CHECKLIST.md`)
  follows [`wishlist-transitions.md`](wishlist-transitions.md) exactly. Read that file before you commit. In
  short, such a commit:
  - changes one item in one checklist and nothing else, not code and not another item;
  - has the subject `wishlist(<wishlist>): <what happened to the item>`, naming the checklist's folder;
  - carries one transition line in its body, such as
    `* [<wishlist>/<id> (<short name>)] (null -> [ ]) ADD TO WISHLIST: <text>`;
  - carries the item's append-only HISTORY section below that line, written by `wishlist_history.py`
    ([`wishlist-history.md`](wishlist-history.md)).
- **Attribution.** An agent keeps the commit trailer its own default adds (table below).

## Attribution lines

Each agent puts its own name and link in the footer and keeps the lines it adds by default
(checked against each agent's defaults on 2026-11-06).

| Agent | `<agent>` | The agent's own lines, kept as they are |
| --- | --- | --- |
| Claude Code | `[Claude Code](https://claude.com/claude-code)` | Commit trailer `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, e.g. `Claude Opus 5.5` |
| Codex | `[Codex](https://openai.com/codex)` | Commit trailer `Co-authored-by: Codex <noreply@openai.com>` |
| Devin | `[Devin](https://devin.ai)` | `Link to Devin session: <url>` and `Requested by: @<user>` after the footer, where Devin appends them; commits authored by `Devin AI` (`devin-ai-integration[bot]`) |

An agent that isn't in the table adds a row for itself in the same PR.
