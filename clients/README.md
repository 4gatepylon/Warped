# Clients

One folder per platform, each in its own languages; macOS (`mac/`) first. A client renders, reads
input, joins rooms through the management server and plays through a game server. It may implement
parts of `common/physics/` (e.g. to predict the player's own movement), but knows only what the
game server sends it.
