# Authentication: a game's password

How a game server lets in only the players who know its password. A server's owner makes up a password, gives it
to their friends by any private channel (a WhatsApp message, say), and the server refuses everyone else. It is the
scheme of the infra stack's
[#181](https://github.com/4gatepylon/Warped/pull/181) ("protect the Rust demo's game server with a password
challenge"), written up here so the demo, the game in `game/`, the Mac app and the cloud deployment all build the
same thing. Change it only by changing this page in the same PR as the code, and raise `PROTOCOL_VERSION` when the
bytes change.

## What it protects against, and what not

- **Protects against:** strangers who find the server's address and port (scanners, a leaked address) joining,
  speaking for a player, or using the server to flood someone else with replies.
- **Doesn't protect against:** someone who can watch the traffic between a player and the server (on the same
  café Wi-Fi, say): messages are not encrypted, so they can read positions, and they could replay a captured join
  from the same address or try to guess a weak password offline. Friends cheating is out of scope too: the server
  still takes each window's word for its hits.
- **Upgrade path, if ever needed:** derive a per-session key from the password and both sides' random numbers, and
  seal every datagram with ChaCha20-Poly1305 (24 to 34 bytes per datagram). Not planned now.

## The password

- **Server:** `--game-password-file <path>` names a file holding the password (its final line break is dropped;
  an empty file is an error). Without the flag the password is empty, which suits a local network.
- **Player:** the same flag. In the Mac app, picking an online game on the title screen asks for the password in a
  macOS dialog (hidden input) the first time and keeps it in `~/Library/Application Support/Warped/game_password`
  (mode 0600), and asks again after a refusal. A password is never written to a log, a command line or a
  screenshot.
- **Choosing one:** long and random (e.g. `openssl rand -base64 18`), since a captured join lets someone test
  guesses offline.

## The bytes

Every message is one UDP datagram, little-endian, starting with its `Kind` byte (`net.rs`). Authentication adds two
kinds and three fields:

| Message | Direction | Layout |
| --- | --- | --- |
| `Join` (1) | window → server | kind, `PROTOCOL_VERSION` (u16), cookie (16 bytes, zeros until the server sends one), proof (32 bytes), team asked for (u8, optional) |
| `Challenge` (14) | server → window | kind, cookie (16 bytes) |
| `Refused` (15) | server → window | kind, reason (u8: 1 another version, 2 wrong password), the server's `PROTOCOL_VERSION` (u16) |
| `Welcome` (2) | server → window | kind, player id (u32), token (u64), then as before (team, map name, settings) |
| every later message | window → server | kind, token (u64), then as before |

- **Cookie** = the first 16 bytes of HMAC-SHA256(server secret, the sender's address as text, `ip:port`). The server
  secret is 32 random bytes made at start, so cookies change when the server restarts.
- **Proof** = HMAC-SHA256(password, cookie), 32 bytes. The password itself never crosses the network.
- **Token** = 64 random bits the server picks per player at their welcome.

## The exchange

1. The window sends `Join` with a zero cookie (and a proof of it).
2. The server checks the version first: another version gets `Refused` (reason 1), which the window reports as
   "the server runs another version of the game". Then the cookie: one that isn't this address's gets a
   `Challenge` carrying the right one. A `Challenge` (17 bytes) is smaller than a `Join` (52 bytes), so a forged
   source address can't turn the server into an amplifier.
3. The window resends `Join` with that cookie and its proof.
4. The server compares the proof with its own in constant time. A wrong one gets `Refused` (reason 2), which the
   window reports as a wrong password and stops retrying (the game exits with code 3, `WRONG_PASSWORD_EXIT_CODE`,
   which the title screen takes as "ask for the password again").
   A right one gets the `Welcome`, with the player's token.
5. From then on the server accepts a message only if its token, its player id and its sender's address match a
   joined player. Anything else is dropped without a reply.

## Cost

One HMAC per join attempt and 8 bytes on each message from a window. Nothing on the server's snapshots. That's
negligible next to a 60 Hz snapshot of up to 1,200 bytes.

## Tests every implementation keeps

Partitioned by the join's version × cookie × proof, plus the later messages' token:

- right version, no cookie → `Challenge`, no slot made;
- right version, another address's cookie → `Challenge`;
- right version, right cookie, wrong proof → `Refused` reason 2, no slot made;
- right version, right cookie, right proof → `Welcome` with a token;
- another version, anything else → `Refused` reason 1;
- a later message with a wrong token, a wrong id, or from another address → ignored, the game unchanged;
- no reply to an unproven address is ever longer than the datagram that caused it;
- an empty password on both sides works (local network).
