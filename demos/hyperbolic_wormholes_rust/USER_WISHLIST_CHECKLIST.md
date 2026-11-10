# User wishlist checklist

Every feature and fix the user has asked for in the Rust wormholes demo, so that agents on any platform (Claude Code,
Codex, Devin) can see what is wanted, who is working on what, and what is done. How to add, claim and finish items
is in [`AGENTS.md`](AGENTS.md).

- `[ ]` open: nobody has started it.
- `[~]` claimed: someone is working on it; the line says who, on which branch, since when.
- `[?]` pending user review: merged into a `staging/user-<username>` branch (before 2026-11-08, `staging`); the line names the PR; the user hasn't confirmed it works yet.
- `[x]` done: the user tested it and confirmed it works.

## Claimed

- [~] **W-143 Declarative levels.** The user, 2026-11-09: "Please use proper dry and come up with a clear way to specify maps. Imagine—in the future we want to be able to DEFINE new maps. Not yet, but that will be out there in the future. to enable that, you will ned to set things up a declarative format properly for that. Assume we won't need to support new map topologies/space/types than waht is currently therE". Design in #485 (`common/maps/LEVELS.md`). Claimed by Claude Code (the lead's subagent, worktree agent-a6fba5f1c56b9a7c3), branch `claude/declarative-levels-validator`, 2026-11-09. Spec #485 (merged), validator #489.
  - **Where the work is:** worktree `/Users/4gate/git/Warped/.claude/worktrees/agent-a6fba5f1c56b9a7c3` on the user's Mac; branch `claude/declarative-levels-validator` (pushed): the validator's level checks and their tests. Step 2 (moving each level onto `level.json`, `common/maps/LEVELS.md`'s migration plan) waits for the levels-as-libraries PRs. State on 2026-11-09.
- [~] **W-126 Same play in every level.** "it looks like in Space Station you don't have the same health bars and
  stuff. All the different levels should have the health bars and the weapons and sort of the same stuff that we made
  happen in Lunar, including even the speckled noise for the lasers and the head bobbing and stuff like that when
  you're running, the way the gun moves. All that has to be the same throughout the entire game. The only things that
  kind of change are, like I mentioned, of course, the laser colors. It's green for the snow planet, golden white for
  dust, red and blue for both Lunar and for Space Station, and also golden white for the Dark Forest. For Dark Forest,
  it can just be like golden, golden. They should all have their own mini maps that are nicely represented. All should
  have their own maps. [...] They all need to be possible to win, which means it needs to be possible to get to the
  pylon. They all have the same ability for the player to place two wormholes if the player wants. It should be
  possible for all of them to press E when you're next to the pylon to disable it. The patterns for the pylons are
  three for lunar and three for dust. Dark forest and two for snow, and then only one pylon for the space station."
  And: "Respawn timers and all that stuff should be the same throughout." And, about each level's own features:
  "dust is special in that it has basically this wind. It's like a sandstorm, and the wind bends the laser in the
  direction of the wind. [...] Dust is also hyperbolic. [...] Dark Forest is like the toroidal setup [...] meant to be
  kind of dark. There's meant to be a little bit of light coming in between the foliage. It's meant to be covered with
  wormholes [...] a planet or level where you can access these ladders that let you go into the trees. So it's kind of
  like a Wookiee planet style, but with bamboo. Snow Planet has [...] exactly one instance of basically a tall
  cylinder in the map that's like a blizzard zone. And if you enter there, you're slowed and you take damage. And the
  damage should, like in all the other variants or locations of the game, sort of jitter your screen and give you like
  the red and blur coming in from the outside. This is something that Lunar has, and that everything has to have.
  Whenever you take damage, in the past, the only ways to take damage were basically by getting shot, but here there's
  like a natural world as well." Claimed by Claude Code (session
  `wizardly-hamilton-23a121` and its subagents), one branch per level, 2026-11-08.
  - **Where the work is** (for any agent picking it up): a parity audit of every level against Lunar Surface on
    `claude/levels-checks`, then each level's fixes on its own agent's branch (`claude/city-sphere-fixes`,
    `claude/dark-forest-lens-robots`, `claude/snow-planet-torus`, `claude/dust-wormhole-speed`), in worktrees under
    `/Users/4gate/git/Warped/.claude/worktrees/` on the user's Mac. State, 2026-11-08: started. Dust: #427
    (`claude/dust-parity`, open). Snow Planet: #421
    (branch `claude/snow-planet-parity`, open). City Sphere: #429
    (HUD bars, hit effect, movement, jump, exit prompt, zoom, head bob, speckle, walking console bot) and #435
    (thrown wormholes, flashlight, third person, ground dust, Lunar's beam looks; `claude/city-sphere-parity-2`,
    open). Lunar Surface, Dust and Snow Planet: #434 (servers
    take a console hold only in reach and aimed at, console bots walk to every pylon, `claude/console-reach-walking-bots`,
    open). Dark Forest: #431 (Lunar's HUD, hit effect and
    "YOU DIED", `claude/dark-forest-parity-hud`, merged), #439 (playable and winnable: the server takes a console hold
    only in reach and aimed at, 10 s; console bots walk to every pylon by the ladders; both guns with Lunar's hits;
    Lunar's jump; hidden bot windows; `claude/dark-forest-playable`, merged) and #445 (Lunar's movement: speeds,
    sprint energy, crouch, jump, air control, step, a hit's slowing, the shot delay after moving, the settings'
    respawn delay; Lunar's flashlight and exit prompt; `claude/dark-forest-movement`, open, 2026-11-09) and #455
    (Lunar's beam looks and speckle, in gold; `claude/dark-forest-beams`, open, 2026-11-08); still missing there: the
    Tab pages, the minimap, third person, thrown wormholes. Dark Forest's thrown wormholes (`Q`): #479
    (`claude/dark-forest-thrown-wormholes`, open, 2026-11-09).
    Shared code: #447 (`demos/warped_shared_rust/`, the files identical in Lunar Surface, Dust, Snow Planet, City
    Sphere and the match, moved into one library they all use, no change in play or pictures;
    `claude/shared-level-code`, merged); its `PLAN.md` is the next step, one movement, guns, health, HUD and robots
    for all five levels: #449 (the settings sections and `shot_noise` shared, `claude/shared-play-settings`,
    merged), #452 (the guns' state and helpers shared, `claude/shared-weapons`, merged), #456 (the movement rules
    shared, with a play trace per level, `claude/shared-movement`, open), #457 (City Sphere's guns on the shared
    `Weapons`, Lunar's rules, `claude/city-shared-guns`, merged), #459 (the HUD's bars, pulses, hit effect, lobby and
    "YOU DIED" lines, the hit feedback and the server's damage count shared, `claude/shared-hud-health`, open),
    #461 (City Sphere moves by the shared movement rules and gains Lunar's crouch, `claude/city-crouch`,
    merged), #466 (Dark Forest on the shared crate: its HUD, hit effect, shot noise and settings sections,
    glam 0.34; `claude/dark-forest-shared-hud`, merged), #468 (Dark Forest's guns on the shared `Weapons`,
    `claude/dark-forest-shared-guns`, merged), #473 (Dark Forest's robots are Lunar's teal robot,
    animated by the shared gait rules; `claude/dark-forest-teal-robots`, merged), #474 (Dark Forest's player
    moves by the shared rules through a `Ground`; `claude/dark-forest-shared-movement`, merged), #475 (the
    third-person view on `V` in Dark Forest, from the shared `third_person`, which City Sphere uses too;
    `claude/shared-level-modules`, open). Dark Forest's thrown wormholes still need its renderer to capture lenses
    while playing.
    Every level: #442 (fighting bots that walk to the nearest enemy and shoot, respawn after the server's delay,
    and `make play-all-levels` checks eliminations, respawns and walks per level; `claude/playable-end-to-end`, open).
