#!/bin/bash
# `install.sh` installs Warped on this Mac from the download page: it downloads `Warped.zip` with
# curl, which doesn't mark it as downloaded from the internet, so macOS opens the app without the
# Open Anyway step; then it puts `Warped.app` in Applications and opens it.
#
# Interface:
#     WARPED_INSTALL_FOLDER: where to put Warped.app (default /Applications).
#     The deploy replaces @SITE_URL@ with the download page's address.
#
# Example:
#     curl -fsSL @SITE_URL@/install.sh | bash
#
# Must know:
#     - An older Warped.app is moved into the download's temporary folder, not deleted outright.
set -euo pipefail
site_url="@SITE_URL@"
install_folder="${WARPED_INSTALL_FOLDER:-/Applications}"
if [ "$(uname -s)" != Darwin ] || [ "$(uname -m)" != arm64 ]; then
  echo "Warped needs a Mac with Apple silicon (M1 or later)." >&2
  exit 1
fi
download_folder="$(mktemp -d)"
echo "Downloading Warped from ${site_url}..."
curl -fL --progress-bar "${site_url}/Warped.zip" -o "${download_folder}/Warped.zip"
ditto -x -k "${download_folder}/Warped.zip" "${download_folder}"
if [ -e "${install_folder}/Warped.app" ]; then
  mv "${install_folder}/Warped.app" "${download_folder}/previous_Warped.app"
fi
ditto "${download_folder}/Warped/Warped.app" "${install_folder}/Warped.app"
echo "Installed ${install_folder}/Warped.app; opening it."
open "${install_folder}/Warped.app"
