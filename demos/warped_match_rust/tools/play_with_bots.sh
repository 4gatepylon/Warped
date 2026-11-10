#!/usr/bin/env bash
# `play_with_bots.sh` plays a match without other people, in one command: it starts a match server,
# bots on this player's team (allies) and on the other team (enemies), each a headless match client
# that joins every round's level as a bot, and this player's match window; it stops the server and
# the bots when the window closes. Without enemies, one bot on the other team sits out every round,
# since a round starts only when each team has a player. Click a level on the progression page to
# play it now.
#
# Interface:
#     --team <blue|red>: this player's team (default blue).
#     --allies <count>: bots on this player's team, 0 to 4 (default 0).
#     --enemies <count>: bots on the other team, 0 to 5 (default 0: play alone); allies and
#         enemies together at most 7, since a level takes 8 players.
#     --bot-objective <consoles|fight>: what the bots do (default consoles).
#     --match-port <port>, --port <port>: the match server's and each round's level server's UDP
#         ports (default 47850, 47852).
#     --match-levels <path>: the levels file (default match_levels.yaml).
#     --dry-run: print the commands and start nothing.
#     Output: the server's and bots' logs in .artifacts/play_with_bots/<run_id>/, with command.txt.
#     Network: everything it starts runs with WARPED_LOOPBACK_ONLY=1, on 127.0.0.1 only, so macOS never
#         asks for network permission and nobody on the network sees the match.
#
# Example:
#     tools/play_with_bots.sh --allies 2 --enemies 3 --bot-objective fight
set -euo pipefail

readonly MAX_ALLY_COUNT=4
readonly MAX_ENEMY_COUNT=5
readonly MAX_BOT_COUNT=7
readonly SIT_OUT_ROUND_COUNT=255
readonly CRATE_FOLDER="$(cd "$(dirname "$0")/.." && pwd)"
readonly PROGRAM="$CRATE_FOLDER/.build/release/hyperbolic-wormholes"
readonly USAGE="Example: tools/play_with_bots.sh --team blue --allies 2 --enemies 3 --bot-objective fight"
readonly COMMAND_LINE="$0 $*"
export WARPED_LOOPBACK_ONLY=1

team=blue
ally_count=0
enemy_count=0
bot_objective=consoles
match_port=47850
level_port=47852
match_levels="$CRATE_FOLDER/match_levels.yaml"
is_dry_run=0
while [ $# -gt 0 ]; do
    case "$1" in
        --team) team="$2"; shift 2 ;;
        --allies) ally_count="$2"; shift 2 ;;
        --enemies) enemy_count="$2"; shift 2 ;;
        --bot-objective) bot_objective="$2"; shift 2 ;;
        --match-port) match_port="$2"; shift 2 ;;
        --port) level_port="$2"; shift 2 ;;
        --match-levels) match_levels="$2"; shift 2 ;;
        --dry-run) is_dry_run=1; shift ;;
        -h|--help) sed -n '2,23p' "$0"; exit 0 ;;
        *) echo "play_with_bots.sh: unknown argument $1. $USAGE" >&2; exit 2 ;;
    esac
done
case "$team" in
    blue) other_team=red ;;
    red) other_team=blue ;;
    *) echo "play_with_bots.sh: --team must be blue or red, got $team. $USAGE" >&2; exit 2 ;;
esac
if ! [[ "$ally_count" =~ ^[0-9]+$ ]] || [ "$ally_count" -gt "$MAX_ALLY_COUNT" ]; then
    echo "play_with_bots.sh: --allies must be 0 to $MAX_ALLY_COUNT, got $ally_count. $USAGE" >&2; exit 2
fi
if ! [[ "$enemy_count" =~ ^[0-9]+$ ]] || [ "$enemy_count" -gt "$MAX_ENEMY_COUNT" ]; then
    echo "play_with_bots.sh: --enemies must be 0 to $MAX_ENEMY_COUNT, got $enemy_count. $USAGE" >&2; exit 2
fi
if [ $((ally_count + enemy_count)) -gt "$MAX_BOT_COUNT" ]; then
    echo "play_with_bots.sh: --allies and --enemies add up to at most $MAX_BOT_COUNT (a level takes 8 players), got $((ally_count + enemy_count)). $USAGE" >&2; exit 2
fi
case "$bot_objective" in
    consoles|fight) ;;
    *) echo "play_with_bots.sh: --bot-objective must be consoles or fight, got $bot_objective. $USAGE" >&2; exit 2 ;;
esac
if [ ! -x "$PROGRAM" ]; then
    echo "play_with_bots.sh: $PROGRAM isn't built; run make play-alone or cargo build --release first" >&2; exit 1
fi

server_command=("$PROGRAM" --serve-match --match-levels "$match_levels" --match-port "$match_port" --port "$level_port")
bot_prefix="$PROGRAM --match 127.0.0.1 --match-port $match_port --bot"
bot_commands=()
bot_teams=()
for ((i = 0; i < ally_count; i++)); do
    bot_commands+=("$bot_prefix --team $team --bot-objective $bot_objective")
    bot_teams+=("$team")
done
for ((i = 0; i < enemy_count; i++)); do
    bot_commands+=("$bot_prefix --team $other_team --bot-objective $bot_objective")
    bot_teams+=("$other_team")
done
if [ "$enemy_count" -eq 0 ]; then
    winners="$team"
    for ((i = 1; i < SIT_OUT_ROUND_COUNT; i++)); do winners="$winners,$team"; done
    bot_commands+=("$bot_prefix --team $other_team --bot-round-winners $winners")
    bot_teams+=("$other_team")
fi
window_command=("$PROGRAM" --match 127.0.0.1 --match-port "$match_port" --team "$team")
if [ "$is_dry_run" -eq 1 ]; then
    echo "${server_command[*]}"
    printf '%s\n' "${bot_commands[@]}"
    echo "${window_command[*]}"
    exit 0
fi

run_folder="$CRATE_FOLDER/.artifacts/play_with_bots/$(date -u +%Y%m%d-%H%M%S)"
mkdir -p "$run_folder"
printf '%s\ncommit: %s\n' "$COMMAND_LINE" "$(git -C "$CRATE_FOLDER" describe --always --dirty)" > "$run_folder/command.txt"
echo "Run folder: $run_folder"
trap 'kill $(jobs -p) 2>/dev/null || true' EXIT
"${server_command[@]}" > "$run_folder/server.log" 2>&1 &
for i in "${!bot_commands[@]}"; do
    ${bot_commands[$i]} > "$run_folder/bot_$((i + 1))_${bot_teams[$i]}.log" 2>&1 &
done
"${window_command[@]}"