- [~] **W-103 Reality bend performance.** "Performance optimization on the reality bending version of the game":
  frames under the reality bend (W-68 (reality bend)) are slow. Claimed by Claude Code (subagent of a session on
  `leah`'s Mac in `Warped-main`), branch `claude/reality-bend-perf`, worktree `/Users/leah/Documents/dev/Warped-rb-perf`,
  2026-11-08.

## Open

- [ ] **W-101 Pylons hold the black hole.** "Make it look like the pylons are holding the black hole (beams?)". Builds
  on W-68 (reality bend), which adds the pylons and the black hole. The user's later requests, 2026-11-08: "Make the
  lasers coming out of the pylons go higher into the sky", filed as W-109 (taller pylon beams), then "is it possible
  for you to make the beams from the pylons go up and arc through the sky to the black hole in the center"; the user
  chose a high arch that replaces the straight beams, so W-109 (taller pylon beams) is folded into this item. Shelved
  by the user, 2026-11-08: the arches (80 m, the user's pick in the game) are on branch `claude/taller-beacon-beams`
  (#361, not merged), but views of the black hole render about 1.5 to 2.5 times slower than without them.
- [ ] **W-113 A black hole for every pylon.** "Instead of making the black hole bigger, BOTH make it bigger AND also
  add another black hole for every pylon solved." Builds on W-107 (time dilation), where each console turned off grows
  the black hole at the centre.
- [ ] **W-114 Fly around black holes.** "You can fly around black holes after the first pylon console deactivation
  (only the first)". Builds on W-107 (time dilation)'s black hole and on the pylons' consoles.
- [ ] **W-115 Pylons per map.** "Update gameplay to say that there are 3 pylons etc., but say that per-map the effect
  of deactivating a pylon varies. For all other maps also have 2 pylons (so not 4 consoles and one pylon, 2 pylons;
  and for the last map 3 pylons instead of 1 pylon with 5 consoles)." The user chose, 2026-11-08: one console per
  pylon on every level; per team, 3 pylons on Dust and Lunar Surface and 2 on Snow Planet, City Sphere and Dark
  Forest. The design doc says so in its own PR; this item is building it in the game.
- [ ] **W-105 Screen tearing.** "There is screen tearing" (in the reality bending version). Found in #319: not vsync
  tearing (the window presents at vsync, one frame in flight) but bands and strips on far ground under the reality
  bend, from `BEND_SAG_PIXELS` = 0.5 in `render_voxel.metal`; 0.1 px removes them at 13% more GPU time (34% on the
  overview view). The user chose to keep 0.5 px for now, 2026-11-08, with W-107 (time dilation) replacing the bend.
- [ ] **W-40 Graphics caveats.** Fix the Lunar Surface graphics caveats listed in `PLAYTEST.md` ("Graphics on Lunar
  Surface, to fix in later pull requests"); slightly wrong graphics were accepted for playtesting "and we can fix
  them in a following PR". Among them: rays trapped near a throat are black on Lunar Surface (the black band seen
  when turning inside a wormhole, which the house already paints from the pixels around it; "it doesn't need to be
  100% correct physically"), and Lunar Surface should match the WebGL level.
- [ ] **W-44 Smooth wormhole interior.** Inside a wormhole, looking sideways shows a grainy texture; it should look
  pretty, and a trivial fix such as blurring is fine. Looking at a wormhole from outside must stay correct.
- [ ] **W-47 Admin dashboard.** Change the server's settings in real time: someone connects as an "admin", who can't
  play but has a dashboard where they can change or reset those settings.
- [ ] **W-51 Basic visuals.** Players can change some basic visuals, such as saturation, so the game isn't always so
  dark.
- [ ] **W-130 No own body through a wormhole.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): You don't see yourself through a wormhole, since the player has no body outside the third-person view (seen in Dark Forest).
- [?] **W-139 Dark Forest thrown-wormhole sheet.** A bug found by agents on 2026-11-09: in Dark Forest's `spawn` view, a hard straight-edged sheet covers part of a thrown wormhole's lens (upper right), like a clipping glitch in the lens capture.
  #478: it was red's lift mouth on its deck, its lens reaching through the deck; no lens is drawn from below a
  mouth's floor now, so the deck's underside (the sheet) hides it whole.
- [?] **W-141 One window for the whole game.** The user, 2026-11-09: "please make it so that all this stuff is one window (if you need to make it one app thats fine, yuo can make a new demo for that and in the main readme say that the official mvp version is in that new folder)", and "thing 2 is to be able to play against gbots or alone selecting from the game itself". Also: "please also make sure to make it possible to hop into any of the maps", and "from the menu (game menu) it should be possible to see which map we are on (where the wavefronts) and stuff like that", "game menu and also in-game menu I mean". The app shell (main menu, game menu, any one level, the match status in both menus) is in `demos/warped_mvp_rust/` (#490; the Mac app, `make package-mac-app`, in #523; the levels in the window and game settings follow). Dark Forest as a library the app plays in its window: #494.
- [?] **W-144 Gravity toward black holes and wormholes.** The user, 2026-11-09: "make gravitational attraction to black holes and wormholes", "it should be relatively weak, but noticeable; i.e if you run towards a black hole you are sped up, if you are runing aw4ay you are slowed", "the acceleration should be elss than walking speed but can be more than coruch speed (in all maps, so it might vary)". Pending review in #521: every black hole and wormhole mouth pulls walkers along the ground in every level (`warped_shared::gravity`, settings `gravity`).
- [ ] **W-145 No walls at map edges.** The user, 2026-11-09: "on all maps there are NOT walls on the ends and as you walk off the edges of the map you get slowd with the same efefct as dust and lunar (i.e. it should zenos paradox you basically)", "also you can get rid of the toroidal topology to have just euclidean specificalyl for dark forest and snow planet", "(so snow planet and dark forest have perma slow mechanics", "since dark forest wont have a wall it needs to be such that you are slowed in teh forest so that you see trees in front of you as you walk off the map (ie you should never see empty space", "(you an put a caveat in game design that the toroidal maps are not actually toroidal and that that is a worok in progress btw)".
- [ ] **W-147 Shooting while moving, by game setting.** The user, 2026-11-09: "add to wishlist the ability to toggle via game settings whether you can shoot while jumping, and seperately while moving laterally (i.e. while walking) and seperately while sprinting".
- [ ] **W-148 Team boundary planes in every level.** The user, 2026-11-09: "all levels should have the red/blue boundary planes that slightly shine (blue red)". Lunar Surface has them already (W-64 (zone planes)).
- [ ] **W-149 Shimmer through boundary planes.** The user, 2026-11-09: "the visuals through those boundary planes should shimmer a little bit until you cross them". Builds on W-148 (team boundary planes in every level).
- [ ] **W-150 Arrow keys and clicks for menu pages.** The user, 2026-11-09: "enabling arrow keys for switching between menu panes and also enabling clicking on them".
- [ ] **W-151 Menu layout and help tooltip.** The user, 2026-11-09: "menu page have menu name at the top and the options BELOW it (i.e. the panes "which pane you are in" with brackets and the pane names", "there should also be a question mark that fi you hver tells you the tooltip for how to navigate the menu instea edof showing by default the instruction Q/E".
- [ ] **W-152 Pylons stay off, as a setting.** The user, 2026-11-09: "pylons once turned off are turned off forever and that this is a toggled setting (put that in wishlist, dont impl yet)", "that way if you jump back to a level that was previously played that you just barely lost, it takes less time to win again in theory". Not to be built yet.
- [ ] **W-153 Shot through a wormhole looks wrong on the other side.** A bug the user found playtesting, 2026-11-09: "a bug where bug shot across a wormhole shows up with the wrong visuals on the other side (looks like it's dust right on the surface of teh wormhole, but it shouldk be warped instead)". The user: "do NOT fix this yet".
- [ ] **W-154 Voice agent for map variants.** The user, 2026-11-09: "a way to use a voice agent to spin up map variants or modify levels even while they are running". Builds on W-143 (declarative levels).
- [?] **W-155 Play against bots.** The user, 2026-11-09: "add to wishlist ability to play against bots". Also asked for in W-141 (one window for the whole game): "thing 2 is to be able to play against gbots or alone selecting from the game itself"; being built in the app (play alone and play against bots from the main menu, and `make play-alone` / `make play-bots` for the match). And: "note wishlist paly agaisnt bots includes playing with multiple allied and enemy bots so that you can play 5v5 or smth like that". #515: `make play-alone`, `make play-bots ALLIES= ENEMIES= BOT_OBJECTIVE=` in the match, and bots on your team and the other from the app's main menu (at most 7: a level takes 8 players).
- [ ] **W-156 Friendly fire, by game setting.** The user, 2026-11-09: "add setting in game settings to toggle whether or not friendly fire is possible".
- [ ] **W-157 Bullet drop, by game setting.** The user, 2026-11-09: "add setting in game settings for bullet drop".
- [ ] **W-158 Per-map game settings.** The user, 2026-11-09: "settings that are custom for maps: Gravity on lunar; Reflectivity and n bounces in sphere; wind speed on dust; snowstorm size in ice planet; nothing in dark forest but it should still have that menu /(and have it be empty)". A per-level section of the game settings in the game menu, from each level's definition (W-143 (declarative levels)).
- [ ] **W-159 Tell wormhole pairs apart.** The user, 2026-11-09: "some way to tell apart which pairs of portals go together" (portals being wormholes), "because sometimes it's highly non-obvious where will go where". And: "note how we might use colors per player or per team or something else for this identiifcation and that it should be possible to toggle/switch around". Ideas to choose from: one colour per wormhole pair (a rim or glow shared by both mouths), the thrower's player colour for thrown pairs, team colours, or a matching symbol or number on both mouths; which one is used is a setting the player can toggle.
- [ ] **W-160 Play any variant or commit quickly.** The user, 2026-11-09: "skills to be able to run multiple differnt game variants side by side and/or have some "link to be able to play the game at this specific commit hash" where that link is always available in every commit (or easy to erive i.e. by pasting hash into some site)", "this way it's super easy to test super quickly". This extends the `interactive-review` skill (`.agents/skills/interactive-review/`), whose review lanes are the first step: the user, "interactive-review skill is like the fiorst step towards what I just describedcl".
- [ ] **W-161 Real toroidal flat space.** The user, 2026-11-09: "actually implemneting toroidal flat space". For now Snow Planet and Dark Forest are plain euclidean with slowing edges (W-145 (no walls at map edges)); Snow Planet's looping world is parked on its stack (#423, #424, #430, #438, #446), over budget at 3440x1440 when it stopped.
- [ ] **W-162 Shots come sideways from the crosshair.** A bug the user found playtesting, 2026-11-09: "bullets come sideways from the crosshair in dust (at least dust but unsure about toehrs)". To check: whether it is Dust's wind bend (2.4x, by design, which makes shots land off the crosshair even at 10 m), the shot's stray (sniper noise, rifle heat), the beam drawn from the gun's muzzle rather than the eye, or third person's aim.
- [ ] **W-163 Aim through a scope.** The user, 2026-11-09: "being able to aim through a scope for both rifles". Today aiming zooms the view (2.5x sniper, 1.5x rifle) without a scope picture.
- [ ] **W-164 More weapons.** The user, 2026-11-09: "adding more weapons". Today there are two guns, the sniper and the rifle.
- [ ] **W-165 Money to buy wormholes.** The user, 2026-11-09: "money mechanics that allow you buy wormholes". Today each player has two thrown wormholes (Q) for free.
- [ ] **W-166 Clear rules for destroying thrown wormholes.** The user, 2026-11-09: "generally understanding and determinising the user wormhole destruction mechanics (Can be they be destroyed after placement?) and how to do it right so taht the wormhole doesn't have wierd non-determinism about whether it bends bullets or is killed by them". Today (Lunar's rules, also in Dark Forest since #479) a shot through an enemy pair's throat damages it (2 sniper or 5 rifle hits break it) while the shot is also carried through, and a shot at a lone mouth's ball destroys it at once; first step: write down exactly when a shot bends, damages or both, on server and window, and make it the same every time.
- [ ] **W-167 Ownership over wormholes.** The user, 2026-11-09: "being having ownership over wormholes". Related: W-159 (tell wormhole pairs apart), W-165 (money to buy wormholes) and W-166 (clear rules for destroying thrown wormholes).
- [ ] **W-168 Fading out into a black hole.** The user, 2026-11-09: "a graphical effect where the world gets darker and arker from outside in (and dims broadly, blurring) when you are pulled into a dark hole (NOT worm hole))". Related: W-144 (gravity toward black holes and wormholes).
- [ ] **W-169 Better sounds.** The user, 2026-11-09: "getting sounds for all the guns etc.. and that the sounds should be better than what they are right now". New sounds come through the adding-assets skill (approval and credits).
- [ ] **W-170 Wishlist to GitHub issues.** The user, 2026-11-09: "switching from wishlist markdown to github issues". Issues follow the github-issue skill (title format, template, labels); the agents' rules about the wishlist (in each demo's AGENTS.md) would move with it.
- [ ] **W-171 The game out of demos/.** The user, 2026-11-09: "refactoring the code to not be in demos/ but instead to be in the main location as the main game". Related: the open refactor plan PR #375 (`game/REFACTOR_PLAN.md`: a `game/` workspace with warped-core, client and server), and the one-window app in `demos/warped_mvp_rust/` (W-141 (one window for the whole game)).
- [ ] **W-172 Better City Sphere graphics.** The user, 2026-11-09: "improving graphics on space station, including laser, the actual glass/mirrors".
- [ ] **W-173 Professional gun models.** The user, 2026-11-09: "improving gun sprites to look professional". No general sprites item exists; related: W-96 (a different rifle model), W-12 (gun on screen), W-73 (gun lit by the scene).
- [ ] **W-174 Working downloadable bundle.** The user, 2026-11-09: "having a working downloadable bundle". Related: W-116 (download page, pending review) and W-141 (one window for the whole game: the Mac app).
- [ ] **W-175 Stable download website.** The user, 2026-11-09: "having a stable website from where the bundle can be downloaded". Related: W-116 (download page).
- [ ] **W-176 Official name and domain.** The user, 2026-11-09: "having an official name and domain for the stable website".
- [ ] **W-177 Online servers people can use.** The user, 2026-11-09: "having a running server (or management + game server(s) such that players can play across different parts of the world) and to actually have this set up and running on gcloud (or equivalnet) so that real people can play online". Related: W-42 (server in the cloud, pending review).
- [ ] **W-178 Keep strangers out of matches.** The user, 2026-11-09: "to have some kind of password or key or secret token or waitlist mechanism to avoid randoms joining matches". Related: common/protocol/authentication.md (in #375).
- [ ] **W-179 Keep strangers out of the game.** The user, 2026-11-09: "to have some kind of password or key or secret token or waitlist mechanism to avoid randoms joining the game at all".
- [ ] **W-180 Elo or ranking.** The user, 2026-11-09: "to have some kind of elo or ranking system based on wins between players".
- [ ] **W-181 Create games on the server.** The user, 2026-11-09: "having the ability to create games on the server".
- [ ] **W-182 Join without knowing the IP.** The user, 2026-11-09: "have ability to not need to know the IP of the server directly".
- [ ] **W-183 Web app for running games.** The user, 2026-11-09: "have the ability to see running games, etc.. from a webpage (and more broadly to have all the non-real-time-video-game features in a web-app as well)".
- [ ] **W-184 Spectating.** The user, 2026-11-09: "have the ability to spectate a game from the app or from a webpage".
- [ ] **W-185 Donations.** The user, 2026-11-09: "to have the ability to donate to the mantainers via a stripe link".
- [ ] **W-186 Load balancing.** The user, 2026-11-09: "to have load balancing in case there are too many players and that this hould redirect to the stripe link".
- [ ] **W-187 Asking players to pay.** The user, 2026-11-09: "to ask that everyone after pllaying for more than 30m pay the maintainers (or maybe after their match; and it's unclear if they should be forced or just be asked kindly; this is still under consideration as the game is built)". Undecided; don't build without the user.
- [ ] **W-188 Map editor.** The user, 2026-11-09: "ability to build maps from a map editor with a reasonably large universe of pre-existing skins etc... and on only hyperbolic and euclidean and possibly toroidal space soon". Builds on W-143 (declarative levels); related: W-161 (real toroidal flat space).
- [ ] **W-189 Black holes can kill, by setting.** The user, 2026-11-09: "ability to toggle whether black holes kill you".
- [ ] **W-190 World-building.** The user, 2026-11-09: "to do world-building so the world for this game is precisely described and everything fits the same aesthetic and a nice storyline basically".
- [ ] **W-191 Voice map edits for developers.** The user, 2026-11-09: "voice to map and/or map edits both offline and online (Realtime); this is for developers and testers". Related: W-154 (voice agent for map variants), W-143 (declarative levels), W-188 (map editor).
- [ ] **W-192 Play recordings for the editing agent.** The user, 2026-11-09: "ability to have a recording of play such that when voice to map edits is done the agent can have access to the frames and/or video that need to change and/or highlight the issue (and possibly better vision models to help with this if needed... unclear)". Related: W-191 (voice map edits for developers).
- [ ] **W-193 Voice to map for players.** The user, 2026-11-09: "voice to map for actual users". Related: W-154 (voice agent for map variants), W-191 (voice map edits for developers).
- [ ] **W-194 Security review and anti-cheat.** The user, 2026-11-09: "some security review and mechanism to reduce the chance of cheating and other possible issues". Related: W-178 (keep strangers out of matches), W-179 (keep strangers out of the game).
- [ ] **W-195 Moving black holes.** The user, 2026-11-09: "moving blackholes".
- [ ] **W-196 Match timers and ties.** The user, 2026-11-09: "match timers for the game to be possible to tie".
- [ ] **W-197 Menus that fit the game's look.** The user, 2026-11-09: "better graphics for the menus that fit the general story and aesthetic of teh game I alldued to before". Related: W-190 (world-building), W-151 (menu layout and help tooltip).

## Pending user review

- [?] **W-146 Pylon squares and own-team box.** The user, 2026-11-09: "the squares mechanic for how many flags have been captured/pylons deactivated should be in every map" and "also there should be a colored box around the team that is your team so players can easily tell which team they are on". #520: in every level (Lunar Surface, the match, Dust, Snow Planet, City Sphere, Dark Forest) a square per console at the top centre (3, 3, 2, 1, 2 per team), an empty slot once turned off, with a box in your team's colour around your team's squares; your team's rows boxed on the scores page, and "you play ..." boxed on the match's progression page (the app's game menu).

- [?] **W-140 Hold Tab for the map, Esc for the menu.** The user, 2026-11-09: "one more request is to be able to click tab to see the map and instead if you press escape then you see the overal menu", "when you hit tab you see the map only as long as you hold tab", "(and you can update hotkeys to reflect this)", "the point is that users will want to see where on the map they are pretty often". In every level.
  #486: in every level (Lunar Surface, the match, Dust, Snow Planet, City Sphere, Dark Forest): holding `Tab`
  shows the map over the game while play goes on; `Esc` opens the menu (freeing the mouse), `Esc` again closes it;
  the menu's new QUIT page leaves the level, and a MATCH page shows the app's match status. Shared `warped_shared::menu_and_map`; old saved key bindings upgrade.
- [?] **W-138 No aliasing.** The user, 2026-11-09, about W-136 (sharp centre, soft edges): "I think middle sharp is fine but there has to be enough blur to remove aliasing elsewhere", "aliasing is a big issue", "like no one ever ever like aliasing so you gota figure out how to deal with. that in particular". Applies to every level: the soft edges, half-resolution lenses and the rest of the image.
  #469: Lunar Surface, the match, Dust and Snow Planet: soft edges and Lunar's half-resolution lens shown through a
  smooth cubic B-spline, lasers kept sharp in lenses, and FXAA on every frame (0.2-0.8 ms at 3440x1440).
  #470: option E is the default sharp centre (the middle half sharp, half resolution to three quarters, a quarter
  beyond, all smoothed), 0.3-2.4 ms faster than B at 3440x1440.
  #471: City Sphere: 4x MSAA on its rasterized pass.
  #465: Snow Planet's half-resolution lens, smoothed the same way, with lasers kept sharp in it.
  #476: Dark Forest: the half-resolution wormhole lens upsampled with a smooth B-spline, 16x anisotropic filtering
  (its FXAA was already on).
  #477: Dark Forest: the lower-rate edges of its sharp centre shown through a smooth B-spline, after FXAA.
- [?] **W-136 Sharp centre, soft edges.** The user, 2026-11-09: "if you need to lower resolution in different places, you can just like blur things. One thing you could also consider doing is having like a narrower field of view where like the center of the screen is clear and fully, you know, full resolution, everything, but the edges are blurry or something like that." And: "Try to at least have the ability to see a nice, good resolution for the important things that are right in front of you." Applies to every level's speed work (60 FPS).
  #464: the middle 66% of the width drawn as before and the rest at half resolution, blended (option B, the
  default; options A-E with timings in the PR for the user), in Lunar Surface, the match, Dust and Snow Planet.
  #477: Dark Forest: Metal's variable rasterization rate, full rate 288 px around the crosshair easing to a third
  toward the edges (3440x1440 drawn at 2172x1054), 30-45% less GPU time per view.
- [?] **W-137 Crouched robots sink.** A bug found by agents on 2026-11-09: other windows place a remote player's feet one standing eye height below the eye they receive (`VoxelPlayer::FromEye`), so a crouched player's robot sinks about 0.7 m into the floor (Lunar Surface and its copies).
  #463: each window sends its crouch, and other windows stand a crouched robot on the floor, squashed to the crouched eye height, with its hit shapes (Lunar Surface, Dust, Snow Planet, the match, City Sphere); #473: Dark Forest too, with its robots' switch to the teal robot.
- [?] **W-127 Dark Forest canopy flicker.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): In about one fixed-step capture in three or four, a few thousand canopy pixels differ, also on the base build before any of the 2026-11-08 work; it looks like a GPU race in the FPS-demo renderer.
  #444: the forest is drawn in 8 m tiles, and repeat captures no longer differ in thousands of canopy pixels (8 of 20 differ in at most 28 pixels by 1-2/255, against 4 of 20 by up to 4,173 pixels and 23/255 before).
- [?] **W-128 Dark Forest double lens.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): Standing within 4 m of a gate shows a faint second image of the wormhole's lens over the direct view.
  #448: from inside a gate's visible sphere, the lens covers the view as far as it bends the light, so no half-covered picture of it lies over the direct view (a visible change inside 4 m of a gate, awaiting the user's OK).
- [?] **W-129 Dark Forest lasers off in the lens.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): Lasers seen through a wormhole lens are drawn in the lens atlas's direction: exact for shots through the throat, slightly off their true place otherwise (#426).
  #448: lasers and glows show where the traced lens ray passes them, read from the live faces in that direction, and hide behind the forest.
