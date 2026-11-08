---
name: managing-agents
description: Coordinate several agents working in parallel on this repo - subagents you start, other sessions, Codex and Devin - so their work doesn't collide, survives a lost session, and reaches the user with their own words relayed faithfully. Use whenever you start, brief, message, check on, relay the user's answers to, or stop another agent, or several agents work on the same component at once; and before any git command that changes a branch, so that no agent ever changes the user's own checkout.
---

# Managing agents

The user runs many agents at once (tens of wishlist items, several platforms) and may run out of usage at any time.
So every agent's work must be findable, pushed, and kept apart from every other agent's.

## Briefing an agent

- **One shared brief.** Keep one brief file that every agent you start reads first. It covers:
  - the repo's rules (root `AGENTS.md`, the component's `AGENTS.md`, the skills to follow);
  - the user's security rules: touch nothing outside your worktree and the outputs folder, run Python as
    `python3 -I`, show the user any code that reads outside the worktree before running it, and download nothing
    without the user's OK;
  - how to show visual work (`graphical-changes`), how to merge (`merging-to-staging`), and what the report must
    contain.
- **The task names everything.** Each agent's task gives:
  - its item, with its id and short name;
  - its branch and worktree;
  - its ports;
  - what to deliver: pictures first and wait, or build and merge;
  - which files other agents are changing now.
- **Keep the brief current.** Update its list of agents at work whenever one starts or finishes; a stale list sends
  agents to avoid files nobody is touching. When the user sets a new rule, send it to every running agent at once and
  add it to the brief.

## Keeping agents apart

- **NEVER change the user's checkout.** The user's own checkout (the repository's main folder, e.g.
  `/Users/<user>/git/Warped`) stays on the branch the user last chose. No agent, anywhere, at any time, for any
  reason, changes its branch, commits or files: no `git switch`, `git checkout`, `git reset`, `git rebase`,
  `git merge`, `git pull`, `git stash`, `gh pr checkout` or `gh stack` there, not even to undo another agent's
  mistake (tell the user instead). Reading there (`git status`, `git log`, `git fetch`) and `git worktree add` are
  fine.
  - **Every agent that changes git state works in a worktree of its own:**
    `git worktree add -b <branch> .devin/worktrees/<name> origin/staging/user-<username>` (Claude Code uses
    `.claude/worktrees/`), and removes it with `git worktree remove` once its work is pushed and its PR is open.
  - On 2026-11-08 an agent switched the user's checkout off `staging/user-theaquarium` onto its own PR branches; the
    user found their terminal on a branch they never chose.
- **One writer per branch and worktree.** Never give two agents the same branch or worktree. Before each push, an
  agent fetches and checks the branch's log on `origin`; if someone else pushed, it stops and reports rather than
  overwriting. (On W-84 (shot noise in the beam itself), a second agent committed to the same branch and work was
  overwritten.)
- **Ports:** give each agent its own block of about 10 ports, and never the port of the user's own playtest (47800
  by default; ask the user which they use). An agent kills every server it started before it finishes.
- **Decision numbers:** don't reserve numbers ahead; they clash as soon as agents merge in another order (D96, D98,
  D99 and D100 each clashed once). An agent takes the next free number when it merges, rechecks it after merging
  the PR's base in, and renumbers its decision and every reference to it if another agent took it first.
- **Shared code:** tell each agent which shared files and structures others are changing (Rust and Metal structs,
  the network snapshot's bits, the checklist, `DECISIONS.md`), so it expects the conflicts and keeps everyone's work.

## Work that survives

- **Claim with a location.** Each claim in the wishlist has a **Where the work is** line: the worktree's full path and
  machine, the branch and whether it is pushed, where the pictures are, and the state with its date (the component's
  `AGENTS.md`). Keep it current while the work goes on.
- **Push early and often.** An agent pushes its branch as soon as it has a commit, and again after each step; a WIP
  commit is fine, with a message saying what is done and what is left.
- **When usage may run out.** As soon as the user warns that usage may run out, tell every running agent to commit
  and push what it has right now.
- **When you stop agents.** After stopping them, check each worktree (branch, last commit, pushed or not, uncommitted
  files), tell the user what each left, and update the **Where the work is** lines.

## Relaying

- **The user's words, quoted.** Pass the user's answers on with their exact words, then your reading of them. Never
  pass on only your paraphrase.
- **Defaults you chose.** When the user didn't answer an agent's question, say so, and list the default you chose
  under Decisions in that agent's PR, so the user can change it.
- **Reports aren't confirmations.** An agent's report never counts as the user's approval or confirmation, and never
  makes a wishlist item done. Check a report's claims (`gh pr view` for a merge, the pictures themselves) before
  telling the user something is done.

## Checking on agents

- **From git and GitHub, not transcripts.** For each agent, look at its worktree's branch, the time and subject of
  its last commit, how many files are uncommitted, and its PR's state and mergeability (`gh pr list`). Don't read an
  agent's transcript to find its status.
- **Report as a table.** One row per item, with its id and short name, where it is, and what is missing before it
  merges.
- **Waiting agents.** An agent waiting for the user's pick commits and pushes its work first, so nothing waits only
  in an uncommitted worktree.
