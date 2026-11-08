# Warped

The game: a game window (`warped`) and a game server (`warped-server`), with the code they share written once. It
is being moved here from [`demos/hyperbolic_wormholes_rust/`](../demos/hyperbolic_wormholes_rust/README.md) by the
steps of [`REFACTOR_PLAN.md`](REFACTOR_PLAN.md). Step A1 copied the whole demo into `client/`, so the game plays
exactly as the demo does; the later steps split it into the crates below.

## Layout

| Folder | Crate | What goes there |
| --- | --- | --- |
| `core/` | `warped-core` (library) | Maps, the world and walking in it, lasers, thrown wormholes, settings, the network's messages and the window's side of a connection, bots for tests. No window, GPU or Apple dependency, so it builds on Linux. |
| `client/` | `warped-client`, binary `warped` | The window: title screen, rendering (Metal), HUD, menus, input, sound. macOS only. |
| `server/` | `warped-server`, binary `warped-server` | One game per process on a UDP port, behind the game's password. Builds on macOS and Linux. |
| `legacy_oracle/` | `legacy-oracle` (tests only) | The demo's own source files compiled in place, so tests can compare the game with the demo (plan, 7.3.1). |
| `tools/udp_impair/` (planned) | `udp-impair` | A UDP proxy that adds latency, jitter, loss and reordering, for network tests (plan, 7.6). |

`app/` holds the Mac app's `Info.plist`, icon and note. Which of the demo's files go where, and the workloads the
infra stack (Mac app, Linux server bundle, cloud) relies on, is in the plan's sections 3.1 and 4.

## Commands

`make help` lists them. The main ones:

```bash
make serve-game PORT=47800 MAP=lunar-surface GAME_PASSWORD_FILE=password.txt  # a game server, no window
make join-game SERVER=auto GAME_PASSWORD_FILE=password.txt                     # the window, joining it
cargo run --release -p warped-client                                           # the title screen
make package-mac-app ONLINE_GAMES=online_games.txt  # .build/Warped.zip
make build-server-bundle                            # .build/server_bundle.tar.gz, for x86-64 Linux
make test               # formatting, clippy, every crate's tests
make check-linux        # warped-core and warped-server build for x86-64 Linux
make check-environment  # macOS, chip, Metal device, toolchain: what this machine can test
```

The test targets of the plan's section 7.9 arrive with the steps that build them.

## Uncommitted files

Cargo writes to `.build/` (`make clean` removes it); test runs and benchmarks write to
`.artifacts/<script>/<run_id>/` (`make reset-artifacts`), the window's to `client/.artifacts/`.

## Protocol

The messages between the window and the server are specified in [`common/protocol/`](../common/protocol/README.md),
including how a server admits only the players who know its password
([`authentication.md`](../common/protocol/authentication.md)).

## Security

From step C1 of the plan, every game needs a generated password, and all traffic between the window and the server
is encrypted (a Noise handshake, then ChaCha20-Poly1305). The case for that design, its assumptions and the
questions for a security review are in [`common/protocol/security_case.md`](../common/protocol/security_case.md).

Known limitations (the security case's section 7 lists them all):

- **`snow` has had no formal audit.** The encryption uses the `snow` library, the most used Rust implementation of
  Noise, whose own README warns that it hasn't been audited. Its ciphers come from widely used RustCrypto and dalek
  crates. The user accepted this on 2026-11-08.
- **One password per game, shared by everyone in it**, so any player could impersonate the server to the others,
  and a removed player keeps access until the password is changed.
- **Before step C1** the game uses the demo's v1 scheme: a password check without encryption.