- [?] **W-133 Match winning round wording.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): The match's progression page calls the round that wins the match "attacker advances" (e.g. round 5, Dust); changing it changes visible text. Fixed in #443 (pending user review).
- [?] **W-135 e2e-match ports and logs.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): `make e2e-match` (the older match test) uses the game's own ports 47850/47852 and writes its logs into `.build/`, not `.artifacts/`. Fixed in #443 (pending user review).
- [?] **W-132 City Sphere stairs hole.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): The stairs' hole in the glass (#429) is wider than a lift hole: shots pass through it, and players on the glass level can fall in onto the steps.
  #437: the hole opens only as wide as the stairs (a visible change to the hole's outline, awaiting the user's OK).
- [?] **W-124 Four map demos.** "Have a team of sub-agents each try to make a demo analogous to the Rust wormholes
  hyperbolic space demo but for the other 4 maps. I want to have playable versions of each of the other 4 maps." The
  agents may need to get textures so the other maps don't look too bad; they can reuse the robot skins and guns. "You're
  going to make this all in demos, and you're just going to make one folder for each demo." "It's very important to
  have high performance for all of these." "Try to get to a working version as fast as possible for each of them, and
  especially try to get to a working end-to-end version as fast as possible while staying correct. So, you can take the
  ideas I've described and just do the simplest possible version of it. You can start with a very small map for each
  one, and then we're going to want to go on based on feedback." The four maps are W-119 (City Sphere demo), W-120
  (Snow Planet demo), W-121 (Dust demo) and W-122 (Dark Forest demo).
  Merged into `staging/user-4gatepylon` with #393, #396, #397, #399, 2026-11-08. Correctness checks of every level
  and their fixes: #419, #418, #428; frame times of every level on a real Mac: #422.
