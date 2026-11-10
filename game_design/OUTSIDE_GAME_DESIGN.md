# Warped outside-game design

This document designs everything a player meets outside a match: accounts, paying, the kinds of game, custom
games, matchmaking, rating, fair play and friends. [`INSIDE_GAME_DESIGN.md`](INSIDE_GAME_DESIGN.md) designs the match
itself (rules, levels, movement, weapons, the screen), and this document uses its terms: team, match, map, level,
round. How the cloud builds these rules (services, databases, APIs) is the system design, which comes after this
document is agreed.

- **≈** marks a first guess. Every such number is a setting that superadmins change without a new release.
- **TBD** marks something not designed yet.
- **Built** marks a rule the game already has. Everything else here is designed but not built yet.

## Terms

| Term | Meaning |
| --- | --- |
| Account | One player's identity in Warped, tied to one Google account. |
| Purchase | The one payment that unlocks online play for an account, for good. |
| Free allowance | The online play a new account gets before it needs the purchase. |
| Matchmade match | A ranked match that matchmaking forms from the players in the queue. |
| Custom game | A match that a player starts from a saved map, for the players they choose. It is unranked. |
| Tournament game | A custom game that a superadmin starts. It is ranked and can be watched live. |
| Local game | A match that runs on the player's own computer: the tutorial, practice against bots, or play on one Wi-Fi. |
| Game type | The rules a match is played by. Launch has one, the match of `INSIDE_GAME_DESIGN.md`. |
| Superadmin | A person the developers appoint to run the whole game. |
| Region | Where a match's server runs: US West or US East. |
| Cap | The most matches, and the most players, that play online at once. |
| Wait queue | The line a player stands in while the caps are full. |
| Saved map | A map in the sense of `INSIDE_GAME_DESIGN.md` (a line of levels) together with the game settings to play it with, kept under a name so games can be started from it. |
| Creator | The player who made a saved map. |
| Root admin | The player who started a custom game. Starting a game from someone else's saved map makes a player the game's root admin, not the map's creator. |
| Admin | A player the founder or another admin gave the power to manage a saved map, a custom game or a clan. |
| Founder | The one member of a shared thing who can never be removed: a saved map's creator, a custom game's root admin. |
| Whitelist | The accounts that may join a custom game, when its admins restrict it. |
| Warm-up | The time before a match starts, when players connect, try the levels and wait for the others. |
| Spectator | Someone who watches a match live, flying through it unseen. |
| Replay | The recording of a match, which the app plays back from any viewpoint. |
| Clip | A short video a player makes from a replay. |
| Checkpoint | The saved state of a custom game that stopped, from which it can start again. |

## Accounts

- **Sign-in:** a player signs in with Google. Warped has no passwords of its own. One Google account is one Warped
  account.
- **Age:** at sign-up the player enters their birth date. Warped refuses anyone under 13, and keeps only that the
  player confirmed being 13 or older and when, not the date. An account that turns out to belong to someone under
  13 is deleted with its data.
- **Countries:** Warped is not offered in the European Union. Sign-up refuses a player whose connection is there,
  and the terms say so.
- **Name:** each account has one display name that no other account has. The player picks it at sign-up and may
  change it once every ≈30 days. A filter refuses offensive names and names that imitate superadmins.
- **Icon:** the player picks an icon from a set the game ships with. Players don't upload pictures.
- **One session:** an account plays in one place at a time. Signing in on a second computer signs the first one out.
- **Signing out:** a player can sign out of one computer, or of every computer at once.
- **Deleting:** a player can delete their account. Its name, icon, friends and messages go. Its past matches stay
  in other players' histories under a placeholder name, so their ratings still add up.
- **Terms:** signing up means accepting the terms of service, the privacy policy and the code of conduct. The terms
  say the payer is an adult or has a parent's permission.

## Paying

- **One purchase:** online play costs one payment of ≈$13.37 per account. There is no subscription, and nothing
  else is sold at launch.
