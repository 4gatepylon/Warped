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
- [World objects](#world-objects)

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
| **window** | The operating-system window of the app. The official game has exactly one; older demos opened one per level and one for the match. | |
| **game server** | The headless program that hosts games and matches, and that players join. In the official game it is the program of `demos/warped_match_rust/`. | "match server" or "server" in the context of interactions in the world when using multiplayer. |
| **level server** | The headless program a game server starts to run one round's level: its players, bots, shots and consoles. This may not exist in the future | |
| **starter game** | The old 2D square game with its own `clients/starter/`, `game_server/` and `management_server/`. Historical, to be replaced soon. | "the 2D game" |
| **management server** | The starter game's program that starts game servers and tracks who may join them. A historical version exists, and a new version may be built in the future. | |
| **cloud** | Game servers on Google Cloud, with a download page ([`infra/`](../infra/README.md)). | |
| **demo** | A program built to test an idea. Older commits call each level and the match "demos". | |
| **library** | A Rust crate that levels share, such as `warped_shared`. | |

> NOTE: if any of this seems outdated, ask the user.

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