- [?] **W-119 City Sphere demo.** Part of W-124 (four map demos), the satellite city: "Ideally it can be like a sphere,
  where you walk and the roof is like glass or something, and then you can also go up onto the glass level, and
  there's also going to be like obstacles and stuff, but there's going to be no roof on the glass level. On the glass
  level, you can actually shoot directly across. Remember, of course, this is Euclidean space. And additionally, on
  the glass level, if you are on top of the glass, the laser or weapon shots will reflect using just like normal
  reflections off the normal. So the idea is that in the lower, below the glass, everything's just like a normal
  shooter. And above the glass, it's like this more interesting situation where laser reflects off the glass and then
  it also can also be shot directly across. And there should be a well defined parameter for how long it goes. I think
  usually it should be no more than two bounces at most."
  Merged into `staging/user-4gatepylon` with #397, 2026-11-08.
  - **Bugs found playing it, 2026-11-08:** "if you kill the enemy, they immediately respawn. That's not how it should
    work. They should not immediately respawn. I also think you should be able to run a little faster in space
    station. I separately cannot find how to get on top of the inner sphere, like I'm stuck in the outer sphere.
    There should be like stairs going up. That is extremely important because you need to make it to the pylons.
    Also, the mini map is just like a blank, it's just like doesn't have any mini map stuff. It should have like
    actual Cartesian lines and it should, um, like show things properly, I guess." Fixed in #429: respawn after the
    delay with "YOU DIED", sprint 21.6 m/s, stairs from in front of each team's spawns to the glass level, a minimap
    of the level with straight grid lines; with W-126 (same play in every level)'s City Sphere parity.
- [?] **W-120 Snow Planet demo.** Part of W-124 (four map demos): "There should be like a really snowy zone where
  you're like slowed and you start to lose health because you're like freezing. And it should be the case that
  there's like multiple of these. And if you shoot through them, like the shot has like a smaller distance it can
  pass. These can be like blizzard zones or something like that."
  Merged into `staging/user-4gatepylon` with #396, 2026-11-08. Speed: #465 gives it Lunar Surface's half-resolution
  lens (with #464's sharp centre, every bench view under 16.7 ms at 3440x1440; Snow's S14).
- [?] **W-121 Dust demo.** Part of W-124 (four map demos): "Dust has, by default, a fog of sorts, like there's kind of
  a sandstorm going on, and that should refract the light. There should be like a wind always happening, and the light
  should kind of bend in the direction of the wind. There will be no black hole there, but there will still be
  wormholes, and obviously, players can place wormholes. And obviously, that one's also hyperbolic, unlike the other
  ones."
  Merged into `staging/user-4gatepylon` with #393, 2026-11-08. Speed: #420 bakes the terrain's height, so every
  bench view is under 16.7 ms at 2560×1440 with the same pictures (merged, 2026-11-08). Lunar's jump time and
  walkable level mouths for W-126 (same play in every level): #427 (open, 2026-11-08).
- [?] **W-122 Dark Forest demo.** Part of W-124 (four map demos): "I want it to just be kind of just like a normal
  forest. This is just that one's going to be just kind of a normal first-person shooter. Maybe one difference there
  is just to have it have verticality, similar to the Star Wars Endor or whatever, maybe the Wookiee Planet, where
  there's like tree houses and stuff. Maybe have some like rope ladders and ways of going up trees. And crossing
  across them."
  Merged into `staging/user-4gatepylon` with #399, 2026-11-08. #426: robots, players and lasers show through the
  wormholes (merged, 2026-11-08). #439: playable and winnable end to end (merged, 2026-11-08). #445: Lunar Surface's
  movement, flashlight and exit prompt (open, 2026-11-09).
- [?] **W-123 Match progression menu.** "Make sure that the menus, when you see, like, there's a menu that shows the
  game progression, kind of the game history. Like, some state, I suppose, in the game server has to keep track of the
  game history and which team won what and what the wavefronts are, who has ownership, defensive ownership, offensive
  ownership, etc."
  Merged into `staging/user-4gatepylon` with #403, 2026-11-08. #417: `make play-all-levels`, a bot match steered
  through all five levels and checked (no stall, each level's result, the progression).
- [?] **W-125 Photo map page.** "I also really liked the 3 dimensional-looking visualization for the snow map, if
  you can for all maps (including lunar base) make the map as shown when you click tab and then e to get to map look
  not like some random 1950s drawing but instead a real like photo from the top that would be very nice (it could
  have some shadows, etc...) of course, because it doesn't change (it's just the map) it should be cheap to make it
  extremely beautiful". Here: the hyperbolic demo's maps (Lunar Surface and the others); the other levels do theirs
  in W-119 (City Sphere demo) to W-122 (Dark Forest demo). #409.
- [?] **W-118 Join any server.** "The client should be able to connect to any server": a player types a server's
  IP in the app ("I'm not entirely sure if it's possible to put the IP in the IP for the client"), so the
  downloaded app can join a local server now and the GCP server later. #405.
- [?] **W-42 Server in the cloud.** Run the headless game server in the cloud so players can join over the internet
  (the user's "launch from i.e. cloud in the near future"); today the game works on a local network only.
  The user asked on 2026-11-07: running servers so that people can play; it "does not need to be highly
  available or performant", it "just has to work in happy path with <= 20 concurrent players, i.e. across <= 3
  games". On GCP, "completely reproducible using either their Terraform format or some scripts (whichever is
  simplest and easiest to use)"; no git clone on the server ("locally build for the right target and then just
  scp or run a script to upload and do the Terraform"); ideally a "one click setup". On 2026-11-08: "share just
  the hyperbolic Rust demo on Mac", set up to be "slightly more generalizable" for a bigger game later, "but
  please do not overcomplicate this". #379, #381.
- [?] **W-116 Download page.** "A webpage people can go to download the app and an easy installation (and other
  stuff like it has a nice icon etc)"; "think as simple as possible". #379, #381, #405.
- [?] **W-58 Installable bundle.** Define the map in some kind of declarative form so that it can be put into an app
  bundle that other people can install or port without needing the GitHub repository; more generally, refactor
  things so there is an easy "bundle, then give the bundle to another person, and they can run it easily". Before
  starting, talk with the user about how to do it, rather than doing it in some random way.

  The user asked to fold in this plan, which another agent wrote, as part of this item: a **downloadable Mac app**.
  The game is played among friends: a friend downloads a zip, unzips it and runs the game on an Apple silicon Mac
  (macOS 13+), then hosts or joins a game on the same Wi-Fi, without a terminal, Rust or a checkout of the repo. Not
  for the App Store, and no paid Apple developer account.
  1. **Built-in maps:** build the maps (`common/maps/house_demo` and `lunar_surface`, about 136 KB) into the
     program. Today `get_map_folder` finds them through the path to the checkout, which is fixed at build time, so
     they aren't found on other Macs.
  2. **Title screen:** a double-clicked app gets no flags, so it opens on a screen with Host, Join (finds the game on
     the local network) and a map choice. Build it on the W-46 settings menu.
  3. **Logs and errors:** log to `~/Library/Logs/` instead of the terminal, and show fatal errors in a pop-up.
  4. **Version check:** put a version number in the join messages, and refuse a player whose version doesn't match
     the server's, with a clear message, since friends will download at different times.
  5. **App and zip:** a `make app` target that builds the .app (`Info.plist` with name, icon, minimum macOS 13, and a
     line explaining the local network use), signs it ad hoc (free), and zips it with `ditto -c -k --keepParent` so
     the app survives unzipping.
  6. **Local network permission:** as its own app, macOS asks the player whether the game may find devices on the
     local network. If the player refuses, the game says so clearly instead of failing silently.
  7. **First-open instructions:** without notarization, macOS blocks a downloaded app the first time. `PLAYTEST.md`
     and a short note in the zip tell players to open the app once, then click System Settings → Privacy & Security
     → Open Anyway.
  8. **Hosting the zip:** the repo is private, so friends can't download from its GitHub Releases unless added to
     the repo. Ask the user where the zip goes (for example a shared Drive or Dropbox link).

  Not part of this item: Apple notarization (needs the paid account), the App Store, Windows and Linux (the Metal
  renderer would need a wgpu port), Intel Macs, and play over the internet (W-42).

  On 2026-11-07 the user asked for all of W-58 to be done together with W-42 and W-116, and the zip is hosted on
  the W-116 download page; after W-79 the title screen offers only online games and joining on this Wi-Fi. #379.
- [?] **W-117 Server DoS protection.** "Bare minimum security/auth in the form of 'people can't DoS my server' or
  something like that." The user chose a game password with a challenge: the server sends a random challenge
  and the client answers with a keyed hash of the password, so the password never crosses the network, and every
  later message carries a per-player token. #379.
- [?] **W-108 Shoot whenever not moving.** "With a slight cooldown, you should be able to shoot as long as no keys
  are pressed. No movement keys I mean, so if you're in the air while time dilated you should be able to shoot."
  Loosens D50 (no shooting in the air) and D67 (shooting only standing still or crouched). #331.
Merged into a `staging/user-<username>` branch (before 2026-11-08, `staging`), waiting for the user to test and
confirm. Items done before this status existed (2026-11-07)
are here too, since the user hadn't confirmed them.

- [?] **W-94 Sketchfab models for the voxel assets.** "Replace the assets that were formerly voxels like the bunkers
  and spaceship" on Lunar Surface with these CC BY 4.0 Sketchfab models, crediting their authors, in the Rust game:
  1. "Stekelvarken S3 Kazemat" by sjoerdnijkerk (`f409a77deaaf4e3888d47f76ec48dbdd`) replaces the blue spaceship
     lander and its pad at the blue spawn, 3.5 m tall.
  2. "Desert Planet Bunker Garage Thing" by eKross (`ebe3177a1c7947b98328d8cab6aa8600`) replaces the red spawn
     hangar, 6 m tall.
  3. "WW2 Field Bunker" by goldenblack4 (`6ea5cebdaa82455aa3aad26baa6e0375`) replaces the two derelict habitat
     modules in the middle, 3.5 m tall.
  4. "Sci-Fi Beacon/Way Point Marker Free Model" by AMMediaGames (`a1d91ebb3e2d41bba31c02b11423d97f`) replaces the
     core and pad of both team pylons: 4 m tall on bare ground, the 5 consoles stay in a ring around it, and each
     beacon shoots a team-coloured beam 400 m straight up that dims as consoles are switched off.
  5. "Hesco Barrier PBR" by polbrainstorm (`ab21217481094e9a84f989e71fd56a59`) replaces the two red barriers in the
     middle: each is a 16 m row of 1.4 m Hesco blocks, keeping the 3 m gap in the middle.

  The user chose: ray-trace the real triangles; the laser hits the triangles while walking uses hand-fitted boxes;
  commit the models with textures downscaled (about 20 MB).
  #333 (Devin's commits, merged by Claude Code together with W-106 (Teal robot player model) at the user's request
  after Devin stopped, 2026-11-08; each team's three beacons are 12 m tall, one console each, beaming while it is on).
- [?] **W-106 Teal robot player model.** "Replace the player model with that [Teal V.2] and rig it with an animation":
  the Sketchfab model "Teal V.2" by Lagst (`d6a54ff9468c451bbdbd879afa03f20a`, Sketchfab Standard) replaces the
  cylinder and head drawn for every player's body (the others', and your own in the third-person view, W-61
  (third-person view)). The user chose: idle, walk, run, jump and death clips, keyframed in Blender on the model's own
  rig; lasers keep hitting the current box, not the triangles; the model is ray-traced with W-94 (Sketchfab models for
  the voxel assets)'s triangle renderer. #333.
