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
| Party | 1 to 5 players who queue together and always land on the same team. |
| Queue | The players and parties waiting for matchmaking to form a match. |
| Rating window | How far apart in rating matchmaking lets a party and its opponents be. It widens as the party waits. |
| Ready check | The moment each of the ten players confirms they are there, before a matchmade match starts. |
| Ranked match | A matchmade match or a tournament game. Only ranked matches change ratings. |
| Skill | The game's best estimate of how well a player plays, in points. |
| Uncertainty | How unsure the game is of a player's skill, in points. It shrinks as the player plays and grows while they are away. |
| Rating | The number a player is ranked by: their skill minus their uncertainty. |
| Placement | A player's first ranked matches of a season, while their rating is still hidden. |
| Tier | A named band of ratings, such as Gold. |
| Season | A stretch of about three months, after which rankings start afresh. |
| Rankings | The list of players by rating, which anyone can read. |
| Reconnect window | The time a player who dropped out of a ranked match has to come back. |
| Abandon | To leave a ranked match for good: by choice, by not reconnecting in time, by going idle or by being kicked. |
| Cooldown | A time during which a player can't join the queue, given for abandoning or for failing a ready check. |
| Void | To cancel a match so that it changes no rating and gives no penalty. |
| Check | One rule the game tests a player's actions against, during the match or afterwards. |
| Audit | A replay of part of a match's recording, after the fact, that tests exactly whether a player's actions were possible. |
| Report | A player's complaint about another player, with a reason. |
| Suspension | A stop on an account's online play until a superadmin has reviewed it. |
| Ban | A superadmin's or the game's decision that an account may not play online, or may not chat, for a time or for good. |
| Block | One player's choice to hear nothing more from another player. |
| Notice | A message from the game to a player, kept in the app's inbox. |
| Friend | A player another player has agreed to be friends with. |
| Clan | A standing group of players with a name and a tag, managed by the shared [roles](#roles). |
| Profile | The page that shows one player's name, icon, tier, badges and statistics. |
| Commendation | One player's thanks to another after a match. |
| Presence | Whether a friend is offline, online, in the queue or in a match. |

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
  [Matches that don't count](#matches-that-dont-count)).
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

## Matchmaking

Matchmaking forms ranked matches of five against five on the official map and settings. It is the only way to
play a ranked match, apart from tournament games.

### Parties

- A player queues alone or in a party of up to 5. A party's players are always on the same team.
- The party's leader starts and stops the search. Everyone in the party must be able to play: signed in on an app
  the servers accept, past no free allowance without the purchase, not banned, and under no cooldown.
- **Rating gap:** in a party of 2 to 4, the highest and lowest ratings are at most ≈500 apart, so that a strong
  player can't carry a weak friend up the ranks. A full party of 5 has no limit, because it chooses all of its own
  teammates.
- **A party's rating:** matchmaking treats a party as one rating: the mean of its players' ratings, moved ≈25% of
  the way toward its best player's. The lean makes up for the edge that playing together gives.

### The queue

There is one queue, so that the players online find each other as fast as they can.

1. Every ≈5 s, matchmaking takes the party that has waited longest and looks for nine more players around it.
2. It only takes parties whose rating is inside the first party's rating window. The window starts at ≈±100 and
   grows by ≈50 every 10 s the party waits, up to ≈±500. Matchmaking never matches players further apart than
   that.
3. It prefers parties of the same sizes on both teams, a party of 3 against a party of 3, and fills the rest with
   players who queued alone.
4. It only takes players who can play in the same region: each player's delay to that region is at most ≈120 ms.
   Of the regions that fit, it picks the one where the slowest player's delay is lowest.
5. With ten players, it tries every way to split them into two teams of five that keeps parties whole, and takes
   the split where the two teams' ratings are closest. If the weaker team's chance to win is below ≈40% in every
   split, it drops the match and keeps looking.
6. Which team is blue and which is red is random.

While waiting, a player sees how long they have waited and an estimate of how much longer.

### The ready check

- When matchmaking has ten players, each of them gets ≈20 s to confirm.
- If all ten confirm, the match's server starts in the chosen region, and the players go to its
  [warm-up](#warm-up).
- If anyone declines or doesn't answer, there is no match. That player's party leaves the queue, and the player
  gets a short cooldown that grows if they do it again (see [Cooldowns](#cooldowns)). The other parties go back
  into the queue with the time they had already waited.
- If the caps are full, the confirmed match waits in the wait queue ahead of new custom games, and the players
  see their place.

### Too few players: bots

- A party that has waited ≈2 minutes without a match is offered a match with bots. The offer names how many human
  players it would have.
- A party that accepts can be put in a match where bots fill the empty places, the same number of bots on each
  team where that is possible. The bots play at about the level of the human players' ratings.
- A match that starts with bots is not ranked: it changes no rating and counts for no leaving penalty.
- A party that declines keeps waiting for ten human players. If there aren't enough, it gets no match.
- A match with bots needs at least ≈2 human players. A player alone is pointed to practice against bots in a
  local game.

### After the match

- The players see the result and how their rating changed (see [Rating](#rating)).
- A party stays together and can queue again at once. Players who met in the match can form a party for the next
  one.

### Later, without changing these rules

- **Opening hours:** the queue is always open at launch. Superadmins gather players at set times with
  announcements. A setting for the queue's opening hours can be added later.
- **More queues:** an unranked queue, or one per game type, can be added when there are enough players to fill
  them. Each queue would follow the steps above with its own settings.
- **Tournaments:** at launch a superadmin sets up each tournament game by hand. Brackets that start the games
  themselves can be added on top of tournament games.

## Rating

A rating says how good a player is at winning ranked matches. It moves only in ranked matches, by the match's
result and the ratings of the other nine players.

### What a rating is

- The game keeps two numbers for each player: a **skill**, its best estimate, and an **uncertainty**, how unsure
  it is. A new account has a skill of ≈2,000 and an uncertainty of ≈1,000.
- A player's **rating** is their skill minus their uncertainty, so a new account's is ≈1,000. The rating is
  cautious on purpose: a few lucky wins don't put a new player at the top, and it rises as the game grows sure.
- Matchmaking compares **skill**, not rating, so a strong new player meets strong opponents after a few matches.
  Wherever [Matchmaking](#matchmaking) says rating, it means skill.

### What changes it

- **Only ranked matches:** matchmade matches and tournament games. Custom games, local games and matches that
  start with bots change nothing.
- **The team's result:** everyone on the winning team gains and everyone on the losing team loses. A match that
  runs out of time is a tie, which moves the two teams toward each other. Kills and deaths don't matter.
- **The other players:** a win against a team with higher skills gains more than a win against a weaker team,
  and losing to a weaker team costs more.
- **Uncertainty:** a player the game is unsure about moves further per match. Each match shrinks the player's
  uncertainty.
- **The formula can change:** Warped starts with OpenSkill, a rating method made for teams. The game keeps every
  ranked match's result, so that all ratings can be worked out again if the method changes.

### Placement

- A player's first ≈10 ranked matches are placement matches. Until they are played, the player has no tier, isn't
  in the rankings, and sees "Placement, match 3 of 10" instead of a rating.

### Time away

- After ≈14 days without a ranked match, a player's uncertainty grows a little each week, up to a new account's.
  Their skill doesn't change, so their rating falls slowly and recovers within a few matches.
- After ≈30 days without a ranked match, a player leaves the rankings until they play one.

### Tiers

A tier is a band of ≈200 rating points with a name, shown next to the player's name and icon.

| Tier | Rating |
| --- | --- |
| Bronze | below ≈1,200 |
| Silver | ≈1,200 to 1,400 |
| Gold | ≈1,400 to 1,600 |
| Platinum | ≈1,600 to 1,800 |
| Diamond | ≈1,800 to 2,000 |
| Master | ≈2,000 to 2,200 |
| Warped | ≈2,200 and above |

### Seasons

- A season lasts ≈3 months.
- When a season ends, each player keeps a badge of the highest tier they reached, shown on their profile.
- When the next one starts, every player keeps their skill and gets back ≈half of a new account's uncertainty.
  They play ≈5 placement matches, and the rankings start empty.

### Rankings

- The rankings list players by rating, highest first: name, icon, tier, rating, ranked matches played and share
  of them won.
- Anyone can read them on the web without signing in, and in the app. Nobody changes them by hand.
- A player is listed once they have finished this season's placement, unless they are banned or have been away
  ≈30 days.
- The app also shows the rankings among a player's friends and inside their clan.

### Match history

- Each player sees their past online matches: the kind of game, the map, the result, both teams, each player's
  kills and deaths, how their rating changed, and the replay.

## Leaving a ranked match

These rules follow Counter-Strike's. They apply to ranked matches only: leaving a custom game costs nothing.

### Dropping out and coming back

- A player whose app or connection fails can rejoin the same match. The app offers it as soon as it is back.
- The player has ≈3 minutes in total per match to be away. While they are away, their team plays one short.
- A player who gives no input for ≈3 minutes is removed from the match, as if their connection had failed.

### Abandoning

A player abandons a match when they choose to leave it, use up the reconnect window, or are kicked by their team.

- **The first to abandon on a team** takes a loss, whatever the match's result, and a [cooldown](#cooldowns).
- **Their teammates** may then leave freely: a teammate who leaves after that takes no cooldown, and the match
  doesn't change their rating.
- **A bot takes the place** of each player who abandoned, playing at about that player's level, and the match
  stays ranked for everyone still in it.
- **The other team** is not freed by this. Each team has its own first player to abandon.

### Matches that don't count

- **Nobody connects:** if a player never connects during the [warm-up](#warm-up) of a matchmade match, the match
  is voided, the others go back into the queue, and that player gets a cooldown.
- **Server failure:** if the match's server fails, the match is voided for everyone.

### Cooldowns

- Abandoning gives a cooldown of ≈30 minutes, then ≈2 hours, ≈24 hours and ≈7 days for each further time.
- Failing a ready check gives ≈2 minutes, then ≈10 minutes, then ≈30 minutes.
- A player's next cooldown drops back one step for every ≈7 days without an offence.
- A cooldown is given whether or not the player meant to leave, as in Counter-Strike.
- During a cooldown the player can't join the queue. They can still play custom and local games.

## Votes in a ranked match

- **Kick:** a player calls a vote to kick a teammate and must pick a reason: cheating, abuse, not playing, or
  another they type. The kick passes with ≈3 yes votes from the 4 other teammates, the caller's included. A kicked
  player has abandoned. Each player calls at most ≈1 kick vote per match.
- **Surrender:** once a teammate has abandoned, a team can vote to surrender. If every human player left on the
  team votes yes, the match ends as a loss for that team and a win for the other, and counts in full.
- **Pause:** each team may pause the match ≈1 time for ≈60 s, for example while a teammate reconnects.

## Rollbacks

- When an account is banned for cheating, its ranked matches of the last ≈30 days are voided: every other
  player's rating is worked out again as if those matches hadn't been played.
- The players of those matches are told (see [Notices](#notices)).

## Fair play

No game stops every cheat. Warped makes cheating hard to do, likely to be caught, and costly when it is: a ban
loses the account, its purchase and its rating.

### What the game checks

- **The server has the last word.** A player's app works out its own movement and shots, and the match's server
  accepts only what its checks allow. Hits, eliminations, consoles, scores and results are the server's alone.
- **Checks during the match** are cheap and never stop an honest player:

  | Check | What it refuses |
  | --- | --- |
  | Movement bounds | Moving faster than the game allows, through walls, or staying in the air too long. |
  | Shot rules | Shooting faster than the weapon fires, without ammo, or from where the player isn't. |
  | Line of fire | A hit on a player the shooter's place can't reach, by a table worked out per level. |
  | Shot path | A hit whose path crosses solid blocks, checked on a coarse copy of the level. |

  A refused action doesn't happen: the server puts the player back, or ignores the shot.
- **Hidden players stay hidden.** The server sends an app only what its player could see or hear, so a changed
  app has nothing to show through walls.
- **Only players get in.** A match's server talks only to the apps of the players admitted to that match, over a
  connection that nobody else can read or forge.
- **Checks are a list.** Each check says what it looks at, whether it runs during the match or afterwards, and
  what failing it does. Adding a check changes no other rule here.

### Audits

An audit replays a stretch of a match's recording exactly, which is too slow to do during the match, and tests
whether what one player did was possible.

- **All audits run afterwards**, apart from the match. They never slow a match down.
- **What gets audited:** a random ≈5% of ranked matches, every player with a new report for cheating, and every
  player the game's statistics single out. Players who are in a match right now are audited first.
- **Failing:** the first failed audit gives the player a warning. After ≈2 failed audits the account is banned
  for cheating, and removed from its match if it is in one.
- **Statistics:** the game records numbers such as accuracy, reaction time and how often a player aims at
  opponents they can't see. At launch these only choose whom to audit. Later, a system may ban on them directly.

### Reports

- A player reports another from the match, the match's history, a profile or a chat, and picks a reason:
  cheating, abuse in chat or voice, spoiling the match on purpose, an offensive name, or being under 13.
- A player sends at most ≈10 reports a day. Reporting the same player twice for the same match counts once.
- **Automatic suspension:** an account is suspended until a superadmin reviews it when, within ≈90 days:
  - ≈90 different players have reported it for cheating, in different matches; or
  - it was kicked for cheating from ≈10 different games, by different players.

  Only different reporters in different matches count, so that a group can't suspend someone by reporting them
  again and again.
- **Review:** a superadmin reads the reports, watches the replays and the audits, and either lifts the suspension
  or bans the account.

### Bans

| Kind | What it stops | Given by |
| --- | --- | --- |
| Cooldown | Joining the queue, for minutes to days | The game, for abandoning (see [Cooldowns](#cooldowns)) |
| Game ban from one custom game | Joining that game | That game's admins |
| Chat ban | Text and voice chat, for a time or for good | A superadmin |
| Suspension | All online play, until reviewed | The game |
| Ban | All online play, for a time or for good | A superadmin, or the game after ≈2 failed audits |

- A ban belongs to the account. A banned account gets no refund.
- **Appeals:** a banned or suspended player can appeal ≈1 time per ban, in writing. A superadmin answers.

### Notices

The app has an inbox of notices from the game. A notice tells a player:

- that an audit of theirs failed, as a warning;
- that they are suspended or banned, why, for how long, and how to appeal;
- that a player from one of their ranked matches was banned for cheating, and how their rating was restored. This
  notice is sent after the ban, never on suspicion;
- that a report of theirs led to action;
- what a superadmin announces to everyone.

### Chat and voice

- A filter hides offensive words in text chat, unless the reader turns it off.
- A player mutes anyone, for one match or for good, in text and in voice.
- **Blocking:** a blocked player can't message, invite or talk to the player who blocked them, and matchmaking
  avoids putting the two on the same team when it can.
- Text messages are kept ≈30 days so that a superadmin can review a report. Voice is not recorded.

## Friends, parties and clans

### Friends

- A player sends a friend request by name, from a match's scoreboard or history, or by a link. The other player
  accepts or declines. Either one ends the friendship at any time.
- A player sees each friend's presence: offline, online, in the queue, or in a match and which kind. A player can
  appear offline.
- **Recent players:** the app lists the players of a player's last matches, so that a good teammate is easy to
  find again.

### Parties

- Any player forms a party and invites friends or recent players, up to 5 in all. The one who formed it is its
  leader, who can pass the lead on and remove players.
- A party queues for a matchmade match together (see [Matchmaking](#matchmaking)), or its leader takes it into a
  custom game, where each player still picks a team.
- A party has its own text and voice chat, which carry on between matches.

### Clans

- A clan has a name, a tag of 2 to ≈5 letters shown before its players' names, and up to ≈50 members.
- A clan follows the shared [roles](#roles): its founder, its admins, and its members, who are readers. Admins
  invite and remove members.
- A clan has its own text chat and its own [rankings](#rankings) of its members.
- A player belongs to at most ≈1 clan.

## Talking

| Channel | Who is in it | Text | Voice |
| --- | --- | --- | --- |
| Direct | Two friends | Yes | No |
| Party | The party | Yes | Yes |
| Clan | The clan | Yes | No |
| Team | A team, during a match and its warm-up | Yes | Yes |
| Match | Both teams, during the warm-up and after the match ends | Yes | No |
| Game lobby | Everyone in a custom game's lobby | Yes | No |

- **Outside matches** a player talks with friends, their party and their clan from anywhere in the app.
- **Voice** is for the party and the team. A player picks push-to-talk or an open microphone, and mutes anyone.
- **During play** opposing teams don't talk: the match channel is open only before the match and after it.
- Pings and the other signals a player gives inside a match are part of `INSIDE_GAME_DESIGN.md`.
- The rules against abuse are in [Chat and voice](#chat-and-voice).

## Profiles

- A profile shows a player's name, icon, clan, tier, the badges of past seasons, commendations, and statistics:
  matches played, share won, kills and deaths, and the levels they win most on.
- A player opens their own [match history](#match-history) from it, and others see as much of it as the player
  allows.
- **Commendations:** after a match, a player commends up to ≈3 others who played in it, for teamwork, for
  leading or for being good company. A profile shows how many of each the player has. Players can't commend
  themselves, and commendations from the same party count once.

## Notifications

The app tells a player, wherever they are in it, and through the computer's notifications when it is in the
background:

- a friend request, and an invitation to a party, a custom game or a clan;
- a direct message;
- that the ready check has started, with a sound;
- that their place in the wait queue has come up;
- a new [notice](#notices).

## Privacy settings

Each player chooses:

| Setting | Choices |
| --- | --- |
| Who may send friend requests | Everyone, friends of friends, nobody |
| Who may send direct messages | Friends, nobody |
| Who may invite to parties and games | Everyone, friends, nobody |
| Who sees the profile's statistics and match history | Everyone, friends, only the player |
| Presence | Shown to friends, or always offline |

A player's name, icon, tier and place in the rankings are always public.

## Settings that follow the player

A player's own settings, such as key bindings, mouse sensitivity and team colours, are saved to their account, so
they are the same on every computer the player signs in on.

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
- the [rankings](#rankings), read-only;
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
| Matchmaking | Players in a party | 1 to 5 |
| Matchmaking | Largest rating gap inside a party of 2 to 4 | ≈500 |
| Matchmaking | How far a party's rating leans toward its best member | ≈25% of the gap to its mean |
| Matchmaking | How often matchmaking looks for a match | ≈5 s |
| Matchmaking | Rating window at the start | ≈±100 |
| Matchmaking | Rating window growth | ≈50 every 10 s |
| Matchmaking | Largest rating window | ≈±500 |
| Matchmaking | Least and most win chance a team may have | ≈40% and ≈60% |
| Matchmaking | Largest delay to a region a player is matched into | ≈120 ms |
| Matchmaking | Time to answer the ready check | ≈20 s |
| Matchmaking | Wait before bots are offered | ≈2 min |
| Matchmaking | Fewest human players in a match with bots | ≈2 |
| Rating | A new account's skill and uncertainty | ≈2,000 and ≈1,000, so its rating starts at ≈1,000 |
| Rating | Placement matches in a player's first season | ≈10 |
| Rating | Placement matches in later seasons | ≈5 |
| Rating | Time away before uncertainty grows | ≈14 days |
| Rating | Time away before a player leaves the rankings | ≈30 days |
| Rating | Width of a tier | ≈200 rating points |
| Rating | Length of a season | ≈3 months |
| Rating | Uncertainty given back at a new season | ≈half of a new account's |
| Leaving | Reconnect window, in total per match | ≈3 min |
| Leaving | Idle time before a player is removed | ≈3 min |
| Leaving | Cooldowns for abandoning | ≈30 min, 2 h, 24 h, 7 days |
| Leaving | Cooldowns for failing a ready check | ≈2 min, 10 min, 30 min |
| Leaving | Time without an offence that lowers the cooldown one step | ≈7 days |
| Votes | Yes votes needed to kick, of the 4 other teammates | ≈3 |
| Votes | Kick votes one player may call in a match | ≈1 |
| Votes | Pauses per team in a match, and their length | ≈1 of ≈60 s |
| Rollback | How far back a cheater's matches are voided | ≈30 days |
| Fair play | Failed audits before a ban | ≈2 |
| Fair play | Share of ranked matches audited at random | ≈5% |
| Fair play | Different reporters, in different matches, before a suspension | ≈90 |
| Fair play | Kicks for cheating, in different games by different players, before a suspension | ≈10 |
| Fair play | Time over which reports and kicks are counted | ≈90 days |
| Fair play | Reports one player may send per day | ≈10 |
| Fair play | Days chat messages are kept for review | ≈30 |
| Fair play | Appeals per ban | ≈1 |
| Friends and clans | Most friends | ≈200 |
| Friends and clans | Most members of a clan | ≈50 |
| Friends and clans | Clans one player belongs to | ≈1 |
| Friends and clans | Length of a clan's tag | 2 to ≈5 letters |
| Commendations | Commendations a player gives per match | ≈3 |
| Chat | Days direct messages are kept | ≈30 |

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
- **Bots in the queue:** the fewest human players a match with bots needs, and whether a party that accepted bots
  still gets a full human match when one turns up.
- **Party gap:** whether the largest rating gap inside a party should be counted in rating points or in tiers.
- **Blocked players:** whether matchmaking should also keep a blocked player off the other team.
- **How much a match counts:** whether a rating change depends only on win, loss or tie, or also on how far the
  winner's wavefront went.
- **Playing on with a bot:** whether a team that lost a player to an abandon, and loses, should lose less rating.
- **Surrender:** Counter-Strike lets a team surrender only after a teammate abandoned, as far as I recall;
  confirm that Warped should copy that condition, and how many votes a surrender needs.
- **Kicks by a party:** a party of four can vote out the fifth player. Whether a player kicked that way should be
  spared the loss and the cooldown.
- **Tier names:** the names here are placeholders.
- **Reconnect window:** Counter-Strike's exact window isn't published; ≈3 minutes is a first guess.
- **Counting window:** whether reports and kicks should be counted over ≈90 days, over a season, or for ever.
- **Kicks that count:** whether kicks by a custom game's admins count toward a suspension, or only vote kicks in
  ranked matches.
- **Naming the cheater:** whether the notice after a ban names the banned player.
- **Voice:** whether voice chat is ever recorded for review. This design records none.
- **Clans:** what a clan is for beyond chat and its own rankings (clan matches, a clan rating), and whether a
  player may be in more than one.
- **Voice:** whether voice is on by default or push-to-talk only, and whether opposing teams ever hear each other.
- **Profiles:** which statistics a profile shows, and whether a player may hide theirs.
- **Commendations:** whether they give anything beyond a number on the profile.
