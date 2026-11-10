#!/usr/bin/env bash
# `package_mac_app.sh` builds Warped.app, the one-window game a player double-clicks: the app
# (`warped`, as `Contents/MacOS/Warped`), the match server and Lunar Surface (`warped-match`), each
# other level of the levels manifest (`Contents/MacOS/levels/<level>`), the manifest with its map
# pictures and each level's server settings file (at its path from the manifest, for the game
# master's settings), the shared assets and models, an icon and Info.plist; signs it ad hoc and zips it with
# its note on opening it. The levels are built one after another into one shared target folder, so
# their common crates are built once.
#
# Interface:
#     --dry-run: print what it would build and copy, and change nothing.
#     Output: .build/Warped/Warped.app, .build/Warped/How to open Warped.txt and .build/Warped.zip
#         (all in .build/, removed by make clean).
#
# Example:
#     tools/package_mac_app.sh
set -euo pipefail

readonly CRATE_FOLDER="$(cd "$(dirname "$0")/.." && pwd)"
readonly REPO_FOLDER="$(cd "$CRATE_FOLDER/../.." && pwd)"
readonly MATCH_FOLDER="$REPO_FOLDER/demos/warped_match_rust"
readonly APP_FOLDER="$CRATE_FOLDER/.build/Warped"
readonly APP="$APP_FOLDER/Warped.app"
readonly APP_ZIP="$CRATE_FOLDER/.build/Warped.zip"
readonly ICONSET="$CRATE_FOLDER/.build/AppIcon.iconset"
readonly LEVELS_TARGET="$CRATE_FOLDER/.build/levels_target"
readonly ICON_SIZES="16 32 128 256 512"

is_dry_run=0
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) is_dry_run=1; shift ;;
        -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
        *) echo "package_mac_app.sh: unknown argument $1. Example: tools/package_mac_app.sh --dry-run" >&2; exit 2 ;;
    esac
done

cd "$CRATE_FOLDER"
cargo build --release --locked
level_lines="$(./.build/release/warped --print-level-programs)"
if [ "$is_dry_run" -eq 1 ]; then
    echo "app: $APP"
    echo "match server: $MATCH_FOLDER (hyperbolic-wormholes) -> Contents/MacOS/warped-match"
    while IFS=$'\t' read -r slug folder program picture settings; do
        echo "level: $REPO_FOLDER/$folder ($program) -> Contents/MacOS/levels/$slug; picture $picture; settings $settings"
    done <<< "$level_lines"
    exit 0
fi

rm -rf "$APP_FOLDER" "$ICONSET"
mkdir -p "$APP/Contents/MacOS/levels" "$APP/Contents/Resources" "$ICONSET"
cp .build/release/warped "$APP/Contents/MacOS/Warped"
(cd "$MATCH_FOLDER" && CARGO_TARGET_DIR="$LEVELS_TARGET" cargo build --release --locked --bin hyperbolic-wormholes)
cp "$LEVELS_TARGET/release/hyperbolic-wormholes" "$APP/Contents/MacOS/warped-match"
cp "$MATCH_FOLDER/match_levels.yaml" "$APP/Contents/Resources/match_levels.yaml"
cp "$MATCH_FOLDER/server_settings.yaml" "$APP/Contents/Resources/server_settings.yaml"
while IFS=$'\t' read -r slug folder program picture settings; do
    (cd "$REPO_FOLDER/$folder" && CARGO_TARGET_DIR="$LEVELS_TARGET" cargo build --release --locked --bin "$program")
    cp "$LEVELS_TARGET/release/$program" "$APP/Contents/MacOS/levels/$slug"
    if [ "$picture" != "-" ]; then
        mkdir -p "$(dirname "$APP/Contents/Resources/$picture")"
        cp "$REPO_FOLDER/$folder/${picture#*"$folder"/}" "$APP/Contents/Resources/$picture"
    fi
    if [ "$settings" != "-" ]; then
        mkdir -p "$(dirname "$APP/Contents/Resources/$settings")"
        cp "$REPO_FOLDER/$folder/${settings#*"$folder"/}" "$APP/Contents/Resources/$settings"
    fi
done <<< "$level_lines"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)"
sed "s/@VERSION@/$version/" "$MATCH_FOLDER/app/Info.plist" > "$APP/Contents/Info.plist"
for size in $ICON_SIZES; do
    sips -z "$size" "$size" "$MATCH_FOLDER/app/app_icon.png" --out "$ICONSET/icon_${size}x${size}.png" > /dev/null
    sips -z $((size * 2)) $((size * 2)) "$MATCH_FOLDER/app/app_icon.png" --out "$ICONSET/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
mkdir -p "$APP/Contents/Resources/assets/models" "$APP/Contents/Resources/models"
cp -R "$REPO_FOLDER/demos/fps_demo/assets/textures" "$APP/Contents/Resources/assets/textures"
cp -R "$REPO_FOLDER/demos/fps_demo/assets/models/bolt_action_rifle_7_62" "$APP/Contents/Resources/assets/models/"
rsync -a "$REPO_FOLDER/common/maps/models/" "$APP/Contents/Resources/models/"
cp app/how_to_open.txt "$APP_FOLDER/How to open Warped.txt"
codesign --force --deep --sign - "$APP"
ditto -c -k --keepParent "$APP_FOLDER" "$APP_ZIP.partial"
mv "$APP_ZIP.partial" "$APP_ZIP"
echo "App: $APP"
echo "Zip: $APP_ZIP"
