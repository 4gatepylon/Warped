#!/bin/sh
# `download_textures.sh` downloads Dust's Poly Haven texture sets (CC0, 2K JPG: albedo, OpenGL
# normal, AO-roughness-metalness) into `assets/textures/<set>/`, skipping files already there.
#
# Usage: tools/download_textures.sh   (run from demos/dust_rust/; `make download-assets` does)
set -eu
SETS="aerial_sand gravelly_sand cliff_side large_sandstone_blocks"
for set in $SETS; do
    mkdir -p "assets/textures/$set"
    for map in diff nor_gl arm; do
        file="assets/textures/$set/${set}_${map}_2k.jpg"
        if [ ! -s "$file" ]; then
            curl -fsSL -o "$file.partial" "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/$set/${set}_${map}_2k.jpg"
            mv "$file.partial" "$file"
            echo "Downloaded $file"
        fi
    done
done
