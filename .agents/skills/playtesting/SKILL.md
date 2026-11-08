---
name: playtesting
description: Hand a game, demo or other visual work to the user to playtest - how to give them commands and how to set up a way to test it yourself. Use whenever the user will test or try something visual you made (a game, a demo page, a desktop or iOS app), or you give them commands to run it.
---

# Playtesting

When the user tests a game or other visual work you made:

- **Commands:** give commands that enter a folder with `pushd`, not `cd`, so the user can get back
  to where they were with `popd`.
- **No screen sharing:** the user doesn't share their screen or microphone, so ask them to playtest
  desktop apps and anything else you can't see yourself. Where you can, work with them to set up an
  environment you can test in yourself, such as a browser page or a simulator.
- **Gameplay changes:** what to give the user to play, including smaller three.js demos, is in the
  `sharing-gameplay` skill.