- [?] **W-107 Time dilation.** "Get rid of the world warping mechanic. Instead we're going to build time dilation. The
  more pylons you disable, the more the black hole at the center grows. Then, when you get closer to the black hole,
  time slows down time dilation style." Replaces W-68 (reality bend)'s warping of the map into a sphere. After
  playing: "make the time dilation more intense all around, and then make the effect also be that when the black hole
  gets bigger it's the black part grows a little but mostly the distortion area grows"; the user said "this looks
  good" (D108). #329.
- [?] **W-104 Sky flashes when shooting.** "There are some very odd bugs, ex when shooting the sky flashes" (in the
  reality bending version). #316.
- [?] **W-96 A different rifle model.** "For the rifle the gun has to change to a different one ideally; see if you
  can find something for that." The user picked Poly Haven's bolt-action rifle from the pictures ("bolt action is
  good"): gun 2 is held as it, drawn at 0.28x so its scope, bolt and stock show; the sniper keeps the revolver
  (D106). #304. Then: "make a quick PR to switch which gun model is used where; we accientally got them flipped":
  the sniper (gun 1) now holds the bolt-action rifle and the rifle (gun 2) the energy revolver. #318.
  - **Where the work is** (for any agent picking it up): merged from branch `claude/rifle-model` (pushed to
    `origin`), worktree `/Users/4gate/git/Warped/.claude/worktrees/agent-a74067ef49c6e1c4e` on the user's Mac,
    pictures in
    `/private/tmp/claude-501/-Users-4gate-git-Warped--claude-worktrees-wizardly-hamilton-23a121/6463b76f-4842-4353-8c38-5e49bbbd5dcd/scratchpad/shots/W-96/`
    (temporary, under `/private/tmp`; the `W-96_final_*` files show the result). State, 2026-11-08: on `staging`,
    waiting for the user to test it.
- [?] **W-68 Reality bend.** Place three pylons on each side; each pylon is a "disabler" for the black hole in the
  center. The more pylons get disabled, the bigger the strength of the black hole gets, causing reality to warp around
  it: the stronger the black hole gets, the more the edges of the map start to warp up into a sphere. With only one
  pylon left, the map is completely projected onto the inside surface of a sphere. On Lunar Surface itself, with one
  console per pylon; the black hole rises to the sphere's centre, and a team loses when all three of its pylons are
  down. #298, #299, #300, #301. The warping into a sphere was removed by W-107 (time dilation), #329.
- [?] **W-37 Stronger curvature.** The curvature can be set in the YAML settings, higher than now: twice the current
  curvature by default (and at most twice). After seeing 2x speckle far from the map's middle, the user chose 1.5x
  as the default: "1.5x curvature is fine". #285.
- [?] **W-99 Players in team colours.** A bug found while taking screenshots: a player's figure is coloured by their
  player number, not their team (a red-team player can be drawn blue, a blue one yellow), while the minimap and MAP
  page use team colours. The user said to add and fix it ("Yes, add and fix"). #290.
- [?] **W-83 Giant ships in the distance.** Add a Death Star and a Star Wars Star Destroyer-looking spaceship off in
  the distance. To be built as original designs in that spirit (a moon-sized battle station with a dish, a long
  wedge-shaped capital ship), not copies of the films' models, and without their names in the code. From two options
  of each, the user picked "Death Star option B" (the dish in the upper half) and "ship option A but you can add a
  bunch of lights on it like B" (D101). #303.
  - **Where the work is** (for any agent picking it up): merged into `staging` in #303 from branch
    `claude/giant-ships` (pushed to `origin`; worktree `/Users/4gate/git/Warped/.claude/worktrees/agent-a58a7f92ff135229f`
    on the user's Mac), pictures in
    `/private/tmp/claude-501/-Users-4gate-git-Warped--claude-worktrees-wizardly-hamilton-23a121/6463b76f-4842-4353-8c38-5e49bbbd5dcd/scratchpad/shots/W-83/`
    (temporary, under `/private/tmp`). State, 2026-11-08: done and on `staging`; waiting for the user to test.
- [?] **W-03 Playtest guide.** Setup and the commands to play together, with known caveats; `main` official,
  `staging` the latest. #92, #94, #98, #99.
- [?] **W-36 Lobby.** A lobby page where players see how many players are on each team, choose a team, and then
  play as that team; friendly fire stays on, and only the enemy's consoles can be turned off. #131.
- [?] **W-98 Milky Way.** Have a Milky Way in the skybox ("add to wishlist having a milky way", "in teh skybox").
  From three looks, the user picked C, photo-like ("get milky way photo like merged"): drawn in code, arching across
  the sky from base to base (a placement chosen for the user), its brightness in `player_settings.yaml` (D102).
  #317.
  - **Where the work is** (for any agent picking it up): merged from branch `claude/milky-way` (pushed to `origin`),
    worktree `/Users/4gate/git/Warped/.claude/worktrees/agent-a6ce576a32709d775` on the user's Mac, built by Claude
    Code and finished by Devin; final pictures in
    `/Users/4gate/git/Warped/demos/hyperbolic_wormholes_rust/.artifacts/wishlist_pictures/W-98/final/`. State,
    2026-11-08: on `staging`, waiting for the user to test it.
- [?] **W-102 Rifle sways when jogging.** The rifle goes side to side when jogging, like a normal rifle ("add to
  wishlist having rifle going side to side when jogging like a normal rifle"). From before and after pictures (the
  sprint pose already tilted it), the user chose the sway as built ("ok menrge to staging", "i like your animation");
  after #318 swapped the gun models, "I want them both swaying ... get both working realistically based on their
  size": both guns slide 2 cm walking and 4 cm sprinting in step with the head bob, and swing by their length, the
  1.23 m rifle 1.7° to 3.4° and the 0.30 m revolver a quarter of that ("merge this") (D107). #324.
  - **Where the work is** (for any agent picking it up): merged from branch `claude/rifle-sway` (pushed to `origin`),
    worktree `/Users/4gate/git/Warped/.claude/worktrees/agent-a74067ef49c6e1c4e` on the user's Mac, started by
    Claude Code and finished by Devin; pictures in
    `/Users/4gate/git/Warped/demos/hyperbolic_wormholes_rust/.artifacts/wishlist_pictures/W-102/both_guns/`. State,
    2026-11-08: on `staging`, waiting for the user to test it.
- [?] **W-134 Dust map page picture stale.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): Dust's committed MAP page picture (`map_pictures/dust_r50m.png`) was drawn before the ground switched to 3 noise octaves; a fresh render differs in 6.5% of its pixels.
  #440: redrawn with `make make-map-picture MAP=dust`.
