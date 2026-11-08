---
name: compatible-changes
description: Change things that outlive one build of a game - a player's saved settings file, the bytes game windows and servers send each other, the server settings sent to players, and map files - without breaking what users already have or what another player still runs. Use whenever you add, remove or rename a setting or a key binding, change what the network sends (a snapshot, a message, a bit), or change the map format or a map file.
---

# Compatible changes

Some things live longer than the build that wrote them: a player's saved settings, the messages between a server and
windows built from other commits, and map files. A change that forgets them breaks the user's game the moment they
pull, or a friend's game while they haven't. (In the Rust demo this nearly happened twice: W-82 (wormhole colour from
motion) removed a setting, and W-61 (third-person view) added a key binding, either of which would have stopped the
user's saved settings from loading.)

## Saved player settings

The menu saves each player's settings to `~/.config/warped/player_settings.yaml` (or `--player-settings`), and the
game rejects a file with an unknown or a missing key (fail fast). So in the Rust demo's `src/settings.rs`:

- **Removing or renaming a key:** add the old key's path to `REMOVED_PLAYER_KEYS`. A saved file that still has it
  loads, and the key is dropped; any other unknown key still fails.
- **Adding a key:** add its path to `ADDED_PLAYER_KEYS`. A saved file without it loads with the key's built-in value;
  any other missing key still fails.
- **Renaming** is a removal plus an addition. Say in the PR that the old value is not carried over, or carry it over
  explicitly and test that.
- **Test both:** an old save loads, and a similar key that is not on the list still fails.
- **Never** read or write the user's real saved file in a test or a screenshot run; use `--player-settings
  player_settings.yaml` or a file in your scratch folder.

## Server settings

The server sends its `server_settings.yaml` to every player when they join, and a window that can't read them says the
server runs another version of the game.

- **What breaks:** adding, removing or renaming a server setting makes windows built before the change unable to join.
  That is acceptable within one playtest's commit, but say so in the PR and in the user's test steps ("rebuild every
  window from this commit").
- **Validate on the server:** a setting's range is checked where the file is loaded, so a bad value fails at start
  with an error naming the key, not in a player's window.

## What the network sends

The server and windows exchange fixed byte layouts (`src/net.rs`): snapshots, each player's record, and its packed
bits. Windows check a snapshot's length exactly, and one of the wrong length means "another version" to them
(`Client::is_other_version`).

- **Spare bits first.** Where the data fits, put it in spare bits of an existing byte, which keeps every length the
  same. Older windows then read the old bits as before and ignore the new ones.
- **One owner per bit.** Before using a bit, find every use of that byte (e.g. the laser byte: `RIFLE_BIT`,
  `TEAM_BIT`, `SPECKLE_FIRST_BIT`), and name yours with a constant beside them and a line in the record's docstring
  that says which bits it takes. Two changes once
  put the shot-noise share and the thrown wormholes' tint in the same bits, and only a merge caught it.
- **A new length is a new version.** Adding bytes changes the snapshot's length, so windows from before the change
  stop joining and say why. Make that change knowingly: say so in the PR, and update the tests that assert the
  layout's sizes.
- **Test the round trip:** write a record and read it back with every new field at its ends (0, its maximum), and
  check that the old fields read the same.

## Map files

Maps follow the portable format in `common/maps/` (spec in its README, schema in `map.schema.json`, `version` field).

- **Optional fields keep the version.** A change that existing readers can ignore (a new optional field, a new style)
  keeps `version`; any other change raises it (the README's rule). Programs that read maps must ignore what they don't
  use, and fail clearly on a version they don't know.
- **Generated maps stay reproducible.** A map that a script exports (Lunar Surface's `export_map.mjs`) is regenerated
  by that script, and the exporter must still reproduce the committed file byte for byte.
- **Validate:** run the map tests and the validator (`make -C common/maps test`) on every map you change or whose
  format you change.

## In the PR

Under Decisions, say what each change does to what already exists:
- old saved settings load or not;
- windows from before the change can join or not;
- old maps load or not.

In the test steps for the user, say which of those need a rebuild.
