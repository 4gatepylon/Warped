# Authentication and encryption

How a game server lets in only the players who know its password, and how every datagram between a window and the
server is encrypted. Every implementation follows this page (the demo, the game in `game/`, the Mac app, the cloud
deployment); change it only together with the code, and raise the protocol version when the bytes change. Why this
design is secure enough, and what it doesn't protect, is argued in [`security_case.md`](security_case.md), which
changes with this page.

There are two versions:

| Version | Where | What |
| --- | --- | --- |
| **v2** (this page's design) | `game/`, from step C1 of [`game/REFACTOR_PLAN.md`](../../game/REFACTOR_PLAN.md) | Every server must have a password, generated (never chosen), and every datagram is encrypted and authenticated with a standard Noise handshake and ChaCha20-Poly1305. |
| **v1** | the demo, from the infra stack's [#181](https://github.com/4gatepylon/Warped/pull/181), and `game/` until C1 | An HMAC challenge proves the password; nothing is encrypted; the password may be empty. Described at the end. |

## v2

### Building blocks

Only standard, widely deployed cryptography, through one maintained library; nothing home-made:

| Block | What it does | Where else it protects real traffic |
| --- | --- | --- |
| [Noise Protocol Framework](https://noiseprotocol.org/noise.html), pattern `Noise_NNpsk0_25519_ChaChaPoly_SHA256` | the handshake: both sides prove they know the password and agree on fresh keys | WireGuard (VPNs), WhatsApp's client-server traffic, the Lightning Network, libp2p |
| X25519 (Curve25519 Diffie-Hellman) | fresh keys for each session, so a password leaked later can't decrypt recorded games (forward secrecy) | TLS 1.3 (most HTTPS connections), SSH, Signal, WireGuard |
| ChaCha20-Poly1305 (RFC 8439) | encrypts each datagram and detects any change to it (an AEAD cipher) | TLS 1.3, QUIC (HTTP/3), WireGuard, SSH |
| SHA-256, HKDF (RFC 5869) | hashing inside Noise; turning the password into a 32-byte key | everywhere |
| [`snow`](https://docs.rs/snow) 0.10 (Rust) | implements Noise, including a "stateless" transport mode made for UDP, where the sender numbers each datagram | the most used Rust implementation of Noise (about 1,100 GitHub stars, 600 commits since 2017, tested against the Noise test vectors and fuzzed). It has had **no formal audit** of its own. The ciphers it runs by default are the RustCrypto `chacha20poly1305` and `sha2` crates and `curve25519-dalek`, pure Rust, so the server still builds for Linux with the infra stack's musl target. |

### The game password

- **Generated, never chosen.** `warped-server create-game-password --output-file <path>` draws 20 bytes (160 bits)
  from the operating system's random generator and writes them as text: `warped2-` followed by the base32 (RFC
  4648, lowercase) of the 20 bytes and a 2-byte checksum (the first 2 bytes of SHA-256 of `warped password` and the
  20 bytes), in groups of 4 letters joined by `-`. That's 36 letters, so the password is
  `warped2-xxxx-xxxx-xxxx-xxxx-xxxx-xxxx-xxxx-xxxx-xxxx`. The command prints the file's path, not the password.
  The owner copies it (`pbcopy < <path>`) into a message to their friends.
- **Why generated:** someone who records a handshake can test password guesses offline as fast as their computer
  allows. A chosen password can be guessed; 160 random bits can't be, ever. That makes a pre-shared key safe without
  slow password hashing.
- **Always required.** `warped-server` doesn't start without `--game-password-file`, or with a file that isn't a
  valid `warped2-` password (exit code 2, with the command that creates one). This holds even on a local network.
  The window checks the same way before sending anything, so a mistyped password is caught by its checksum on the
  player's machine ("the password was mistyped"), not by the server.
- **Reading it:** letters in either case; dashes, spaces and line breaks are ignored.
- **Keeping it:** the file is mode 0600. The Mac app keeps one password per server address in
  `~/Library/Application Support/Warped/game_passwords/` (a file per `<address>_<port>`, mode 0600), so players
  can switch between friends' servers. A password is never logged, put on a command line or shown on screen.
- **The key:** psk = HKDF-SHA256(input = the 20 bytes, salt = `warped psk`, info = `v2`), 32 bytes, the Noise
  pre-shared key.
- **Changing it:** create a new one and restart the server. Everyone needs the new one.

### Datagrams

Every datagram starts with a 3-byte header in the clear: magic `0xF7`, `PROTOCOL_VERSION` (u8), type (u8).
Multi-byte numbers are little-endian. Indexes are random u32 numbers each side picks for a session, as in
WireGuard, so a datagram names its session without naming the player.

| Type | Direction | After the header | Size |
| --- | --- | --- | --- |
| 1 `HandshakeInit` | window → server | sender index (u32), Noise message 1 (`psk, e`, with a 16-byte zero payload) | 71 bytes |
| 2 `HandshakeResponse` | server → window | sender index (u32), receiver index (u32), Noise message 2 (`e, ee`, empty payload) | 59 bytes |
| 3 `Transport` | both | receiver index (u32), counter (u64), the game message encrypted with ChaCha20-Poly1305 and its 16-byte tag | message + 31 bytes |
| 4 `OtherVersion` | server → window | the server's `PROTOCOL_VERSION` (u8), in the clear | 4 bytes |

- **Game messages** are the existing ones (`Join`, `State`, `Snapshot`, ...), each starting with its `Kind` byte,
  now always inside a `Transport` datagram. v1's `Challenge`, `Refused` and token go away.
- **Counter:** each direction of a session numbers its datagrams 0, 1, 2, ...; the counter is the Noise nonce
  (`snow`'s `StatelessTransportState`), so it is never reused under one key. The receiver drops a datagram whose
  counter it has already seen or that is more than 64 behind the highest it has seen (a sliding window, as in
  WireGuard and IPsec). Reordered datagrams within the window are accepted. Changing the header in transit changes
  the nonce or the session, so decryption fails.

### The exchange

1. The window, holding the password, sends `HandshakeInit`.
2. The server reads Noise message 1. With the wrong password, its tag doesn't verify (cheap: no key exchange yet),
   and the server drops it without a reply. With the right one, it answers `HandshakeResponse` and keeps the
   half-open session for 5 s. `HandshakeInit` is padded to be longer than `HandshakeResponse`, so a forged sender
   address never makes the server send more than it got. The server accepts at most 20 handshakes a second from
   one IP address and holds at most 64 half-open ones.
3. Both sides now have two keys, one per direction. The window sends its first `Transport` datagram, the `Join`
   (the team it asks for). That datagram proves the window received the response at its address, so only then does
   the server make the player and answer with the `Welcome`, which may be large. The session is now open.
4. Every later datagram is a `Transport`. The server moves a session to the address of its newest authenticated
   datagram, so a player whose home router changes their port keeps playing.
5. A session ends on `Leave` or after 3 s without a valid datagram. A window that comes back does a new handshake,
   with new keys.
6. A datagram with the magic but another `PROTOCOL_VERSION` gets `OtherVersion`, which the window reports as "the
   server runs another version of the game". Anything else invalid gets no reply.

### What it protects against, and what not

- **Protects against:** strangers joining or interfering. Anyone on the network path (the same café Wi-Fi, an
  ISP) reading, changing, replaying or injecting game traffic. Guessing the password, offline or online. Using the
  server to flood a third party. Recorded games being decrypted later if the password leaks (forward secrecy).
- **Doesn't protect against:**
  - anyone who has the password: friends can join, and could impersonate the server to each other;
  - an observer seeing that a game is played, how big and how frequent its datagrams are;
  - a forged `OtherVersion` showing a window a wrong message (it carries no secret and changes nothing);
  - friends cheating: the server still takes each window's word for its hits.

### Cost

- **Handshake:** two X25519 operations per side, well under a millisecond, once per join.
- **Each datagram:** ChaCha20-Poly1305 over at most 1,200 bytes, a few microseconds, plus 31 bytes. A 10-player
  snapshot at 60 Hz costs the server well under 1% of a core. The snapshot budget of 1,200 bytes per datagram
  (plan, step C3) includes those 31 bytes.
- **Dependency:** `snow` with its default pure-Rust primitives, version pinned and at least 7 days old when added.

### Tests every implementation keeps

Each partition, and the tests that cover its parts, goes in the test file's docstring (`unit-tests` skill):

- **Password text:**
  - a generated password parses back to its bytes;
  - wrong prefix, wrong length, wrong checksum, letters outside base32 → refused with a reason;
  - case, dashes and spaces don't matter.
- **Server start:** no password file, empty file, malformed password → exit code 2; valid → runs.
- **Handshake:** right password → session open; wrong password → no reply, no state kept; an `HandshakeInit`
  replayed from a recording → a response the replayer can't use, and no player made; another version →
  `OtherVersion`.
- **Transport:**
  - one changed byte anywhere → dropped;
  - a replayed counter → dropped;
  - reordered within 64 → accepted once each;
  - more than 64 behind → dropped;
  - a valid datagram from a new address → the session moves there.
- **Amplification:** no reply to an unproven address is longer than the datagram that caused it.
- **Strangers during a game** (plan, 7.5 scenario 7): no slot, no change to the game, nothing noticed by players.

## v1 (the demo, until replaced)

The infra stack's #181, kept here so the demo and `game/` before step C1 stay documented:

- `--game-password-file` holds any password; without it the password is empty.
- `Join` carries `PROTOCOL_VERSION` (u16), a 16-byte cookie and a 32-byte proof. The cookie is the first 16 bytes of
  HMAC-SHA256(server secret, the sender's `ip:port`); the proof is HMAC-SHA256(password, cookie).
- A wrong or missing cookie gets `Challenge` (kind 14, the right cookie). A wrong version or proof gets `Refused`
  (kind 15, reason 1 or 2, the server's version). A right proof gets `Welcome` with a 64-bit token.
- Every later message from the window carries the token, and the server checks token, player id and address.
- Nothing is encrypted, a weak password can be guessed offline from a recorded join, and an empty password lets
  anyone in. That's why v2 replaces it.
