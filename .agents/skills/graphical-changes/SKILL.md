---
name: graphical-changes
description: Show the user screenshots, videos and clearly named options for every change to how something looks, before finishing it. Use whenever you add or change anything visual - a model, colour, shape, light, sky, effect, animation, HUD or menu, shader or render setting - in a game, demo or app; whenever a visual choice is uncertain or the user asks (or implies) a range of looks or parameters to explore; and whenever you make a collage or comparison sheet.
---

# Graphical changes

The user judges every visual change by eye, so an agent shows it rather than describes it, and lets the user pick
the look.

## When this applies

Any change that alters what the user sees: models, colours, shapes, lighting, the sky, particles and effects,
animation and motion (of the camera, the gun, other players), the HUD and menus, and render settings that change the
picture.

## Options before building

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

- **Screenshots, always:** render them from the game or app itself (in `demos/hyperbolic_wormholes_rust/`,
  `--bench-views` and `--view <name>`), at the resolution the user plays at, from several viewpoints: the usual
  player's view, near, far, and the edge cases of the change (edge-on, from inside, at its limits).
- **Videos, ideally:** anything that moves or changes over time (motion, sway, an animation, a flicker, walking
  through something) also gets a short video, a GIF or MP4 of a few seconds.
- **Before and after:** show today's look (`staging`) beside the change.
- **Shared before finishing:** send them to the user before you call the work done, open the PR for review, mark a
  wishlist item pending or merge, and again whenever the look changes after the user's feedback.

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

## Label every comparison

- **Labels:** every collage that compares options or parameters (such as the speckle strengths of W-84 (shot noise
  in the beam itself)) carries text that says what each image is: a label on each image, or a header on each row and
  each column naming the parameter and its value (columns `speckle 0`, `speckle 0.1`, `speckle 0.2`; rows `near`,
  `far`).
- **Legible:** the text is large enough to read at the size the sheet is shown, light on a dark band outside the
  picture, so it hides nothing.
- **File names:** a file's name says what it shows too: `zone_walls_a_faint_0.25_vs_b_stronger_0.5.png`.

## Tools

- The game writes PNGs; assemble sheets and GIFs with Pillow: `uv run --with pillow python <script>`.
- If ImageMagick or ffmpeg is missing (seen on the user's Mac, 2026-11-08), use Pillow for sheets and GIFs, and ask
  the user before installing anything.
