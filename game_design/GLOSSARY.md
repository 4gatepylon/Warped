# Glossary

The official words of this project, one meaning each. Read "How to use it" first; the sections after it hold the
terms.

## How to use it

- **Official terms, always.** Every agent MUST use these terms exactly, in code, docs, commits and PRs.
- **The user may speak loosely.** The user may use a slightly different word for a thing. The third column of each
  table, "Might be called by the user", lists the ones we know. When the meaning is obvious from the context, read
  the user's word as the official term. When it isn't, ask the user a precise question: name the word, quote the
  sentence it came up in, give the meanings you see, and say which you would pick. Never write the loose word
  yourself.
- **New terms.** A term this file lacks needs the user's agreement. Ask, then add its row in the same change. Never
  invent or guess a term.
- **One meaning.** If two terms here seem to fit, or one term seems to mean two things, that is a mistake in this
  file. Ask the user, and fix it.

## Sections

- [How to use it](#how-to-use-it)
- [Structure of play](#structure-of-play)
- [People and entities](#people-and-entities)
- [Programs and places](#programs-and-places)
- [Pages and SWGs on the window](#pages-and-swgs-on-the-window)
- [World objects](#world-objects)
- [Worlds](#worlds)
- [Moving and looking](#moving-and-looking)
- [Space and physics](#space-and-physics)
- [Building, measuring and shipping](#building-measuring-and-shipping)
- [Tracking work](#tracking-work)
- [Words we retired on 20261110](#words-we-retired-on-20261110)

## Structure of play

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **game** | A game that exists but hasn't started: a game server hosts it, it has game settings and a choice of what to play (the whole map, or one level), and players join it and pick teams. | |
| **match** | A game that has started and is ongoing: two teams play across the levels of a map until a team wins it or it ties. Once it ends it is a *finished match*. | "game" |
| **free play** | A game or match on one level instead of the whole map. | |
| **round** | One play of one level during a match, which one team wins. | |
| **level** | One playable space: Dust, Snow Planet, City Sphere, Dark Forest or Lunar Surface. Each level has its own world. | "map", "demo" |
| **world** | A level's 3D game world: the place where its actual objects are, such as the ground, structures, pylons, warp objects and the sky. | "game world" |
| **map** | The line of levels a match is played across: the blue levels, one purple level in the middle, then the red levels. The default map has five levels. | |
| **level layout** | The data that says where one level's world has its ground, structures, spawns, pylons, warp objects, lights and sky, in the portable format of [`common/maps/`](../common/maps/README.md). | "map", "map file" |
| **map layout** | Several level layouts, plus the overall rules that join them into a map, such as which level leads to which. | |
| **depth** | How far a level is from the purple level: 0 for purple, then 1, 2, ... outward. | |
| **team** | Blue or red, about 5 players each. The purple level belongs to neither. | |
| **wavefront**, **attacker**, **defender** | A team's next level to win on the other team's side; the team whose wavefront is being played; the team that owns the level being played. Not built yet. | |
| **spawn** | Where a team's players enter a level. | |
| **zone** | One region of a level's world: the blue zone, the middle zone or the red zone. City Sphere has only a blue zone and a red zone. | |
| **boundary plane** | The faint see-through wall in a team's colour at the edge of its zone, which stops neither players nor lasers. In City Sphere it is a dome over each team's pylon. | "zone plane", "zone wall" |
| **route**, **floor** | One way across the middle zone; one walkable layer inside a level. | |
| **level edge** | The soft edge of a level, where players slow down without end as they climb, so they never leave. | "map edge" |
| **instance** | One particular copy of something that is defined once. Each hesco wall in a world is an instance of the hesco block structure, and a level being played in a round is an instance of that level. | |

## People and entities

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **entity** | A thing in a world that acts, for a person or by itself: a player or a bot. Every entity has a sprite. | |
| **player** | The entity of a person who is playing. It has the bot sprite. | |
| **bot** | The entity the computer controls, on your team or the other. | "robot" |
| **sprite** | The visuals of an entity or an object: its shape and its **skin**, the colours and textures on it. | "skin" |
| **bot sprite** | The robot-shaped sprite that every player and every bot has, in its team's colour. | "robot", "robot sprite", "bot skin", "robot skin" |
| **game master** | The person who chooses what everyone plays and changes the game settings. | |
| **operator** | The person who runs a game server. | |
| **elimination** | A player or bot is hit enough to be out of health. It counts as a *death* for them and, when an enemy did it, a *kill* for the shooter. | "died", "killed" |
| **respawn** | An eliminated player's or bot's return, after the respawn time, on a ring around their team's spawn. | |
| **health** | What a hit takes away. | |
| **energy** | What sprinting uses up and time refills. | |
| **ammo** | What a gun uses when it fires. The sniper rifle holds one shot and reloads after it; the rifle heats up as it fires and cools down afterwards. Ammo never runs out for good. | |

## Programs and places

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **the app** | The official game as one program with one window (`warped`, or `Warped.app` on a Mac). | Often called the `client` as well (as in server/client). |
| **window** | The operating-system window of the app. The official game has exactly one; older demos opened one per level and one for the match. The window is made up of pages (see [Pages and SWGs on the window](#pages-and-swgs-on-the-window)). | |
| **game server** | The headless program that hosts games and matches, and that players join. In the official game it is the program of `demos/warped_match_rust/`. | "match server" or "server" in the context of interactions in the world when using multiplayer. |
| **level server** | The headless program a game server starts to run one round's level: its players, bots, shots and consoles. This may not exist in the future | |
| **starter game** | The old 2D square game with its own `clients/starter/`, `game_server/` and `management_server/`. Historical, to be replaced soon. | "the 2D game" |
| **management server** | The starter game's program that starts game servers and tracks who may join them. A historical version exists, and a new version may be built in the future. | |
| **cloud** | Game servers on Google Cloud, with a download page ([`infra/`](../infra/README.md)). | |
| **demo** | A program built to test an idea. Older commits call each level and the match "demos". | |
| **library** | A Rust crate that levels share, such as `warped_shared`. | |

> NOTE: if any of this seems outdated, ask the user.

## Pages and SWGs on the window

Everything the player sees is on the window. The window shows **pages**, and what is on one page is that page's
**SWG**. This section has the terms and the SWG model.

### Terms

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **page** | Like a web page: one whole thing the window shows, such as the level page or a menu. Every menu is a page. A **sub-page** is a page inside another, such as the level menu's SCORES. | "screen" |
| **scene window graph (SWG)** | Pronounced "swig". A conceptual model of what is on a page: everything the player sees that is not part of the 3D world, as a tree like a web page's DOM. | |
| **render** | As a noun: a leaf of an SWG where the SWG stops and a "pure" graphical thing starts. As a verb: to show something on the screen, nothing more. | |
| **UIE** | A UI element, pronounced "ooh-wee": a leaf of an SWG that is a piece of the interface itself, such as a bar. | |
| **scene** | What a level's world looks like from the player's eye, or from behind and above in third person. | "level view" |
| **scene render** | The render of the scene: the rasterized representation of the match at the time of play, as you are playing it. | |
| **minimap render** | The render of the minimap. | |
| **HUD** | Heads-up display: the information a player sees while walking normally, on top of the scene render. It holds the bars, the minimap, the crosshair, the console bar, the pylon squares and the readouts. The main map is not part of it. | |
| **health bar**, **energy bar**, **ammo bar** | The HUD's bars, in the bottom left: health, sprint energy, and the ammo of the gun in hand. | "the bars" |
| **minimap** | A disk in a corner of the HUD (top right by default) that shows the ground within the minimap's range (`minimap.range_m` in player settings, 60 m by default), with players as dots and shots as lines. | "mini map" |
| **crosshair** | The mark in the middle of the HUD. Shots leave the eye toward it. | |
| **console bar** | The bar below the crosshair that fills while the player holds `E` on a console. | |
| **pylon squares** | At the top centre of the HUD: one square per console of each team, an empty slot once it is turned off, and the **own-team box**, a box in the player's team colour around their own team's squares. | |
| **readouts** | The HUD's text lines of numbers, such as speed, height, or how fast the player's time runs near a black hole. | |
| **main menu** | The app's first page: the games on the network to join, joining by an address, hosting, playing alone or against bots, what to play, player settings, credits, quit. | "title screen" |
| **game menu** | The app's page around a game or match: its status, your team, rejoining, the game master's choice, game settings, player settings, leaving to the main menu. | |
| **game page** | The page you play in. Its SWG has the scene render, the actual render of the level's world. | The "game". |
| **level menu** | The page `Esc` opens over the level page, with the sub-pages MATCH, SCORES, SETTINGS, MAP, CREDITS and QUIT. The player stands still while it is open, and the game goes on. | "the menu", "in-level menu", "in-game menu", "game menu" (the official game menu is the app's page around a game, so ask when the context doesn't settle which) |
| **main map** | The picture of the whole level from above, as opposed to the minimap. It comes in two kinds, the satellite map and the paper map. | "the map", "in-game map", "map page" |
| **satellite map** | The main map shown while `Tab` is held, a hidden part of the level page's SWG. It is live: it shows where the players are now, and its fog of war starts at the player's fog of war range. | |
| **paper map** | The main map on the level menu's MAP sub-page. It shows the whole level clear, with no fog of war, and is not live: no players are on it. | |
| **fog of war** | The darker, blurred part of the satellite map, behind a white rim: everything farther along the ground from the player than the **fog of war range** (`main_map.fog_of_war_range_m` in player settings, 60 m by default). Other players still show in it. | "blurriness" |
| **game settings** | The game master's settings of a game, a sub-page of the game menu. The file stays `server_settings.yaml`. | "server settings" |
| **player settings** | A player's own keys and view, a sub-page of every menu. The file is `player_settings.yaml`. | |
| **level menu** | The page of a level run alone, outside the app, where a player picks a team: it says CONNECTING, then CONNECTED with each team's head count. In the app, the team is chosen on the game menu. | "team screen" or "lobby" |
| **banner** | A message over the scene render: YOU DIED with a countdown, `<TEAM> TEAM WINS`, LOADING before a level's first frame, and the connection messages. | |
| **window title** | The text in the window's title bar: the frame rate, the render time, the GPU's time and the render scale. | |
| **level menu tooltip** | The help the level menu's `?` shows while the mouse is over it. | |

> NOTE: We may call things that appear over the level page "in-game", such as the "in-game menu" (the level menu) or
> the "in-game map" (the satellite map, on `Tab`). That wording is not correct: a game is one that hasn't started. Read
> it as the official term, and never write it.

### The SWG model

> NOTE: The SWG is terminology and a mental model only. There is no SWG file and no SWG code, on purpose. Later we
> may define SWGs more formally, with an SWG language, YAML or another format. Until the user asks for that, don't
> create one.

1. **What it holds.** Everything on a page that is visualized and is not part of the 3D world is part of the page's
   SWG.
2. **A tree.** An SWG is a tree of nodes, as a web page's DOM is.
3. **Only leaves are rendered.** Leaf nodes are shown on the screen, and every other node groups its children.
4. **A leaf is a render or a UIE.** Always one of the two.
5. **Offsets.** Conceptually, each leaf has an offset from its parent along the `z` axis (depth) as well as the `x`
   and `y` axes. They could be relative or absolute coordinates, as in CSS, but we don't handle that yet.
6. **Shared depth, by design.** Nodes can have any such offsets, but it is almost always recommended, and designed,
   that the UIEs and renders of a group share a similar `z`: the HUD's leaves all have the same `z`, and the HUD is
   on top of the scene render.
7. **Hidden parts.** A part of an SWG can be hidden until something shows it, as the satellite map is.
8. **One SWG per page.** Each page has its own SWG. Leaving the level page for the game menu leaves one SWG for
   another.

## World objects

What is in a world. An object's visuals are its sprite, and the colours and textures on it are its skin, as for an
entity.

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **object**, **structure** | The same thing: anything placed in a world, such as a wall, a rock, a pylon or a building. Every object in a world has a name that is unique in that world. | "thing", "building" |
| **pylon** | One of a team's 1 to 3 objectives in a level, near its spawn, made of one or more consoles. While its console is on, a pylon sends up a **pylon beam**, a column of light in its team's colour. | "flag" |
| **console** | One part of a pylon. The opposing team deactivates consoles by holding `E` on them, and wins the level when all are off. | |
| **warp object** | A black hole or a wormhole. Level designers place them, and each player throws a pair of wormhole mouths. | |
| **black hole** | A warp object that bends light and lasers, slows time near it, and sends a player who falls in back to their spawn. | |
| **wormhole** | Two linked mouths: what goes into one comes out of the other, bent on the way like light. | |
| **mouth** | One end of a wormhole. | "wormhole" |
| **thrown wormhole** | The pair of mouths a player throws with `Q`: a purple mouth, then a green one, once per level. | |
| **fixed wormhole** | A wormhole a level designer placed. | |
| **laser** | The light ray a shot draws, from the gun's tip, in the shooter's team colour. | "beam" |
| **shot** | One firing, with its path and what it hits. | |
| **beam noise** | The flicker of random pixels of a laser. | "speckle" |
| **shot noise** | The small random angle by which a shot strays from the crosshair. | |
| **sniper rifle**, **rifle** | The two weapons every player carries (`1` and `2`). The rifle looks like an energy revolver. | "revolver" |
| **team colour** | Blue or red, on bot sprites, lasers, pylon beams, boundary planes and the minimap's dots. A player's thrown mouths are purple and green. | |
| **shimmer** | The slight wavering of what a player sees through a boundary plane, until they cross it. | "haze" |
| **edge stretch** | What a player sees as they slow down at the level edge (Zeno's paradox): the view stretches out from the middle of the screen, more and more as they slow. Its strength is the player setting `view.edge_stretch_frac`. | "Zeno's paradox", "getting Zeno's-paradoxed" |
| **haze** | Thin fog in a level's air. | |
| **sky** | What is drawn beyond a level, for looks only, such as the sun, a planet, stars and the Milky Way. | "skybox" |

## Worlds

Each level's world, with every object's name, is in [`worlds/`](worlds/README.md): one file per level, with its zones,
its named objects and marked map pictures. An object's name there is its official name, such as "blue left pylon" or
"blue hesco wall 3". The naming rules are in that folder's README.

## Moving and looking

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **walk**, **sprint**, **crouch**, **jump** | The ways a player moves on the ground. Sprinting (`Shift`) is twice as fast as walking and uses energy, crouching (`Ctrl`) lowers the eye and halves the walking speed, and a jump (`Space`) gets a player onto a 1 m block. | "run" for sprint, "jog" for walk |
| **aim** | Holding the right mouse button: the view zooms in, more with the sniper rifle, and turns more slowly. | |
| **first person**, **third person** | The two views `V` switches between: from the player's eye, and from slightly behind and above the player's bot sprite. In both, shots leave the eye toward the crosshair. | "third-person view" |
| **head bob** | The view moving up and down with each step. | "head bobbing" |
| **gun sway** | The gun in hand swinging with each step. | "the way the gun moves" |
| **flashlight** | A cone of light from the player's eye while `F` is held. Other players don't see it yet. | |
| **deactivate** | To turn a console off: hold `E` for 10 s while aiming at it from within 3 m. Letting go, or aiming away, resets its progress. | "capture", "disable" |
| **throw** | To send out one of the player's two wormhole mouths with `Q`. A throw that doesn't land within its flight time, or lands too close to a wall or another warp object, **fizzles**, and can be thrown again. | |

## Space and physics

Space and physics terms are subject to change. If any agent notices discrepancies with the code let the user know immediately.

"Curvature" alone can mean a level's curvature type or its curvature amount. Write the one you mean.

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **space** | The geometry of a level's world: hyperbolic (Dust, Lunar Surface), inside a sphere (City Sphere), or flat with two looping axes, T² × ℝ (Snow Planet and Dark Forest, which don't loop yet). | |
| **curvature type** | Which way a level's space is curved: hyperbolic, spherical, or flat (Euclidean). It is `space.kind` in a level layout. | "curvature" |
| **curvature amount** | How strongly a level's space is curved, given by its **curvature radius** in meters: the smaller the radius, the stronger the curvature. It is `space.curvature_radius_m` in a level layout, and the game setting `space.curvature_per_map_curvature` can curve a level more than its layout (Lunar Surface's 40 m plays as 26.7 m). Flat space has none. | "curvature" |
| **gravity** | What pulls players onto the ground, or outward onto the sphere in City Sphere. | |
| **pull** | The acceleration that draws a player along the ground toward a black hole or a mouth. It builds a **drift**, a velocity added to the player's walk. | "gravity" toward a warp object |
| **time dilation** | Time running slower for a player near a black hole: their movement, energy, console holds and guns slow down. | |
| **bend** | What a warp object does to light, to lasers and to a player's path near it. | |

## Building, measuring and shipping

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **voxel level** | A level drawn from blocks by the levels' shared code: Lunar Surface, Dust, Snow Planet and City Sphere. Dark Forest is built on the `fps_demo` engine instead. | |
| **model**, **texture** | The files a sprite is made from: a 3D shape, and the picture on a surface. The CREDITS sub-page says where each comes from. | |
| **render scale** | The share of the window's resolution a level's picture is drawn at. `-` and `=` lower and raise it, and the game can adjust it to hold the frame rate. | "render resolution" |
| **frame rate**, **GPU time**, **render time** | How many pictures a second the game draws, and how long the GPU and the whole renderer take for one. The window title shows all three. | |
| **bench view** | A fixed view of a level used to measure GPU time, such as Lunar Surface's `wormhole_exit`. | |
| **Mac app**, **bundle** | `Warped.app`, the app packaged for macOS, and the zip of it that players download. | |
| **download page** | The public web page where players get the bundle ([`infra/`](../infra/README.md)). | |
| **playtest** | The user playing a build to judge it. | |

## Tracking work

| Say | Meaning | Might be called by the user |
| --- | --- | --- |
| **wishlist**, **item** | The checklist of what the user has asked for ([`USER_WISHLIST_CHECKLIST.md`](../demos/hyperbolic_wormholes_rust/USER_WISHLIST_CHECKLIST.md)), and one request on it, with an id such as `W-85` and a short name. | |
| **open**, **claimed**, **pending user review**, **done** | An item's four statuses: nobody has started it; an agent is working on it; it is merged into a `staging/user-<username>` branch and waits for the user to test it; the user tested it and confirmed it works. An item is done only once the user confirms it. | "done" for merged but unconfirmed |
| **WIP** | Work in progress: a commit that saves unfinished work so another agent can pick it up. | |

## Words we retired on 20261110

Older docs and commit messages, including parts of [`DESIGN.md`](DESIGN.md), still use these words. Use the new
term in anything new, and fix the old text when you edit it. The user may still say an old word; read it as the new
term.

| Old | New |
| --- | --- |
| map demo, four map demos | level, the levels |
| per-map settings | game settings |
| map edge | level edge |
| zone plane, zone wall | boundary plane |
| mini map | minimap |
| match server | game server |
| server settings (the menu's) | game settings |
| title screen | main menu |
| team screen | lobby |
| screen | page |
| in-game menu, in-level menu | level menu |
| in-game map | main map |
| level view | scene |
| robot, robot sprite, bot skin | bot sprite, or bot for the entity |
| speckle | beam noise (the user may still say speckle) |
| kazemat, bunker (as a term) | object; each one has its own name in its world, such as blue bunker |
| curvature (alone) | curvature type or curvature amount |
| gravity toward a warp object | pull |
| capture a flag, disable a pylon | deactivate a console |
| run, jog | sprint, walk |