- [?] **W-131 Lunar wormhole_exit slow at full size.** A bug found by agents on 2026-11-08 and not fixed (the user: "for non-trivial bugs you can just note them and mention them in wishlist"): Lunar Surface's `wormhole_exit` bench view takes about 109 ms of GPU time at full 2560x1440 (the reference and the match's copy alike); in play the window's render scale drops to keep frames fast, but the view itself is far over 16.7 ms.
  #450: Lunar Surface's ground is baked and each ray tests less; 19 of 21 bench views now take at most 16.6 ms at
  2560x1440 at full resolution (`wormhole_exit` 109 → 49.5 ms, still over; in play the render scale drops there).
  #454: safe terrain steps and the beam's haze between samples, 2-6% more; `laser_into_wormhole` 16.6 ms.
  #458: exact ground hits far from the origin, shadow safe steps, blocks of groups (D112): at 3440x1440, 19 of 21
  views under 16.7 ms (`red_spawn` 21.0 → 14.5 ms); `wormhole_exit` (57.7 ms) and `laser_into_wormhole` (18.9 ms) left,
  with options for the lens (half resolution, longer bent steps) in the PR for the user.
  #462: the user's option D (half-resolution lens, Dust's longer bent steps, D114): at 3440x1440 `wormhole_exit` 57.7
  → 24.5 ms and `laser_into_wormhole` 18.6 → 13.1 ms; 20 of 21 views under 16.7 ms.
  Mouth B over the red landing pad is what keeps `wormhole_exit` slow (its rays test the pad's 14 shapes at every bent
  step); moving it beside the pad (branch `claude/lunar-mouth-b`, not merged) measured 24.1 → 15.5 ms. The user kept
  mouth B on the pad (2026-11-09) rather than move it: "you can allow that specific location to be a little bit
  slower, its fine".

## Done

Confirmed by the user after testing.

- [x] **W-41 Play across computers.** Check that people on two or more computers on the same Wi-Fi can play
  together ("the user can play the game with other humans on the same wifi"); so far it has only been tested on one
  Mac over loopback.
  Confirmed by the user, 2026-11-09: "it ahs been tested between multiple players on mac and it owrks".

- [x] **W-111 Crash shooting a wormhole ball.** A bug: "if you shoot a wormhole ball (before creating a wormhole) you
  get a crash". The first thrown ball drops on the ground, the second one also, and they become a wormhole when the
  second touches the ground; shooting a ball before then crashes the game. The user, 2026-11-08: "you should be
  able to shoot your own purple ball and have it do nothing", "if you shoot enemy purple ball its destroyed" (one
  hit, for a ball that is down), "document that in game mechanics for lunar base map". The crash is fixed by
  #362 (Leah, 2026-11-08); branch `devin/wormhole-throw-fixes`, 2026-11-08,
  makes one hit destroy an enemy's lone mouth and documents both. Confirmed by the user, 2026-11-08: "great it
  seems to work".
- [x] **W-112 No third throw.** "You should not be able to throw at all after 2." Today "you can throw more than 2,
  but currently after the 2nd throw it does nothing and disappears": the third `Q` press throws a ball that does
  nothing. Contradicts `game_design/DESIGN.md` ("Thrown wormholes"), where the third press throws the purple mouth
  again and moves it. The user, 2026-11-08: "first click throws purple wormhole, second one throws green (causing
  them to connect and transform into real wormholes) and third doesnt do anything". Fixed on branch
  `devin/wormhole-throw-fixes`, 2026-11-08: once both mouths
  are down `Q` does nothing until death or destruction clears them; `DESIGN.md` updated to match. Confirmed by the user, 2026-11-08: "great it seems to
  work".
- [x] **W-110 Team-coloured robots.** "is it possible for you to recolor the teal robot? to be other colors than
  teal?", then "yep team colors!": the players' body of W-106 (Teal robot player model) in each team's colour, blue
  or red, instead of teal; corpses and your own body in third person too. #344. Confirmed by the user, 2026-11-08: "looks good merge it".
- [x] **W-77 Loading and connected.** Show a loading bar or circle (the window takes a few seconds to load), and say
  when the server is connected properly. The user asked for the loading screen "in the same style as the progress
  bar while disabling a pylon". Loading screen: #292, #293; connected: the lobby says CONNECTING, then CONNECTED,
  #307. Confirmed by the user, 2026-11-08: "i love hte loading screen", then "looks good".
- [x] **W-93 Larger player icons on the minimap.** "Make the player icons larger on the minimap." #253. Confirmed by the user, 2026-11-07: "larger player icons. and throw wormholes and shoot enemy wormholes are done".
- [x] **W-60 Throw wormholes.** Each player has a purple and a green wormhole. `Q` throws one, then the next press
  throws the other. These are each player's linked wormholes. #197–#204. Confirmed by the user, 2026-11-07: "larger player icons. and throw wormholes and shoot enemy wormholes are done".
- [x] **W-89 Shoot enemy wormholes.** "If you shoot enemy wormholes it makes the wormhole flash red, and after a
  few shots (5 rifle shots) the wormholes are both destroyed." Builds on W-60 (#197–#204). You chose: a shot through
  an enemy's mouth (or at a lone mouth's ball) counts, 2 sniper shots, damage kept until destroyed, both mouths flash
  (D98). #246–#248. Confirmed by the user, 2026-11-07: "larger player icons. and throw wormholes and shoot enemy wormholes are done".
- [x] **W-86 Dust as small polyhedra.** For the ground-shot dust (W-56, #208): make the particles polyhedra (they can
  be hardcoded) rather than spheres, make the big ones smaller, and make slightly less dust. The user OKed the
  screenshots before it merged ("dust polyhedra looks good plz merge"). #234. Confirmed by the user, 2026-11-07: "dust as small polydra is good so it should be masrked as done".
- [x] **W-71 Shadows on the hills.** Fix the shadow bugs on the hills: there is horrible aliasing, and random, ugly
  shadows on the hills. #233. Confirmed by the user, 2026-11-07: "shadows on hills is good enough so it shouledbe marked as done".
