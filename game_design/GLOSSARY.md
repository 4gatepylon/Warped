# Glossary

The words this project uses for the game, one meaning each. Every agent MUST use these terms exactly, in code,
docs, commits, PRs and messages. If you need a term this file lacks, or aren't sure which one fits, ask the
player (the user you work for) a precise question and wait for the answer: name the term, quote the sentence
where it came up, give the meanings you see, and say which one you would pick. Never invent or guess a term.

- *Proposed* marks a pick between words the project used inconsistently, or a definition the sources didn't
  settle. The player hasn't confirmed it; the questions at the end ask.
- **Say** is the term to use. **Not** lists the words it replaces. Older docs and commit messages, including
  parts of [`DESIGN.md`](DESIGN.md), still use the retired words; use this file's term in anything new, and fix
  the old text when you edit it.
- A new term needs the player's agreement. Add its row in the same change.
- Sources: [`DESIGN.md`](DESIGN.md), the READMEs of [`demos/warped_mvp_rust/`](../demos/warped_mvp_rust/README.md)
  and its parts, the wishlist, and the commit messages on `staging/user-<username>` (2026-11-10).

## Structure of play

| Say | Meaning | Not |
| --- | --- | --- |
| **game** | A game that exists but hasn't started: a game server hosts it, it has game settings and a choice of what to play (the whole map, or one level), and players join it in the lobby and pick teams. | calling a started game "a game" |
| **match** | A game that has started and is ongoing: two teams play across the levels of a map until a team wins it or it ties. When it ends it is a *finished match*. | "game" for an ongoing game |
| **free play** | A game or match on one level instead of the whole map. | |
| **round** | One play of one level during a match, which one team wins. | |
| **level** | One playable space: Dust, Snow Planet, City Sphere, Dark Forest or Lunar Surface. It has its own space, ground, structures, pylons, warp objects, sky and theme. | "map" for one level; "demo" for a level |
| **map** | The line of levels a match is played across: the blue levels, one purple level in the middle, then the red levels. The default map has five levels. | one level; a level's layout |
| **level layout** | The data that says where a level's ground, structures, spawns, pylons, warp objects, lights and sky are, in the portable format of [`common/maps/`](../common/maps/README.md). The folder keeps the name "maps". *Proposed.* | "the map", "map file" |
| **depth** | How far a level is from the purple level: 0 for purple, then 1, 2, ... outward. | |
| **team** | Blue or red, about 5 players each. The purple level belongs to neither. | |
| **wavefront**, **attacker**, **defender** | A team's next level to win on the other team's side; the team whose wavefront is being played; the team that owns the level being played. Not built yet. | |
| **spawn** | Where a team's players enter a level. | |
| **zone** | One of a level's three regions: the blue zone, the middle zone and the red zone. | |
| **boundary plane** | The faint see-through wall in a team's colour at the edge of its zone, which stops neither players nor lasers. In City Sphere it is a dome over each team's pylon. *Proposed.* | "zone plane", "zone wall" |
| **route**, **floor** | One way across the middle zone; one walkable layer inside a level. | |
| **level edge** | The soft edge of a level, where players slow down without end as they climb, so they never leave. *Proposed.* | "map edge" |

## People and bodies

| Say | Meaning | Not |
| --- | --- | --- |
| **player** | A human or a bot taking part in a game. | |
| **bot** | A computer-controlled player, on your team or the other. | "robot" |
| **robot** | The 3D body every player appears as, in their team's colour. A robot is what you see; a bot is who steers it. | "bot" for the body |
| **game master** | The player who chooses what everyone plays and changes the game settings. | |
| **operator** | The person who runs a game server. | |
| **elimination** | A player is hit enough to be out of health. It counts as a *death* for them and, when an enemy did it, a *kill* for the shooter. | "died", "killed" for the event |
| **respawn** | An eliminated player's return, after the respawn time, on a ring around their team's spawn. | |
| **health**, **energy** | What a hit takes away; what sprinting uses up and time refills. | |

## Programs and places