- **Where:** the game opens a Stripe payment page in the browser. Warped never sees the card. The account unlocks
  as soon as Stripe confirms the payment.
- **Free allowance:** a new account plays online for free until it has finished ≈1 match or played ≈30 minutes of
  matches, whichever comes first. Warped never interrupts a match for payment: the block applies the next time the
  player tries to join one.
- **What the block stops:** joining the queue, joining or starting a custom game, and watching a tournament game
  live. The player can still sign in, play local games, read the rankings, and see their profile and replays.
- **No passes:** nobody plays online past the free allowance without their own purchase. A player who paid can't
  invite a friend in for free.
- **Refunds:** a refunded or disputed payment locks the account's online play again. A banned account gets no
  refund.
- **Local games are free:** the tutorial and practice against bots run on the player's computer and need no
  purchase.

## Where and on what

- **Platforms:** macOS at launch. Nothing in this document depends on the platform, so that Windows and Linux are
  added by shipping their apps.
- **Download page:** a public web page offers the newest app for each platform. **Built** for macOS.
- **Updates:** the app checks for a newer version when it starts, and updates itself when the player agrees.
- **Versions:** a match's server accepts the newest app and the one before it. An older app is told to update
  before it can play online. **Built:** a server refuses an app whose messages don't fit, and the app says SERVER
  RUNS ANOTHER VERSION.
- **Regions:** match servers run in US West and US East. Accounts, friends, ratings and rankings are the same in
  both. The app measures its delay to each region; matchmaking uses it, and a custom game's root admin picks the
  region.
- **Caps:** at most ≈100 matches and ≈1,000 players are online at once, across both regions. Local games and
  spectators don't count.
- **Wait queue:** while a cap is full, a player who tries to play waits in line, first come first served, and sees
  their place. A player already in a match is never removed to make room.

## Kinds of game

| | Matchmade match | Custom game | Tournament game | Local game |
| --- | --- | --- | --- | --- |
| Who starts it | Matchmaking | Any player | A superadmin | The player |
| Who plays | Whoever the queue matched | Whoever the root admin lets in | Whoever the superadmin lets in | The player, bots, players on the same Wi-Fi |
| Ranked | Yes | No | Yes | No |
| Map and settings | The official map and settings | Any saved map the starter may use | Any saved map | Any map on the computer |
| Watched live | No | If an admin turns it on | If an admin turns it on | No |
| Replay | For its players | For its players | For anyone | None |
| Bots | Only with every player's consent, or to replace a leaver | As the settings say | As the settings say | Yes |
| Needs the purchase | After the free allowance | After the free allowance | After the free allowance | No |
| Counts toward the caps | Yes | Yes | Yes | No |

- **Ranked means matchmade.** A player can't make a ranked game of their own, because friends could trade wins in
  it. Tournament games are the one exception: they are rare, and a superadmin sets each one up by hand.
- **Local games:** the tutorial is a local game that follows a script teaching curved space, alone or with bots.
  **Built:** playing on one Wi-Fi, joining a server by its address, and bots.
- **Game types:** every match names its game type, and every rule in this document that depends on the match's
  rules (who won, how long it lasted) reads them from the game type. Adding a game type later changes no rule here.

## Roles

Saved maps, custom games and clans share one set of rules for who may do what, so a player learns them once.

| Role | May do |
| --- | --- |
| Founder | Everything an admin may, and delete or close the thing. Nobody can remove or demote the founder. |
| Admin | Everything a writer may, and add, demote or remove admins, writers and readers. An admin can remove another admin, but never the founder. |
| Writer | Change the thing. |
| Reader | See and use the thing, without changing it. |

- **One founder:** the founder is the player who made the thing. A saved map's founder is called its creator, and a
  custom game's founder its root admin.
- **Leaving:** anyone can leave, the founder too. When the founder leaves, the admin who has held the role longest
  becomes the founder. With no admin left, a custom game closes, and a saved map stays usable by its readers but
  nobody can change it.
