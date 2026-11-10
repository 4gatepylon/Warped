# Decisions (the one-window app)

Decisions made instead of the user while building `warped` (W-141, one window for the whole game).
**Weigh in** marks the ones the user should check.

- **Menus are drawn on the CPU in SDL's debug font and copied into the window's Metal layer**,
  rather than drawn with SDL's renderer: the level draws into the same `CAMetalLayer` when the app
  hands it the window, and SDL's renderer can't share a layer with Metal. A page is drawn again only
  when it changes, so the menus cost the GPU one copy per change. It keeps the match window's look.
  **Weigh in** if the menus should look richer (a real font, pictures behind them).
- **Escape never quits.** In a level it opens the in-level menu, in the in-level menu it resumes, in
  the game menu it goes to the main menu (the user's rule), on the main menu it does nothing.
  Quitting is the main menu's or the in-level menu's Quit row, or closing the window.
- **A hosted game outlives leaving its game menu**: leaving goes to the main menu and the match
  server keeps running, with "Back to the game you host" and "Stop hosting" rows there, so an
  Escape too many never ends a game for everyone in it.
- **The game master's choice is free play on one level, or the match**: choosing a level sets the
  match aside to play that level round after round, counting each team's wins, and choosing the
  match goes back to it where it was. Overriding the match's next level instead would break its
  wavefront rules. Either choice ends the round under way and starts the next after the
  intermission, so every player follows. The match's 30-minute clock keeps running during free play.
  **Weigh in** on both.
- **"Hop into any map" from the main menu** is the same free play: the app hosts a match server and
  sends it the chosen level, so a one-level game is still a match server with its game menu, beacon
  and bots.
- **Playing alone runs a bot on the other team that sits out every round** (the match client's
  `--bot-round-winners`, naming the player's team for 255 rounds), because a round starts only when
  each team has a player. The player is blue; switching team moves the bot. A game with allies but
  no enemies gets the same sitting-out bot.
- **Bots: up to 5 allies and 6 enemies** (W-155, play against bots: "5v5 or smth like that"),
  because a level server takes 12 players (`warped_shared::level_net::MAX_PLAYER_COUNT`: a 5v5 and
  a spare player each team), 6 a team. **Weigh in.** In a game with bots the player is blue, and
  switching team doesn't move the bots.
- **The match client passes `--bot-objective` to its rounds' bots** (`consoles` when not given, as
  before), so fighting bots fight.
- **The main menu lists match servers only**, found by a new beacon on the reserved UDP 47851
  (`WARPMTCH`, a version byte, the port, players per team, phase, round; readers ignore bytes
  after the fields they know). A level server alone (`make serve-game`) isn't listed: the app joins
  levels through a match.
- **The level's stand-in**: until the levels are libraries, a round shows a screen naming the level,
  the round and the team, with the level's map picture while Tab is held, and follows the match as
  a level will (it leaves when the round ends, Escape opens the in-level menu). No level process is
  started for the player, so the round goes on without them.
- **Player settings moved to `warped_shared::player_settings`** (Lunar Surface's code, the
  reference, with W-140's key upgrade) so the app and the levels share one reader; the levels still
  use their own copies until they become libraries, which should switch them over. The app's page
  edits every key binding and the in-level menu's nine numbers with the shared
  `settings_editor`, which the game master's settings page will use too.
- **Children's logs** go to `.artifacts/warped/<run_id>/` in the checkout and to
  `~/Library/Logs/Warped/warped/<run_id>/` from `Warped.app` (the match server's to
  `~/Library/Logs/Warped/serve_match/`).
- **The Mac app holds every program it starts** (`make package-mac-app`): the app as
  `Contents/MacOS/Warped`, the match server (which is also Lunar Surface and runs the bots) as
  `warped-match`, and each other level as `levels/<level>`, named from the manifest; the match
  server finds them there (`app_bundle::get_app_level_program`). The levels build one after another
  into one shared target folder, which saves several GB over building each in its own `.build/`.
- **The packaged levels still read their files from the checkout** (`CARGO_MANIFEST_DIR`: maps,
  textures, settings), so the app plays levels only on a Mac with the checkout where it was built.
  Making them relocatable changes every level crate, left to the levels-as-libraries and W-143
  work. **Weigh in** on when the app must run on another Mac.
