---
name: graphical-changes
description: Show the user screenshots, videos and clearly named options for every change to how something looks, before finishing it. Use whenever you add or change anything visual - a model, colour, shape, light, sky, effect, animation, HUD or menu, shader or render setting - in a game, demo or app; whenever a visual choice is uncertain or the user asks (or implies) a range of looks or parameters to explore; whenever you make a collage, comparison sheet or HTML comparison page; and whenever the user's words about a look allow more than one reading, or a look has edge cases to raise with the user.
---

# Graphical changes

The user judges every visual change by eye, so an agent shows it rather than describes it, and lets the user pick
the look.

## When this applies

Any change that alters what the user sees: models, colours, shapes, lighting, the sky, particles and effects,
animation and motion (of the camera, the gun, other players), the HUD and menus, and render settings that change the
picture.

A look that only shows in play (camera sway, recoil, an effect set off by a hit or a jump) also needs something to
play, and a gameplay change usually changes the look too: for those, follow the `interactive-review` skill as well,
which covers playtests, smaller three.js demos and review lanes. Pictures don't replace a demo or a playtest, nor the reverse.

## Options before building

- **Several readings: ask first.** When the user's words allow more than one reading of what they want, ask which
  they mean before rendering any options: one line per reading, with at most one quick picture each if words can't
  show the difference. Then make options only within the reading they pick. (W-69 (shot noise) was first built as a
  stray in the aim when the user meant a visual effect in the beam, W-84 (shot noise in the beam itself), and its
  "black pixels" could mean black or see-through.)
- **Raise every edge case you can find.** Before building, and again when you show the result, list for the user as
  many edge cases as you can, one line each, and ask about those whose answer changes the work. Raising too many is
  better than the user finding one in play. Look for at least:
  - **Viewpoints:** near, far, from above, edge-on, from inside, at real size and zoomed.
  - **Who sees it:** you or every player, each team, another player's window, the spectator.
  - **Motion:** standing, walking, sprinting, crouching, jumping, turning, aiming, during a hit or a respawn.
  - **Places in this game:** through a wormhole mouth, near the black hole's lens, in the 2 m walkways and doorways,
    inside a habitat module, at the map's soft edge, on the crater wall, from each spawn.
  - **Settings and limits:** each setting at 0, its default and its maximum; the lowest render scale; another map
    (the house).
  - **Time:** the first frame, a long session, many at once (shots, players, effects).
  - **Compatibility:** settings files saved before the change, older windows in the same game.
  - **Cost:** frame time, at full resolution.

- **An uncertain look gets options.** When the user's words don't settle the look (an opacity, a size, a colour, a
  style, a placement, a model), render 2 to 5 options, say which one you suggest and why, and ask which the user
  likes before building it out. Wait for the answer; work on something else meanwhile.
- **A space of options gets searched.** When the user gives a range to explore ("try opacities from 0.1 to 0.5"), or
  implies one ("make it subtler" implies several strengths; "a different rifle" implies several candidate models),
  sample that space and offer options from it: its ends and its middle, not only your favourite.
- **Options differ only in the option.** Render every option from the same viewpoints, resolution and moment, so the
  user sees the option's effect and nothing else.

## Name every option clearly

- **Name:** each option gets a letter or number, a short descriptive name and its parameter values:
  `A, faint (opacity 0.25)`, `B, stronger (opacity 0.5)`.
- **Same name everywhere:** use that name in chat, in file names, in the labels on the images, in the PR and in the
  wishlist.
- **Answerable question:** ask so that the user can answer with a name: "Which do you prefer: A, faint (opacity
  0.25), or B, stronger (opacity 0.5)?"

## Screenshots and videos before finishing

How to capture them in the Rust demo (offscreen views, temporary hooks, live games, sharing the machine with other
agents and the user's game) is in [capturing-screenshots.md](capturing-screenshots.md); read it before your first
capture.

- **Screenshots, always:** render them from the game or app itself (in `demos/hyperbolic_wormholes_rust/`,
  `--bench-views` and `--view <name>`), at the resolution the user plays at, from several viewpoints: the usual
  player's view, near, far, and the edge cases of the change (edge-on, from inside, at its limits).
- **Videos, ideally:** anything that moves or changes over time (motion, sway, an animation, a flicker, walking
  through something) also gets a short video, a GIF or MP4 of a few seconds.
- **Before and after:** show today's look (the PR's base branch) beside the change.
- **Real renders first; label anything else.** Prefer pictures the game itself rendered. A picture that isn't one (a
  plot of the data the shader draws, text rendered offscreen, a Python copy of a shader) says so on the image and in
  its caption: "Not a game screenshot: the minimap's line data, plotted".
