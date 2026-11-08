---
name: interactive-review
description: Let the user review by playing - give them something to play for every gameplay change (anything that can only really be judged interactively, such as controls, movement and physics feel, weapons and hits, timing, rules, how screens flow, and looks that only show in play), in the game itself or in a smaller, simpler three.js demo; and when several features are in development at once, run each in its own clearly labelled, always-up review lane the user can switch to. Use whenever you add or change how the game plays or responds to input, options of a mechanic need comparing in play, you build an interactive demo, two or more features in development need the user to test them, or the user asks to see, switch between, hold or stop features under way.
---

# Interactive review

Gameplay is anything the user can only fully judge by playing it: controls, movement and physics feel, weapons and
hits, timing, rules, respawns, and how menus and screens flow. Screenshots and videos show it only in part, so an
agent gives the user something to play. When several features are in development at once, each runs in its own
review lane, so the user can review them all in parallel. This skill is a start and will grow.

## Something to play

- **The game first.** When the change runs in the game, hand it to the user to playtest there (the `playtesting`
  skill), with what to try. With several features in development at once, each runs in a lane (Review lanes, below).
- **A smaller demo when that helps.** An agent may also demo a mechanic interactively with three.js in a smaller or
  simpler setting: a flat floor or one room, simple shapes, one player against a dummy. Use one when the game can't
  show the mechanic yet, when building it in the game first would cost a lot, or to compare options quickly.
- **Label it.** A demo says on screen and in its caption that it isn't the game ("Not the game: three.js demo of
  the jump options"), and what it leaves out (curved space, the real map, other players, the network).
- **Options in one page.** When options are compared, the page switches between them with keys or buttons, names
  each as `graphical-changes` does (`A, short hop (jump 4 m/s)`), and shows the name of the option in play.
- **What to try.** List what the user should try, one line each: the moves, places and edge cases that decide the
  choice (`graphical-changes` lists edge cases to look for).
- **A demo isn't the game.** The user's OK on a demo settles the choice; the work is done only once the user has
  played it in the game.

## Building and sharing a demo

- **three.js:** load it the way the demos in `demos/levels/` do (three.js 0.160.0 from a CDN), and download nothing
  else without the user's OK.
- **Where it goes:** a throwaway demo is a run output, in the component's `.artifacts/<script>/<run_id>/` (root
  `AGENTS.md`). A demo worth keeping goes under `demos/` with a README, only if the user agrees.
- **One command to open it:** give the user one command that opens the demo, usually `open <absolute path>/index.html`,
  so keep a demo to one HTML file that works from `file://` where you can. A demo that needs a server (it loads its
  own JavaScript modules) gets one command that starts it on a port of your own (not the user's playtest port) and
  opens it: `python3 -m http.server <port> --directory <folder> & open http://localhost:<port>/index.html`. Stop that
  server once the user is done.
- **Check it first:** before handing it over, check that the page loads without console errors and that every key
  and option does what it says.
- **Rounds:** keep each round's demo, and quote the user's words from each round, as `graphical-changes` does for
  pictures.

## Review lanes

When several features are in development at once, every one runs where the user can switch to it at any moment,
labelled so it can't be mistaken for another, and kept current while its subagent works. Each is a lane.

### A lane

- **One lane, one subagent, one feature.** A lane is one feature in development (a wishlist item or task), built by
  one subagent in a worktree of its own (`devin-worktrees`), unless the user asks for something else.
- **How many:** the user decides how many lanes run at once.
- **A name the user can say:** `L<n>` and the item's short name, e.g. `L2 W-91 (double jump)`. `<n>` counts up from 1
  for the parent's lanes and is never reused while the parent runs, so "L2" always means the same feature.
- **Its own ports:** lane `<n>` uses the block `48000 + 10 × <n>` to `48009 + 10 × <n>`; its game server listens on the
  first. No lane uses the user's own ports (47800, and 47801 for finding servers).
- **Everything labelled:** every window title starts with `[L<n> <short name>]`, the HUD shows the same label in a
  corner (a full-screen window has no title bar), a browser page's `<title>` and heading start with it, and every
  terminal, log line and file name of the lane carries `l<n>`.

### Running a lane

- **Always up:** once the lane has something to try, its game server and one window run until the lane ends:
  `--serve --port <first port>` and `--join 127.0.0.1:<first port>`, never `--join auto`, which could find another
  lane's server or the user's game. A browser lane serves its page on its first port.
- **Separate builds:** each lane builds into its own worktree's `.build/`, so lanes never overwrite each other's
  binaries. The first build of a worktree takes minutes; start it as soon as the lane starts.
- **Swapping:** swapping replaces the build a lane runs with a newer one. After each step of the subagent's that
  builds and passes its tests, the parent starts a new server and window from the new build, on the lane's ports and
  with its label, then closes the old ones. The lane is down only for those moments, and the next time the user
  switches to it, they play the latest build. The game can't load new code into a running window, so a swap restarts
  both. A change that is only settings (`server_settings.yaml`, a map file) needs only a server restart, where the
  game allows it.
- **Hold:** the user can put a lane on hold ("hold L2"), and the parent swaps nothing into it until the user releases
  it, so a window never changes under the user mid-test.
- **Quiet in the background:** a window the user isn't looking at draws as little as it can, so the user's lane and
  the user's own game keep their frame rate.
- **Who runs the processes:** the parent starts, swaps and stops every lane's server and window in background shells
  of its own, because a subagent's shells end with its task (`managing-agents`). The subagent tells the parent when a
  new build is ready.

### The user's view

- **The lane table.** The parent keeps one table and shows it whenever a lane changes: lane, item, state
  (`building`, `ready to try`, `waiting for you`, `on hold`), build running (commit and time), what to try, and one
  command that brings the lane's window back if the user closed it.
- **Switching:** the user switches lanes with the window switcher, by title. The parent says which lanes changed
  since the user last looked.
- **Feedback:** the user names the lane ("L2: jumps too floaty"); the parent passes the user's words on to that lane's
  subagent (`managing-agents`, Relaying). Pictures and pages for a lane go through the parent, labelled with the lane.

### Ending a lane

- **When:** the lane's PR merges, the user drops the feature, or the user asks.
- **Stop everything:** the parent stops the lane's server and window, checks that no process of the lane is left
  (`pgrep -fl` on the binary and the lane's ports), removes it from the table and frees its ports.
- **Cleanup:** the worktree is cleaned up as `devin-worktrees` says.

### What the game needs for lanes

These aren't in the Rust demo yet (seen 2026-11-08); until they are, lanes use what exists and say what's missing:

- a `--window-label <text>` flag that prefixes the window title (today the title is `WINDOW_TITLE: <HUD lines>`) and
  shows the label on the HUD;
- a server flag that stops it announcing itself on UDP 47801, so lanes don't show up when the user's own windows look
  for a server;
- drawing less while the window is in the background;
- a lane runner (a Makefile target or script) that builds, then swaps the server and window with one command.

## When it also changes the look

Most gameplay changes show on screen too (a new weapon has a model and a beam; a jump moves the camera), and many
graphical changes only show in play (camera sway, recoil, an effect set off by a hit). When a change does both,
follow the `graphical-changes` skill too: screenshots, videos and named options for the look, and a demo, a playtest
or a lane for the feel. Neither replaces the other.