| Say | Meaning | Not |
| --- | --- | --- |
| **the app** | The official game as one program with one window (`warped`, or `Warped.app` on a Mac). | |
| **window** | An operating-system window. The official game has exactly one; older demos opened one per level and one for the match. | |
| **game server** | The headless program that hosts games and matches, and that players join. In the official game it is the program of `demos/warped_match_rust/` (its folder keeps the name "match"). *Proposed.* | "match server" |
| **level server** | The headless program a game server starts to run one round's level: its players, shots and consoles. | |
| **starter game** | The old 2D square game with its own `clients/starter/`, `game_server/` and `management_server/`. Historical, to be replaced soon. | |
| **management server** | The starter game's program that starts game servers and tracks who may join them. Historical. | |
| **cloud** | Game servers on Google Cloud, with a download page ([`infra/`](../infra/README.md)). | |
| **demo** | A program built to test an idea. Older commits call each level and the match "demos"; say *level* and *game server* now. | "demo" for a level |
| **library** | A Rust crate that levels share, such as `warped_shared`. | |

## Screens and windows

A *screen* is what the app shows in its window at one moment. A *page* is one tab of the menu.

| Say | Meaning |
| --- | --- |
| **main menu** | The app's first screen: the games on the network to join, hosting, playing alone or against bots, what to play, player settings, credits, quit. |
| **game menu** | The app's menu around a game or match: its status, your team, rejoining, the game master's choice, game settings, player settings, leaving to the main menu. |
| **lobby** | The team screen of a game: it says CONNECTING, then CONNECTED with each team's head count; `1` joins blue and `2` joins red. A player can't shoot from it. |
| **loading screen** | The black screen, titled LOADING, with a bar, before the first frame. |
| **level view** | What the level looks like from the player's eye, or from behind and above in third person: the ground, structures, robots, lasers, mouths and the sky. |
| **menu** | What `Esc` opens inside a level, with the pages SCORES, SETTINGS, MAP, CREDITS and QUIT, and MATCH first in the app. The player stands still while it is open, and the game goes on. |
| **MAP page** | The whole level from above, with a legend. It is a page of the menu and also shows over the game while `Tab` is held. |
| **MATCH page** | The match's status: the level and round being played, who attacks, the scores. |
| **game settings** | The game master's settings of a game, in sections, from the game menu. The file stays `server_settings.yaml`. |
| **player settings** | A player's own keys and view (the SETTINGS page; the file `player_settings.yaml`). |
| **banner** | A message drawn over the level view: YOU DIED with a countdown, `<TEAM> TEAM WINS`, and the connection messages (NO ANSWER FROM THE SERVER, SERVER RUNS ANOTHER VERSION, THE GAME IS FULL). |
| **window title** | The text in the window's title bar: the frame rate, the render time, the GPU's time and the render scale. |
| **tooltip** | The help the menu's `?` shows while the mouse is over it. |

Retired: "title screen" for the main menu, "team screen" for the lobby, "map page" for the MAP page, "match
progression page" for the MATCH page.

## The HUD

**HUD** (heads-up display): the information a player sees while walking normally, drawn on top of the level view:
the health, energy and ammo bars in the bottom left, and the minimap. The map itself is not the HUD. *Confirmed by
the player, 2026-11-10.*

| Part | What it is |
| --- | --- |
| **health bar**, **ammo bar**, **energy bar** | The bars in the bottom left: health, the sniper rifle's reload or the rifle's heat, and sprint energy. |
| **minimap** | A disk in a corner (top right by default) showing the ground within 60 m, with players as dots and shots as lines. |
| **crosshair**, **console bar**, **readouts** | The mark in the middle of the screen, where shots leave the eye toward; the bar below it that fills while the player holds `E` on a console; and text lines of numbers, such as speed, height, or how fast the player's time runs near a black hole. *Proposed* as HUD, because a player sees them while walking normally; the player hasn't named them. |

The HUD shows during normal play, while a player walks around a level. It does not show on the main menu, the game
menu, the loading screen or the credits, which are not in a level.

