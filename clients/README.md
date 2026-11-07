# Clients

One folder per platform, each in its own languages; macOS (`mac/`) first. `starter/` is a bare-bones
Rust + SDL3 client for the starter square game, until the real clients exist. A client renders, reads
input, joins a game server through the management server and plays on it. It may implement
parts of `common/physics/` (e.g. to predict the player's own movement), but knows only what the
game server sends it.