- [x] **W-91 One laser on the minimap.** A bug: when you shoot, the minimap shows two lasers instead of one ("like
  a triangle of some kind"). #243. Confirmed by the user, 2026-11-07: "one laser on minimap seems fixed so shoudl be marked as done".
- [x] **W-04 Click to shoot.** #93. Confirmed by the user, 2026-11-07: "click to shoot should be marked as done if not done".
- [x] **W-05 Laser crash and invisible lasers.** No crash when a laser hits a player on Lunar Surface, and everyone
  sees every laser. #95. Confirmed by the user, 2026-11-07: "laser crashed is fixed so it shouled be marked as done".
- [x] **W-06 No wall climbing.** Walking along a wall doesn't push the player up to its top. #96. Confirmed by the user, 2026-11-07.
- [x] **W-07 PRs into `staging`.** PRs go to `staging` (agents may merge there); only humans merge into `main`, or an
  agent when a human explicitly asks. #97. Confirmed by the user, 2026-11-07.
- [x] **W-08 Jump to climb.** Players need `Space` to get onto a block; no sliding up blocks. #100. Confirmed by the user, 2026-11-07.
- [x] **W-09 Walk on structures.** Players above the lowest ground level walk without jumping; jumping is only for
  going up a level. #101. Confirmed by the user, 2026-11-07.
- [x] **W-10 Respawn countdown.** A "YOU DIED" message and a visible respawn timer. #102. Confirmed by the user, 2026-11-07.
- [x] **W-11 Movement feel.** A head bob, walking and running twice as fast, a sprint energy bar as in the levels,
  and three times the gravity. #103. Confirmed by the user, 2026-11-07.
- [x] **W-12 Gun on screen.** A simple moving gun that jogs when running and points forward when walking; no
  shooting while running. #104. Confirmed by the user, 2026-11-07.
- [x] **W-13 Only the game's keys.** WASD to move, `Space` to jump, click to shoot; `E` no longer jumps, and the keys
  that gave an edge are gone. #105. Confirmed by the user, 2026-11-07.
- [x] **W-14 Smaller energy bar, more head bob.** Energy runs out four times faster; a deeper head bob. #106. Confirmed by the user, 2026-11-07.
- [x] **W-15 No running on an empty bar.** A player can run only with energy left. #107. Confirmed by the user, 2026-11-07.
- [x] **W-16 No shooting while jumping.** #107. Confirmed by the user, 2026-11-07.
- [x] **W-17 Snappy jumps.** Gravity that feels three times as strong: quick jumps and fast falls. #107. Confirmed by the user, 2026-11-07.
- [x] **W-18 Headless server.** A game server without a window that prints its address; every player, the launcher
  included, joins as a client. #108. Confirmed by the user, 2026-11-07.
- [x] **W-19 Win by consoles.** Hold `E` on the other team's consoles to turn them off. Consoles pulse with bright
  light while on, twice as fast while being turned off, and go dark once off. When a team's consoles are all off the
  game ends and says who won, and the server's launcher starts a new game. #110, #115. Confirmed by the user, 2026-11-07.
- [x] **W-20 3D wand.** A prettier gun: a rectangular prism with a coloured tip that turns like a real object. #111. Confirmed by the user, 2026-11-07.
- [x] **W-21 Motion blur.** A tiny bit. #112. Confirmed by the user, 2026-11-07.
- [x] **W-22 Second gun.** The design's rifle as gun 2, looking different from the wand, switched with the mouse
  wheel or `1`/`2`. The game design says how a player with two guns switches and which keys do what. #113. Confirmed by the user, 2026-11-07.
- [x] **W-23 Laser from the tip.** The laser leaves from the wand's tip. #114. Confirmed by the user, 2026-11-07.
- [x] **W-24 Consoles turn off.** Fix: finishing a hold on a console did nothing (a server running another map).
  #115. Confirmed by the user, 2026-11-07.
- [x] **W-25 Rifle recoil.** #116. Confirmed by the user, 2026-11-07.
- [x] **W-26 Settings in YAML.** Speeds, head bob, shots per second and the other game numbers in a well-organised,
  easy-to-understand YAML file that the server takes with a flag and sends to every player. #117. Confirmed by the user, 2026-11-07.
- [x] **W-27 Flying beams.** The laser disappears from the tip onward and flies forward like a bullet, lasting longer
  for the sniper than the rifle; speed and trail per gun in the YAML. #118. Confirmed by the user, 2026-11-07.
- [x] **W-28 Many players, apart.** Several players at once, never spawning one on top of another. #119. Confirmed by the user, 2026-11-07.
- [x] **W-29 Health bar.** A discrete health bar so players see they were hit; the ammo and energy bars in segments
  too. #120. Confirmed by the user, 2026-11-07.
- [x] **W-30 Pink sky noise.** Fix the flickering pink noise flooding the sky when shooting into the sky or at the
  black hole's edge. #122, #124. Confirmed by the user, 2026-11-07.
- [x] **W-31 Hit effects.** A player who is hit is slowed (set in the YAML), sees red and a blur come in from the
  screen's edges, and the screen shakes. #123. Confirmed by the user, 2026-11-07.
- [x] **W-32 Crouch.** `Ctrl` crouches; walking crouched is slower; players can shoot crouched but not while walking
  upright. #125. Confirmed by the user, 2026-11-07.
- [x] **W-33 No respawn by choice.** #126. Confirmed by the user, 2026-11-07.
- [x] **W-34 Crosshair.** #127. Confirmed by the user, 2026-11-07.
- [x] **W-43 Hold for the flashlight.** The flashlight stays on only while the right button is held. #133. Confirmed by the user, 2026-11-07.
- [x] **W-39 Key bindings.** Which key crouches, moves, jumps, switches weapons and so on can be set in YAML
  (now the `keys` of `player_settings.yaml`, each player's own). #135. Confirmed by the user, 2026-11-07.
- [x] **W-45 Player and server settings.** The YAML settings are split into `player_settings.yaml`, each
  player's own, and `server_settings.yaml`, the server's. #140. Confirmed by the user, 2026-11-07.
- [x] **W-46 Settings menu.** A screen to exit to a menu where a player changes their own choices (hotkeys and the
  rest of the per-player settings): `Tab` opens it. #143. Confirmed by the user, 2026-11-07.
- [x] **W-49 Menu with pages.** The menu where players change their hotkeys and the rest is a general menu with
  pages: one page for the hotkeys, another showing a layout of the entire map (`Q`/`E` turn pages). #149. Confirmed by the user, 2026-11-07.
- [x] **W-59 Smooth crater instead of voxels.** Get rid of voxels and replace them with a proper 3D world: an uneven
  lunar surface texture from an online source, the Lunar Surface level inside a crater whose walls are the level's
  walls, a smooth noise-based environment with no voxels at all, smooth shapes for the objects, and the ridge, peak
  and boulders as hills and mounds in the terrain. #147. Confirmed by the user, 2026-11-07.
- [x] **W-62 Sniper ammo bar.** The sniper gets an ammo bar too, but of only one block instead of many: it empties
  when the sniper fires and comes back when the timer resets, letting the player shoot again (a 2 s reload,
  `sniper.reload_s`). #156. Confirmed by the user, 2026-11-07.
- [x] **W-38 Soft map edge.** Instead of a wall at the map's edge, players get slower and slower as they move off the
  map, so they can never leave it, and things stretch longer and longer: a relativistic barrier rather than a block.
  Since the map update (W-59), the hills at the edge may block players from leaving: the "Zeno's paradox" slowdown
  should start when a player climbs the hill, to stop them from leaving. As they slow down, their view of the world
  should warp so it looks as if they are being stretched ever further. #159. Confirmed by the user, 2026-11-07.
- [x] **W-48 Minimap.** A minimap that shows off the map's hyperbolic nature nicely. Its place can be picked at
  least from top or bottom and left or right. Players show on it, and so do laser shots that fall in its range; it
  doesn't show the entire map. It is a circle that shows just where the player is and what is around them (the
  menu's map page, W-49, shows the whole map). #166. Confirmed by the user, 2026-11-07.
- [x] **W-66 Your range on the map page.** The map in the menu shows a circle around where the player is, with a
  radius (the rest is slightly darker and blurred), and shows where they are on the map. #166. Confirmed by the user, 2026-11-07.
- [x] **W-55 Aiming.** Players can aim (right click by default), so the flashlight moves off right click to a
  different key. The right button zooms in; the flashlight is on `F`. #170. Confirmed by the user, 2026-11-07.
- [x] **W-57 Laser stuck in a room.** In one of the rooms on the red side, the one with windows one block wide,
  shooting out from close to a window hits some sort of hitbox (or looks like it) and the laser doesn't leave the
  room. Not a bug: the "hitbox" is the window's glass pane, and glass stops lasers by design (D88). #174. Confirmed by the user, 2026-11-07.
- [x] **W-63 Missing textures.** Get the proper textures, and/or fail fast when textures are missing: "the aquarium's
  fork merge into this forgot to git add the textures and I think they'll be added soon but they are still
  missing". The game fails when they are not there ("it's fine if they are not committed"), and "the error (client
  side) should say what to do to fix it". #168 committed them in `demos/fps_demo/assets/`; #173 stops the client at
  start with the command that restores a missing one. Confirmed by the user, 2026-11-07.
- [x] **W-65 Confirm before exiting.** Pressing `Escape` doesn't exit straight away: the game asks "Do you want to
  exit?", exits if the player presses OK, and otherwise doesn't. #175. Confirmed by the user, 2026-11-07.
- [x] **W-67 Consoles pulse faster and faster.** While a console is being turned off, its pulsing gets faster
  continuously, reaching a fervor right before the console turns off. If the turning off stops (a "disconnect"), the
  pulsing slows down smoothly instead of jumping back. #176. Confirmed by the user, 2026-11-07.
- [x] **W-52 Cyan through a wormhole.** Going through a wormhole turns the whole screen somewhat cyan, simulating a
  relativistic effect on colour if possible. #178. Confirmed by the user, 2026-11-07.
- [x] **W-53 Spaceships overhead.** A few spaceships flying overhead in the sky, purely background world-building, to
  look cool and pretty. #193. Confirmed by the user, 2026-11-07.
- [x] **W-54 Professional HUD bars.** Make the health, ammo and other bars look more professional: at the very least
  slightly translucent, slightly shiny and rounded. Show the user a few options to pick the best from before
  implementing. The user picked "slanted tactical but with the neo background but also make them slightly slightly
  more translucent than the neon example", and asked: "also ideally also make them pulse when in use (i.e. when
  taking damage the health bar pulses with a reddish orange pulse traveling downwards, when you run a blue pulse goes
  from right to left (down), ammo is normal though; another pulse (but much more subtle) occurs when you are
  regaining things such as energy" (D85). #207. Confirmed by the user, 2026-11-07.
- [x] **W-56 Dust from ground shots.** Shooting the ground throws some mist and dust particles up into the air
  (D81). #208. Confirmed by the user, 2026-11-07.
- [x] **W-72 Can't join a team.** A bug, somewhat intermittent it seems: pressing `1` or `2` doesn't join a team.
  It was a join sent to a port no server listened on; windows now say when a server doesn't answer, is full or runs
  another version. #209. Confirmed by the user, 2026-11-07.
- [x] **W-78 Shooting from the team screen.** A bug: you may be able to shoot a laser before leaving the screen where
  you press `1` or `2` to choose a team. Players can shoot only once the server has let them in. #209. Confirmed by the user, 2026-11-07.
- [x] **W-79 Only serve and join.** "Make a commit to disable everything except starting a server and connecting to
  a server; we will not be using the local demo etc." `make serve-game` and `make join-game` remain. #212. Confirmed by the user, 2026-11-07.
- [x] **W-69 Shot noise.** Each laser shot strays a little, by a random amount sampled for each shot. With the rifle the
  noise grows as the player keeps shooting; with the sniper the amount is sampled from an exponential distribution.
  All of it is in the YAML settings. #216. Confirmed by the user, 2026-11-07.
- [x] **W-81 Crouching recharges faster.** "Having crouch recharge walking faster": if you crouch, the energy for
  running is recharged at the same rate as when you run (drains it); if you walk, it is recharged at half that. This
  is the default in the YAML, and in general it should be parameterized. #219. Confirmed by the user, 2026-11-07.
- [x] **W-73 Gun lit by the scene.** There's an issue with the gun where the gun is uniformly lit and not reactive to
  scene lighting. On voxel maps the gun takes the light at the eye: the sun (dark in shadow), lamps, earthshine and
  the ground. #211. Confirmed by the user, 2026-11-07.
- [x] **W-70 Windows to see and shoot through.** Since the map update (W-59) the habitat modules' and walkways'
  windows are glass panes laid over a solid wall, so they show the wall, not the outside, and glass stops lasers
  (W-57, #174). Asked whether windows should be see-through and let lasers through, the user answered "Both". #222. Confirmed by the user, 2026-11-07.
- [x] **W-80 Smoother bar pulses.** The pulsing should start where the bar's filled part ends: losing health, it starts
  at the last hit point, not the bar's end (empty slots don't count). The blue pulse going left should be less dark,
  and it too starts on the last filled slot. Pulses shouldn't light whole segments: they should look more Gaussian
  and more subtle, at a faster pace in general, and in proportion to how fast the value is changing (smoothed). The
  user wants samples first. From the samples the user picked "soft swells except that it is also rate based (and
  make the swell really soft, i.e. really wide gaussian like; think like smoother convolution vibes)" (D94). #231. Confirmed by the user, 2026-11-07.
- [x] **W-87 No colourful spaceship.** Remove the colourful spaceship from the sky (W-53, #193): "the circular space
  station one is fine and great, but the colourful ones look ugly". That is the container freighter; the ring station
  stays, and so do the grey fighters unless the user says otherwise. #232. Confirmed by the user, 2026-11-07.
- [x] **W-88 Delta fighters.** "Make the fighters look more like delta shaped instead of with such thin arms, go
  simple and geometric." Pictures first; the user picked "A wide delta". #242. Confirmed by the user, 2026-11-07.
- [x] **W-95 Credits page.** A credits page in the `Tab` menu that says where the assets came from, and a rule in this
  demo's `AGENTS.md` that any new asset gets added to that page. #262. Confirmed by the user, 2026-11-07: "the credits page looks good yes".
- [x] **W-50 Scoreboard page.** A menu page that shows how many players there are, and each player's kills, deaths
  and team. #164. Confirmed by the user, 2026-11-07: "kd stuff also looiks good so add that as complete as well".
- [x] **W-97 Scores page first.** "Change the page ordering also so kd is the default thing to show up": the `Tab`
  menu opens on the SCORES page. #265. Confirmed by the user, 2026-11-07: "scores page change looks good, mark it as done in a pr".
- [x] **W-75 No spaceship on the blue side.** Get rid of the spaceship on the blue side. The landing pad stays, since
  the blue spawn stands on it; W-94 (Sketchfab models for the voxel assets) replaces the pad with a model later.
  #210. Confirmed by the user, 2026-11-07: "no spaceship on blue side looks good mark it as done".
- [x] **W-92 Lightsaber lasers, rifle first, guns further out.** "Make the lasers look more like this [a photo of a
  lightsaber blade: a white blade with a pale blue glow]. Lighter haze, almost totally uniform on the blade of the
  laser. Make the rifle the default weapon. Make you hold both weapons a little further from the camera so you can
  see more of the weapon on screen. And the laser should be touching the tip of the rifle." #256. After playtesting
  #256: "move the gun back so the handle of the gun is offscreen. it was fine how it was before, just make the gun
  larger." #263. Then: "I see Leah made the lightsaber thing. That's OK. Let's do lightsaber for the sniper and do
  the old laser view for the rifle." The rifle's beam is the laser again in #249, with W-84 (shot noise in the beam
  itself). Confirmed by the user, 2026-11-07: "lightsaber lasers is done, mark it as done".