- **Granting:** a member grants only roles at or below their own, so an admin makes admins, writers and readers.
- **Two things, two sets of roles:** a role on a saved map says nothing about a game started from it. The map's
  creator has no power in that game unless its root admin gives them one.

## Saved maps

A saved map is what a custom game is started from: which levels the match plays, in which order, and the game
settings it plays them with.

- **Levels:** the creator picks levels from the ones the game ships with and puts them in order. The order follows
  the map rule of `INSIDE_GAME_DESIGN.md`: an odd number of levels, one purple level in the middle.
- **Game settings:** the saved map holds a value for every game setting the game has, such as gravity, friendly
  fire, the time to turn off a console, the match's time limit and the respawn times. A new saved map starts from
  the official settings.
- **Who sees it:**

  | Visibility | Who may use it |
  | --- | --- |
  | Private | Only its creator. |
  | Shared | The players the creator or an admin names, each as a reader, a writer or an admin. |
  | Public | Every player, as a reader. |

- **Using one:** a reader starts custom games from the saved map without its creator being there. A writer also
  changes its levels and settings. An admin also shares it.
- **Copies:** a reader can copy a saved map into a new one of their own, which they are the creator of.
- **Changes don't reach running games:** a custom game takes its own copy of the saved map when it starts. Changing
  or deleting the saved map later changes no game that already started.
- **The official map:** the developers keep one public saved map, the official one, with the default map and
  settings of `INSIDE_GAME_DESIGN.md`. Matchmade matches always play it.
- **Building levels:** players can't build levels yet. When they can, a level is shared by the same roles and
  visibilities as a saved map, and a saved map may use any level its creator can read.

## Custom games

A custom game is a room that a player opens from a saved map. It plays one match after another until it closes.

### Starting one

- A player picks a saved map they can read, a name for the game and a region, and starts it. That player is the
  game's root admin.
- An account runs at most ≈1 custom game at a time, because every game uses a server.
- If the caps are full, the game waits in the wait queue before it opens.

### Who gets in

| Setting | What it does |
| --- | --- |
| Listed | A listed game shows in the game list. An unlisted one is reached only by its link or code. |
| Password | Players must enter it to join. **Built:** a game on the internet has a password. |
| Whitelist | Only the accounts on it may join. |
| Ban list | The accounts on it may not join. Kicking a player can add them to it. |
| Most players | Up to 12, the most a match holds. **Built.** |

- **Link and code:** every custom game has a link and a short code. A friend who opens the link, or types the code,
  goes straight to the game's lobby.
- **Game list:** a player browses the listed games by name, saved map, region, how many players are in, and
  whether a password is needed.
- **Teams:** each player picks blue or red in the lobby, up to the team's size. **Built.** Players arrange the
  teams among themselves and with the admins.

### What its admins may do

