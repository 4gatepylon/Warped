# The scene window graph (SWG)

The **scene window graph**, the **SWG** (pronounced "swig"), is a conceptual model of everything the player sees
that is not part of the 3D world, as a tree like the DOM of a web page. Its purpose is clear terms and a shared
mental picture, so that a visual is easy to explain to others and, later, easy to build in a smarter way.

> NOTE: This file is terminology and a mental model only. There is no SWG file and no SWG code, on purpose. Later
> we may define SWGs more formally, with an SWG language, YAML or another format. Until the user asks for that,
> don't create one.

[`GLOSSARY.md`](GLOSSARY.md) defines the game's other words. Use the terms of both files exactly.

## The model

1. **What it holds.** Everything that is visualized and is not part of the 3D world is part of an SWG.
2. **A tree.** An SWG is a tree of nodes, as a web page's DOM is.
3. **Only leaves are rendered.** To *render* a node, as a verb, only means to show it on the screen. Leaf nodes
   are rendered, and every other node groups its children.
4. **A leaf is a render or a UIE.** Always one of the two:
   - A **render**, as a noun, is where the SWG stops and a "pure" graphical thing starts. The **scene render** is
     the render for the scene of the game: the rasterized representation of the match at the time of play, as
     you are playing it. The **minimap render** is the render of the minimap.
   - A **UIE** (UI element, pronounced "ooh-wee") is a piece of the interface itself, such as a bar.
5. **Offsets.** Conceptually, each leaf has an offset from its parent along the `z` axis (depth) as well as the `x`
   and `y` axes. They could be relative or absolute coordinates, as in CSS, but we don't handle that yet.
6. **Shared depth, by design.** Nodes can have any such offsets, but it is almost always recommended, and designed,
   that the UIEs and renders of a group share a similar `z`: the HUD's leaves all have the same `z`, and the HUD
   is on top of the scene render.
7. **Hidden parts.** A part of an SWG can be hidden until something shows it. The main map is a hidden part of the
   game/match SWG.
8. **Different windows are different SWGs.** A menu and the game/match are different SWGs. Exiting the game/match
   to the game menu leaves one SWG for another.

## The regular game, as an example

During play, the regular game's SWG has the HUD and the scene render. The HUD has the minimap render and each of
the bars:

```text
game/match SWG
├── scene render          render
├── HUD                   its leaves share one z, on top of the scene render
│   ├── minimap render    render
│   ├── health bar        UIE
│   ├── energy bar        UIE
│   └── ammo bar          UIE
└── main map              hidden
```

The sketch shows the model; it is not a full list of what the game draws. The 3D world itself (robots, lasers,
pylons, wormholes and the rest) is not in the SWG: the scene render is the picture of it.

## Questions for the player

1. **The word window.** The glossary used *window* for the operating-system window, and you also call a menu or the
   game/match a window, each with its own SWG. Keep both meanings, and say *OS window* for the first? Proposed: yes.
2. **A node that isn't a leaf.** Is *group* the right word for a node such as the HUD, which only groups its
   children?
3. **Other leaves.** When we list more of the game/match SWG, are the crosshair and the console bar UIEs of the
   HUD, and is the main map's picture a render? Proposed: yes to both.