- [x] **W-90 Other players on the map.** Show where other people are on the map. The menu's MAP page shows every other
  player in the game as a dot in their team's colour; the minimap already showed those in reach. #250. Confirmed by the user, 2026-11-07: "other players on map is done mark it as done".
- [x] **W-76 Better scene lighting.** Make better scene lighting: add a slight atmosphere/haze (even though
  it's the moon, which doesn't make sense), and soften shadows. Also bloom ("like i also want bloom"). #214. After playtesting #214: "turn down the haze. i
  don't like the haze. can you actually just turn off the haze in the near distance i want just the distance to be
  hazy. everything is overexposed too", and "these changes have made the game much much slower". #226. Then: "can you add a little bit more haze". #238. Confirmed by the user, 2026-11-07: "better scene lighting is done, mark it as done".
- [x] **W-01 Fast Rust renderer.** The wormholes demo in Rust and Metal at 60+ fps at full resolution, on the house
  and on Lunar Surface. #73, #76, #81–#86, #90, #91. Confirmed by the user, 2026-11-07: "mark fast runst render er as done".
- [x] **W-02 Multiplayer on one network.** Host and join a game on the same Wi-Fi, on `house_demo` and
  `lunar_surface`, with lasers that eliminate players. #77, #79, #80. Confirmed by the user, 2026-11-07: "mark multiplayer on one network as done".
- [x] **W-35 Flashlight.** Right-click turns on a flashlight that shines in a neat cone. #129. Confirmed by the user, 2026-11-07: "mark flashlight as done".
- [x] **W-84 Shot noise in the beam itself.** The user's correction of W-69: "by shot noise I mean that the laser BEAM
  itself has shot noise (i.e. it looks almost kind of buggy or like a bunch of small shiny particles)". The amount is
  sampled per shot, grows as the rifle keeps shooting, and is sampled exponentially for the sniper, all in the YAML
  (W-69's own words). W-69 (#216) built shots that stray from the crosshair instead. After samples, the user chose
  speckled pixels that turn black, white or drop out to show the background, "a combination of black AND dropout AND
  white", then "stick with 118 and goes from amount 0 to 1 as you fire more, exponential average should be at 0.2",
  then "Let's do lightsaber for the sniper and do the old laser view for the rifle. ALSO add the SPECKLE on top of
  that with the 1/1/8 and 0.0-0.5 ratio with the 0.2 mean for exponential distribution for sniper". #249. Confirmed by the user, 2026-11-07: "shot noise is good".
- [x] **W-74 Sharp-edged shapes.** Get rid of the rounded corners on the map's assets (e.g. the buildings): some
  things are cylinders but are kind of blobby, with rounded corners on the top and bottom. If something is a
  cylinder, just make it a cylinder. #210. Confirmed by the user, 2026-11-07: "sharp edged shpes is done".
- [x] **W-82 Wormhole colour from motion.** Fix the wormhole transition colouring (W-52 (cyan through a wormhole),
  #178): it should be a more bluish cyan, NOT greenish cyan. It should NOT be a function of time but of space and
  velocity: as long as you are in the wormhole and moving, it shows up blue (based on your velocity and your place in
  the wormhole); it should NOT go down with a timer. Once you're out of the wormhole it should go away immediately.
  Walking backwards should show a redshift instead. The user wants video or image demos to confirm. #259. After
  testing #259: "You cannot start the transformation so early when going into the wormhole, it should happen only
  for a split second as you are in it. It happens when you are INSIDE it", and "the transformation is still DARK
  BLUE but it has to be very close to white as I had asked". #272. Confirmed by the user, 2026-11-07: "wormhole color from motion is almost done: just mark it as done and make a PR that makes it take slightly longer; ideally just double its radius or wahtever (ioe walking at same speed should take exactly trwice as long of time showing me cyan or red)".
  After seeing #272's pictures: "for the "at the throat" thing you still need to have the dark red and the light
  cyan almost white instead of doing this "warm" stuff. It has to be CRISP (not like diffused by this white screen),
  like the previous pictures I was shown. Still fix the throat timing for the color though", "so overall your throat
  timing looks good", and "its just the specific colors should be a bit more crisp". #289. The user's picks from the
  colour sheets: "i like option 8 for the wormhole", and "for red i like r2" (branch `claude/wormhole-doppler-picks`).
  #271.
  - **Where the work is** (for any agent picking it up): branch `claude/wormhole-tint-crisp`, pushed to `origin`,
    PR #289 into `staging` (crisp colours, and the tint showing twice as long: 0.63 s against 0.32 s at walking
    speed). Worktree `/Users/4gate/git/Warped/.claude/worktrees/agent-acc2c2d6bf5b447c3` on the user's Mac; pictures
    in
    `/private/tmp/claude-501/-Users-4gate-git-Warped--claude-worktrees-wizardly-hamilton-23a121/6463b76f-4842-4353-8c38-5e49bbbd5dcd/scratchpad/shots/W-82/`
    (`fix3_*`), which is temporary (it is under `/private/tmp`). State, 2026-11-08: merged into `staging`.
- [x] **W-85 Stuck leaving a bunker.** A bug: if you try to exit one of the bunkers, you get stuck. Still happening on
  staging at #225 ("i click w and cannot get out"). #237. Confirmed by the user, 2026-11-07: "also stuck in bunker is fixed so mark that as done".
- [x] **W-61 Third-person view.** A way to switch to a third-person view: the camera zooms out to slightly behind the
  character, and the player controls it from there, seeing their own character. `V` switches to a view from 3 m
  behind and 0.6 m above the head, pulled in when a wall or the ground is behind it; shots go from the head into the
  crosshair, and aiming looks from the eye again. #286. Confirmed by the user, 2026-11-08: "third person looks good its done mark at such".
- [x] **W-64 Zone planes.** The painted lines that mark the edges of the blue and red zones become planes jutting out
  of the ground, just barely translucent, to mark out the real zones. Then, from the pictures: "Level A, faint", and
  "the blates should also have a little bit of thickness (rougly 1 player worth of thickenss)". #291. Confirmed by the user, 2026-11-08: "please merge the fact that the walls are odne", "(zone planes are done)".
