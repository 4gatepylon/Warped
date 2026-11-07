---
name: pr-description
description: Write or update a pull request description using this repo's standard template. Use whenever you open a PR (gh pr create, gh stack submit), edit a PR's body, or the user asks for a PR description, title or summary. Every PR in this repo uses the same template.
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

## Stack

<Not stacked.> or <Layer n/m. Based on #<PR below>; #<PR above> builds on this.>

## Size

Reviewable lines: <N> (<what they are, e.g. "Markdown only" or "src/ + tests/">)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
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
- **Stack**: link the PR below (and above, once it exists) by number. Layer 1's base is `main`.
- **Size**: the count of reviewable, hand-written code lines under the PR size rule in the root
  `AGENTS.md` (generated code, lockfiles, verbatim copies and prose don't count).
- Keep the attribution line last. If the session gives a different attribution line, use that one.

## Updating

When a PR changes after review, update the description so it matches the final diff (Changes,
Testing, Size, Decisions), with `gh pr edit <n> --body-file body.md`. If that fails with a
Projects (classic) error (seen once with gh 2.55 on a Linux machine, 2026-10-30), use
`gh api -X PATCH repos/<owner>/<repo>/pulls/<n> -f body="$(cat body.md)"` instead.
