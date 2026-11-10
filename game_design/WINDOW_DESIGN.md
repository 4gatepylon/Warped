# Warped window design

This document describes the app's windows: which windows there are, how a player gets from one to the next, what
each shows, which settings it has and who may change them. [`INSIDE_GAME_DESIGN.md`](INSIDE_GAME_DESIGN.md) describes
what a player sees and does while playing a match, and [`OUTSIDE_GAME_DESIGN.md`](OUTSIDE_GAME_DESIGN.md) the rules
that the windows outside a match carry out.

- **≈** marks a first guess, to be tuned in playtests.
- **TBD** marks something not designed yet.
- **Not built yet** marks a window that is designed but that the Rust game doesn't have yet.

## The windows today

The windows a player meets, in order: the title, the loading screen, the lobby, the match itself
([`INSIDE_GAME_DESIGN.md`](INSIDE_GAME_DESIGN.md#screen)), and the menu, which opens over the match.

- **Title** (the Mac app, opened without flags): a list of the online games on the server the app was built for,
  joining a server by the address the player types (its IP and port, remembered for next time), joining a game on
  this Wi-Fi, and Quit, each game on either map. A game on a server asks once for its password (empty for a
  server without one) and keeps it; a server that refuses it, or runs another version, stops the window with a
  pop-up saying why, and the title screen comes back.
- **Loading:** before the first frame, a black screen titled LOADING names what is loading (the surface textures,
  the shaders, each gun) over a bar like the console bar, filling as it goes.
- **Lobby** (the team screen): it says CONNECTING until the server answers, then CONNECTED with how many players
  each team has. `1` joins blue and `2` joins red; it then says JOINING THE GAME until the server lets the player
  in. A player can't shoot from the lobby.
- **Connection banners:** when a window can't play on its server, a banner says why, and the terminal says what to
  do:
  - NO ANSWER FROM THE SERVER: nothing came back for 2 s;
  - SERVER RUNS ANOTHER VERSION: the server's messages don't fit this build, so rebuild both from the same commit;
  - THE GAME IS FULL: the server already has 12 players.
- **Exit:** `Esc` opens the menu; on its QUIT page (`Q` from SCORES, where it opens) "Do you want to leave the
  game?" leaves with `Return` or `Y`, and `Esc` goes back to the game.

### The menu

- **Menu** (`Esc`, which frees the mouse; `Esc` again closes it and captures the mouse): its name on the first line,
  the names of its five pages on the line under it (the open one in brackets), then the page. It opens on SCORES.
  The left and right arrows, `Q` and `E`, or a click on a page's name turn the pages. On SETTINGS, `Down` or `Up`
  goes from the pages' names into the rows, where the left and right arrows change the chosen number instead, and
  past the first or last row comes back to the names. A `?` at the right of the first line shows the menu's keys
  while the mouse is over it; the keys aren't shown otherwise. Inside the one-window app a sixth page, MATCH, comes
  first, with the match's status (the current level, the round, which team attacks and the scores), and the menu
  opens on it. Inside the menu the arrows, `Return`, `Esc`, `Q` and `E` always work, whatever the key
  bindings say, so a bad binding can always be fixed. The player stands still while it is open,
  but the game goes on.
  - SCORES: the player count, and each player's team, kills and deaths in this game, sorted by
    team and then kills, with the player's own line marked and each team's totals. Before a game
    on a server it says there is none, and before the server's first scores it says to join a team.
  - SETTINGS: the player's own settings, one row each: every key binding (`Return`, then the new
    key, button or wheel; `Esc` cancels), then numbers changed with `Left` and `Right` (mouse look
    speed, motion blur, head bob walking and sprinting, how red, shaky and blurred a hit looks,
    the wormhole colour's strength, the Milky Way's brightness, and the picture's brightness,
    contrast, saturation and gamma, which change only how the finished picture looks on that
    player's screen, W-51, basic visuals), then a row that saves them to
    `~/.config/warped/player_settings.yaml` (or the `--player-settings` file), which later games
    load. A line below says what changed or went wrong.
  - MAP: the whole level from above, with the ground's relief, structures, consoles, wormhole
    mouths, the black hole, the spawns, the zones' edges and the map edge, the player as a disk
    pointing the way they face, a circle of the minimap's reach around them (the rest darker and
    blurred), and every other player in the game, near or far, as a dot in their team's colour.
    A legend line names the marks.
  - CREDITS: where the game's assets come from (their authors, sources and licenses), the font
    and the libraries the game is built on.
  - QUIT: "Do you want to leave the game?"; `Return` or `Y` leaves it (a level run alone quits; the one-window app
    goes back to its game menu), and `Esc` goes back to the game.

## Who changes which settings

| Settings | Where | Who changes them |
| --- | --- | --- |
| A player's own: key bindings, mouse look speed, the minimap, how the picture looks | The menu's SETTINGS page, saved to that player's `player_settings.yaml` | Each player, for themselves only |
| The game's: gravity, friendly fire, when a player may shoot and the other numbers that shape play, the same for everyone in a game | The game server's `server_settings.yaml`, and the one-window app's game menu during a match | The game master. Today any player in the game can act as the game master. |
