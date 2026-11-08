---
name: sharing-gameplay
description: Give the user something to play for every gameplay change - anything that can only really be judged interactively, such as controls, movement and physics feel, weapons and hits, timing, rules and how screens flow - in the game itself or in a smaller, simpler three.js demo. Use whenever you add or change how the game plays or responds to input, options of a mechanic need comparing in play, or you build an interactive demo for the user to try.
---

# Sharing gameplay

Gameplay is anything the user can only fully judge by playing it: controls, movement and physics feel, weapons and
hits, timing, rules, respawns, and how menus and screens flow. Screenshots and videos show it only in part, so an
agent gives the user something to play. This skill is a start and will grow.

## Something to play

- **The game first.** When the change runs in the game, hand it to the user to playtest there (the `playtesting`
  skill), with what to try.
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

## When it also changes the look

Most gameplay changes show on screen too (a new weapon has a model and a beam; a jump moves the camera), and many
graphical changes only show in play (camera sway, recoil, an effect set off by a hit). When a change does both,
follow the `graphical-changes` skill too: screenshots, videos and named options for the look, and a demo or a
playtest for the feel. Neither replaces the other.