These are not the HUD, and each has its own name: the **MAP page** (the whole level from above, in the menu or while
`Tab` is held), the **held weapon**, the **hit effect** (red and a blur from the screen's edges, with a shake),
**motion blur**, **head bob**, **banners**, the **menu** and its other pages, the **window title**, and everything
inside the level (robots, lasers, mouths, pylons, consoles).

Where the HUD shows when the menu, the `Tab` map, a banner or the lobby is on screen is not settled: see the
questions.

Retired: "mini map" and "mini-map" for the minimap, "player icons" for the minimap's dots.

## World objects and effects

| Say | Meaning | Not |
| --- | --- | --- |
| **pylon** | One of a team's 1 to 3 objectives in a level, near its spawn, made of one or more consoles. A pylon sends a **pylon beam**, a column of light in its team's colour, up while its console is on. | |
| **console** | One part of a pylon. The opposing team deactivates consoles by holding `E` on them, and wins the level when all are off. | |
| **warp object** | A black hole or a wormhole. Level designers place them, and each player throws a pair of wormhole mouths. | |
| **black hole** | A warp object that bends light and lasers, slows time near it, and sends a player who falls in back to their spawn. | |
| **wormhole** | Two linked mouths: what goes into one comes out of the other, bent on the way like light. | |
| **mouth** | One end of a wormhole. | "wormhole" for one end |
| **thrown wormhole** | The pair a player throws with `Q`: a purple mouth, then a green one, once per level. | |
| **fixed wormhole** | A wormhole a level designer placed. | |
| **reach** | How far a warp object bends light. | |
| **lens** | What a warp object's bending looks like: the world seen bent around it. | |
| **bunker** | A concrete structure players walk into, such as Lunar Surface's. | "kazemat" |
| **laser** | The light ray a shot draws, from the gun's tip, in the shooter's team colour. Its drawn look and flicker are its *beam look* and *beam noise*. | |
| **shot** | One firing, with its path and what it hits. | |
| **beam noise** | The flicker of random pixels of a laser. | "speckle" |
| **shot noise** | The small random angle by which a shot strays from the crosshair. | |
| **sniper rifle**, **rifle** | The two weapons every player carries (`1` and `2`). The rifle looks like an energy revolver. | |

Level features, by level: Dust has a wind that bends lasers; Snow Planet has a tall blizzard zone that slows and
hurts; City Sphere has a glass floor over the outer floor and walls that make close quarters; Dark Forest has
treehouses on decks, rope ladders, plank bridges, a lookout and five fixed wormholes; Lunar Surface has a black
hole above the middle, a crater wall as its level edge, and bunkers, domes and a landing pad.

## Words we retired

| Old | New |
| --- | --- |
| map demo, four map demos | level demo (older text only) |
| per-map settings | per-level game settings |
| map edge | level edge (*proposed*) |
| zone plane, zone wall | boundary plane |
| mini map | minimap |
| match server | game server (*proposed*) |
| server settings (the menu's) | game settings |
| title screen | main menu |
| team screen | lobby |
| speckle | beam noise |
| kazemat | bunker |

## Questions for the player

1. **HUD's other parts.** Are the **crosshair**, the **console bar** and the **readouts** part of the HUD?
   Proposed: yes, since a player sees them while walking normally.
2. **HUD's visibility.** Does the HUD stay on screen behind the open **menu**, under the `Tab` map, while the
   player is dead (under YOU DIED), and in the **lobby**? Proposed: shown in none of these.
3. **Game and match.** When a game starts it becomes a match: is *start* the right word, and what should the
   finished match's game be called, if anything? After a win, DESIGN.md says "a new game starts"; should that read
   "a new game is created, and starts as a match"?
4. **Game menu and menu.** Is the app's **game menu** the same menu as the **menu** `Esc` opens in a level (with
   its MATCH page), or two different menus? Proposed: two.
5. **Map and level layout.** `AGENTS.md` and `common/maps/` call a level's layout "a map". Do you agree to say
   *level layout* (and keep the folder name), so that *map* only means the line of levels?
6. **Level edge.** Do you agree to retire *map edge* for *level edge*, for the same reason?
7. **Game server.** Do you agree to retire *match server* for *game server*, since a server hosts games before
   they are matches?
8. **Pylon beam.** The wishlist calls a pylon's column of light a "beam", and DESIGN.md also says *beam* for a
   laser's drawn look and flicker. Is *pylon beam* the right name for the column of light?