- **Timing:** anything that happens over time also gets a strip of frames with timestamps (e.g. every 50 ms), its
  duration in seconds ("on for 0.63 s at walking speed"), and a GIF labelled with its playback speed ("shown at 1/3
  speed").
- **Real size and zoom:** show it at real size, as a player sees it, beside any close-up, and label each close-up
  with its zoom ("enlarged 7×").
- **Whose view:** when players see the change differently (your own view against another player's, one team's against
  the other's), show each view.
- **Cost:** give each option's frame time from alternating runs against the PR's base branch, and say when the machine was busy
  (other agents' runs, the user's playtest), since the numbers are then only rough.
- **Shared before finishing:** send them to the user before you call the work done, open the PR for review, mark a
  wishlist item pending or merge, and again whenever the look changes after the user's feedback.
- **In the PR:** attach the final pictures to the PR description's Pictures section (the `pr-description` skill).

## Feedback in rounds

- **Keep every round.** Keep the pictures of each round. When the user refers to earlier pictures ("like the previous
  pictures I was shown"), show the new version beside those rounds.
- **Their words:** quote the user's words from each round in the captions, the PR and the decision.
- **Change only what they asked.** Keep what the user approved ("your throat timing looks good") exactly as it was.

## Sending pictures to the user

- **One message per decision.** Send each choice's pictures in their own message, and never mix two items' options
  in one batch. Each caption starts with the item's id and short name, says which option each file is, and the
  question goes in the same message.
- **Options you can't render.** An option that can't be shown in the game (a model that needs a download or an
  account) is marked "not rendered" in its name, with a link to its page and why, apart from the rendered options.
- **Approved pictures aren't done.** The user's OK on pictures lets the work merge, but the item stays pending until
  the user tests it in the game (this demo's `AGENTS.md`).

## Give the files' paths

- **Paths with every picture:** the agent manager may not render images, so every message that shows images or
  videos also lists the absolute path of each file, one per line, with the option's name beside it.
- **Where they go:** keep them in the component's `.artifacts/<script>/<run_id>/` (root `AGENTS.md`), e.g.
  `demos/hyperbolic_wormholes_rust/.artifacts/bench_views/<run_id>/`, with the sheets and videos you assemble from
  them in the same run folder; not only in a temporary folder such as `/private/tmp`. Name the folder in the item's
  "Where the work is" when there is one.

## Collages of at most 5 × 5

- **Limit:** a collage has at most 5 images across and 5 down. More shrinks every picture and its text until the
  user can't make them out, so split a larger set into several files: `zone_walls_opacity_1_of_3.png`,
  `zone_walls_opacity_2_of_3.png`, ...
- **Detail:** when a small detail decides the choice, crop every image to the region that differs rather than
  shrinking whole frames.

## HTML pages for big comparisons

- **When:** for a complex comparison between many options (more than a couple of collages, several parameters at
  once, videos side by side), an agent may also give an HTML page.
- **What it shows:** the same labelled images and videos, with the same option names, laid out for comparing: for
  example a row per viewpoint and a column per option, buttons that flip between options at one viewpoint, or videos
  that play in step. The page shows the question too.
- **It's an artifact:** keep it in the same run folder as its pictures, refer to them by relative paths so it still
  works when the folder is copied, and load nothing from the internet.
- **One command to open it:** the page works from `file://`, so give the user one command that opens it,
  `open <absolute path of the run folder>/index.html`, beside the files' paths. The question and the option names
  still go in the chat message.
- **Interactive demos** of how a look plays are in the `interactive-review` skill.

## Label every comparison

- **Labels:** every collage that compares options or parameters (such as the speckle strengths of W-84 (shot noise
  in the beam itself)) carries text that says what each image is: a label on each image, or a header on each row and
  each column naming the parameter and its value (columns `speckle 0`, `speckle 0.1`, `speckle 0.2`; rows `near`,
  `far`).
- **Legible:** the text is large enough to read at the size the sheet is shown, light on a dark band outside the
  picture, so it hides nothing.
- **File names:** a file's name says what it shows too: `zone_walls_a_faint_0.25_vs_b_stronger_0.5.png`.

## Tools

- The game writes PNGs; assemble sheets and GIFs with Pillow, run as `python3 -I <script>` on a script in your
  worktree or scratchpad, and label them with Pillow's `ImageFont.load_default()`. Don't download packages or fonts
  (`uv run --with`, `pip install`) without asking the user first, and show the user any Python that reads outside
  your worktree before running it.
- If ImageMagick or ffmpeg is missing (seen on the user's Mac, 2026-11-08), use Pillow for sheets and GIFs, and ask
  the user before installing anything.
