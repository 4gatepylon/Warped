---
name: review-lanes
description: Run several features for the user to test at once, one review lane per feature - an agent, a worktree, a block of ports and its own clearly labelled game server and windows (or browser page) that the agent keeps up to date as it works - so the user can switch between lanes and play each one whenever they like. Use whenever two or more parallel features or fixes need the user to test them interactively, you start agents to build such features, or the user asks to see, switch between, pause or stop features under way.
---

# Review lanes

The user reviews many features at once by playing them, so every feature under way runs where the user can switch to
it at any moment, labelled so it can't be mistaken for another, and kept current by its agent. Each such feature is a
lane. This skill builds on `managing-agents`, `devin-worktrees`, `sharing-gameplay` and `playtesting`.

## A lane

- **One feature, one agent:** each lane is one wishlist item or task, built by one agent (a subagent, or a session
  of its own) in a worktree of its own (`devin-worktrees`).
- **A name the user can say:** `L<n>` and the item's short name, e.g. `L2 W-91 (double jump)`. `<n>` counts up from 1
  for the parent's lanes and is never reused while the parent runs, so "L2" always means the same feature.
- **Its own ports:** lane `<n>` uses the block `48000 + 10 × <n>` to `48009 + 10 × <n>`; its game server listens on the
  first. No lane uses the user's own ports (47800, and 47801 for finding servers).
- **Everything labelled:** every window title starts with `[L<n> <short name>]`, the HUD shows the same label in a
  corner (a full-screen window has no title bar), a browser page's `<title>` and heading start with it, and every
  terminal, log line and file name of the lane carries `l<n>`.

## Running a lane

- **Always up:** once the lane has something to try, its game server and one window run until the lane ends:
  `--serve --port <first port>` and `--join 127.0.0.1:<first port>`, never `--join auto`, which could find another
  lane's server or the user's game. A browser lane serves its page on its first port.
- **Hot swap:** after each step that builds and passes its tests, the lane's agent swaps the new build in: it starts
  the new server and window, then stops the old ones, so the lane is never down for long. It doesn't swap a lane the
  user has put on hold (below), and it says in the lane table which build is running.
- **Data before code:** a change that is only settings (`server_settings.yaml`, a map file) is loaded by a server
  restart alone, without rebuilding or closing the window, where the game allows it.
- **Builds stay in the lane:** each worktree builds into its own `.build/`, so lanes never overwrite each other's
  binaries. The first build of a worktree takes minutes; start it as soon as the lane starts.
- **Quiet in the background:** a window the user isn't looking at draws as little as it can, so the user's lane and
  the user's own game keep their frame rate.
- **Who owns the processes:** the parent starts and stops every lane's processes in background shells of its own,
  because a subagent's shells end with its task. A lane's agent tells the parent when a new build is ready to swap.

## The user's view

- **The lane table.** The parent keeps one table and shows it whenever a lane changes: lane, item, state
  (`building`, `ready to try`, `waiting for you`, `on hold`), build running (commit and time), what to try, and one
  command that brings the lane's window back if the user closed it.
- **Switching:** the user switches lanes with the window switcher, by title. The parent says which lanes changed
  since the user last looked.
- **Hold:** the user can put a lane on hold ("hold L2"), and the parent stops swapping builds into it until the user
  releases it, so a window never changes under the user mid-test.
- **Feedback:** the user names the lane ("L2: jumps too floaty"); the parent passes the user's words on to that lane's
  agent (`managing-agents`, Relaying). Pictures and pages for a lane go through the parent, labelled with the lane.
- **How many:** run at most as many lanes as the user agrees, since every lane's window uses the GPU.

## Ending a lane

- **When:** the lane's PR merges, the user drops the feature, or the user asks.
- **Stop everything:** the parent stops the lane's server and windows, checks that no process of the lane is left
  (`pgrep -fl` on the binary and the lane's ports), removes it from the table and frees its ports.
- **Cleanup:** the worktree is cleaned up as `devin-worktrees` says.

## What the game needs for lanes

These aren't in the Rust demo yet (seen 2026-11-08); until they are, lanes use what exists and say what's missing:

- a `--window-label <text>` flag that prefixes the window title (today the title is `WINDOW_TITLE: <HUD lines>`) and
  shows the label on the HUD;
- a server flag that stops it announcing itself on UDP 47801, so lanes don't show up when the user's own windows look
  for a server;
- drawing less while the window is in the background;
- a lane runner (a Makefile target or script) that builds, then swaps the server and window with one command.
