---
name: pr-description
description: Write or update a pull request description using this repo's standard template, and open every PR against the user's `staging/user-<username>` branch (agents merge into `staging/global` or `main` only with explicit permission). Use whenever you open a PR (gh pr create, gh stack submit), edit a PR's body, the user asks for a PR description, title or summary, or you attach screenshots or videos to a PR. Every PR in this repo uses the same template.
---

# PR description

Every PR description in this repo uses the template below, exactly: the same sections, in the same
order, with the same headings. Never drop a section; if it has nothing to say, write `None.`. This
covers only the description (and title); the PR size rule is in the root `AGENTS.md`, and how to
split and stack PRs is in the `stacked-prs` skill (`.agents/skills/stacked-prs/`).

> NOTE: The template is a living document: it may change, but at any point in time every PR uses
> the current version. If a PR doesn't fit it (a section makes no sense, or something important
> has no place), stop and ask the user for guidance rather than improvising a variant. If the
> template needs to change, change it in this skill in its own PR.

## Base branch

- Open every PR against your user's `staging/user-<username>`, never straight against
  `staging/global` or `main`. In a stack, layer 1's base is `staging/user-<username>`.
- Agents may merge PRs into their user's `staging/user-<username>` themselves, once the PR's own
  tests pass.
- An agent merges into `staging/global` or `main` only after the user explicitly says yes to that
  merge. How the branches fit together is in the `merging-to-staging` skill.

## Template

```markdown
## Summary

<1-3 sentences: what the PR changes and why.>

## Changes

- <one bullet per concrete change, naming the files or folders touched>

## Decisions

<None.> or <the decisions you made rather than the user, each with a short rationale>

## Testing

- <each command run and its result, e.g. `make test`: 12 tests pass>
- <or: Not run: <reason>>

## Pictures

<None: <why nothing visible changes>.> or <screenshots, videos or plots of the change, each with a caption>

## Stack

<Not stacked.> or <Layer n/m. Based on #<PR below>; #<PR above> builds on this.>

## Size

Reviewable lines: <N> (<what they are, e.g. "Markdown only" or "src/ + tests/">)

🤖 Generated with <agent>
```

## Filling it in

- **Title**: short and specific, `<folder or area>: <what changes>` (e.g.
  `physics: add curved-geodesic movement`). For a stack layer, no "1/2" numbering in the
  title; the Stack section carries it.
- **Summary**: the why matters more than the what; the diff already shows the what.
- **Changes**: concrete and complete, but no line-by-line narration of the diff.
- **Decisions**: the choices you made without asking the user, each with a short rationale;
  otherwise `None.`.
- **Testing**: report what was actually run and what happened, including failures. Don't claim a
  test passed unless you ran it in this change.
- **Pictures**: every feature PR that changes what someone sees (a game, demo, app, menu, HUD, web
  page, plot or other graphical tool) shows the change here, in the pictures that fit that PR:
  - a screenshot of a still look, from the viewpoints the change matters in;
  - a video or GIF of anything that moves, animates or responds to input;
  - before and after, from the PR's base branch and the PR;
  - each option, by its name, when the PR leaves a choice to the user;
  - a plot labelled "Not a screenshot" when only data changes.

  Each caption says what the picture shows. How to make them is in the `graphical-changes` skill.
  Write `None: <why>.` only when nothing anyone sees changes.
- **Attaching pictures**: upload them with `gh pr create` or `gh pr edit` and `--attach <file>`
  (gh 2.99.0 or later), never by committing them. Reference each file in the body as
  `![<caption>](./<file>)`, videos too, relative to the body file's folder; `gh` replaces it with
  the uploaded file's URL, which GitHub shows as an image or a video player. Keep `body.md` in the
  pictures' `.artifacts/` run folder. Attaching needs push access and a token type `gh` accepts
  (not a GitHub App token, `ghu_`). Keep each file under 10 MB. If `gh` is older than 2.99.0 (seen with gh 2.96.0 on the user's Mac, 2026-11-08), ask the
  user to upgrade it (`brew upgrade gh`) or to drag the files into the PR in the browser.
- **Stack**: link the PR below (and above, once it exists) by number. Layer 1's base is
  `staging/user-<username>`.
- **Size**: the count of reviewable, hand-written code lines under the PR size rule in the root
  `AGENTS.md` (generated code, lockfiles, verbatim copies and prose don't count).
- **Attribution**: keep the `🤖 Generated with <agent>` line last, with `<agent>` from the table
  below for the agent that wrote the PR. If the session gives a different attribution line, use
  that one.

## Attribution lines

Each agent puts its own name and link in the footer and keeps the lines it adds by default
(checked against each agent's defaults on 2026-11-06).

| Agent | `<agent>` | The agent's own lines, kept as they are |
| --- | --- | --- |
| Claude Code | `[Claude Code](https://claude.com/claude-code)` | Commit trailer `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, e.g. `Claude Opus 5.5` |
| Codex | `[Codex](https://openai.com/codex)` | Commit trailer `Co-authored-by: Codex <noreply@openai.com>` |
| Devin | `[Devin](https://devin.ai)` | `Link to Devin session: <url>` and `Requested by: @<user>` after the footer, where Devin appends them; commits authored by `Devin AI` (`devin-ai-integration[bot]`) |

An agent that isn't in the table adds a row for itself in the same PR.

## Updating

When a PR changes after review, update the description so it matches the final diff (Changes,
Testing, Pictures, Size, Decisions), with `gh pr edit <n> --body-file body.md`. If that fails with a
Projects (classic) error (seen once with gh 2.55 on a Linux machine, 2026-10-30), use
`gh api -X PATCH repos/<owner>/<repo>/pulls/<n> -f body="$(cat body.md)"` instead.
