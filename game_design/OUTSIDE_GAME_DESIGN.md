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

## Open questions

- **Free allowance:** whether "1 match or 30 minutes, whichever comes first" is the rule, or whichever comes last.
- **Unpaid accounts:** whether an account past its free allowance may still chat with friends.
- **Spectators:** whether a tournament game limits how many people watch it.