- kick a player, with or without banning them from this game;
- set the password, the whitelist, the ban list and the most players;
- change the game settings, such as physics values and win conditions, for the matches still to come;
- turn watching on or off and set its delay (see [Watching](#watching));
- start the match, and stop it;
- add and remove admins, by the [roles](#roles) above.

Only the root admin closes the game.

### When it stops

- **Idle players:** a player who gives no keyboard or mouse input for ≈3 minutes is removed from the game.
- **Idle games:** a game where no player has given input for ≈10 minutes stops, and so does a game open for ≈4
  hours. Stopping frees its server.
- **Checkpoints:** a game that stops this way keeps a checkpoint: its saved map copy, its settings, its admins and
  lists, and the match's state (the rounds played, each team's wavefront, the score). An admin starts the game
  again from its checkpoint within ≈30 days, and the match goes on from the start of the round it stopped in.

## Warm-up

Every online match starts with a warm-up, as in Counter-Strike.

- **What players do:** they connect, pick up their team, and move and shoot freely. A player can jump to any of
  the match's levels to try it. Nothing in the warm-up counts.
- **The timer:** everyone sees how long the warm-up still lasts.
- **Matchmade matches:** the warm-up lasts until all ten players have connected, then counts down ≈10 s and
  starts the match. It lasts at most ≈5 minutes: a player who hasn't connected by then has left the match (see
  Rating, in a later section).
- **Custom and tournament games:** the warm-up lasts until an admin starts the match.

## Watching

- **Off unless turned on:** nobody watches a match live unless an admin of that game turns watching on. Matchmade
  matches have no admin, so nobody watches them live.
- **Delay:** the admin who turns watching on sets how far behind the match the spectators see it, from 0 s to ≈5
  minutes, so that a spectator can't help a player.
- **Who watches:** the admin chooses: only the players they name, anyone with the game's watch link, or anyone.
- **What a spectator does:** a spectator flies through the levels unseen and can't touch the match. They see what
  the match shows, with the delay, and follow the match from level to level.
- **Where:** in the app, and in a web browser.
- **Spectators and players don't talk:** a spectator can't send messages to the players of the match.

## Replays and clips

- **Recording:** every online match is recorded as the game's states, not as video, so a replay is small and plays
  back from any viewpoint.
- **Who can watch a replay:** the players of the match, in the app. A tournament game's replay is open to anyone.
- **Replay links:** a player of the match can make a link to its replay. Anyone with the link watches it.
- **Clips:** a player marks a stretch of a replay, up to ≈60 s, and the app renders it to a video on their
  computer, which they keep or share as a link.
- **Versions:** a replay plays in an app that still knows the version it was recorded with. A clip is a video and
  plays anywhere.

## Superadmins

- The developers appoint superadmins, and only the developers can.
- A superadmin can start tournament games, ban and unban accounts, review reports and audits, change every setting
  marked ≈ in this document, and post announcements that every player sees.
- The game keeps a log of every superadmin action: who did what, when, and the value before and after.
- **Maintenance:** a superadmin can stop new matches from starting, so that the matches being played finish before
  the servers are updated.

## Public web pages

Anyone can open these without signing in:

- the download page;
- the rankings, read-only (see Rating, in a later section);
- the terms of service, the privacy policy and the code of conduct;
- the status page, which says whether sign-in, matchmaking and each region work.

## Settings

Every number marked ≈ in this document, in one place.

| Section | Setting | Value |
| --- | --- | --- |
| Accounts | Days between name changes | ≈30 |
| Paying | Price of the purchase | ≈$13.37 |
| Paying | Free allowance, matches | ≈1 |
| Paying | Free allowance, time in matches | ≈30 min |
| Where and on what | Most matches at once | ≈100 |
| Where and on what | Most players at once | ≈1,000 |
| Where and on what | App versions a server accepts | The newest and the one before it |
| Custom games | Custom games one account runs at once | ≈1 |
| Custom games | Most players in one game | 12 (**Built**) |
| Custom games | Idle time before a player is removed | ≈3 min |
| Custom games | Idle time before a game stops | ≈10 min |
| Custom games | Longest time a game stays open | ≈4 h |
| Custom games | Days a checkpoint is kept | ≈30 |
| Warm-up | Longest warm-up of a matchmade match | ≈5 min |
| Warm-up | Countdown once everyone has connected | ≈10 s |
| Watching | Spectator delay an admin may set | 0 s to ≈5 min |
| Watching | Most spectators in one game | ≈20 |
| Replays | Days a replay is kept | ≈90 |
| Replays | Longest clip | ≈60 s |

## Open questions

- **Free allowance:** whether "1 match or 30 minutes, whichever comes first" is the rule, or whichever comes last.
- **Unpaid accounts:** whether an account past its free allowance may still chat with friends.
- **Spectators:** whether a tournament game limits how many people watch it.
- **A founder who leaves:** whether "the longest-serving admin takes over" is right for saved maps and custom games.
- **Changing settings mid-match:** whether custom-game admins may change every game setting during a match, or
  some only between matches.
- **Public saved maps:** whether anyone may publish a saved map to every player, or a superadmin approves it first.
- **Replays:** how long replays are kept, and whether a player can hide their own replays from the others in the
  match.
