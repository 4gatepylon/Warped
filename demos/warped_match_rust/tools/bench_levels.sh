#!/bin/bash
# `bench_levels.sh` times every bench view of the match's five levels (Dust, Snow Planet, City Sphere, Dark
# Forest, Lunar Surface) at one image size, and writes each view's median and p99 GPU time, with the system
# it ran on, to `.artifacts/bench_levels/<run_id>/` of `demos/warped_match_rust/` (`summary.md`).
#
# Interface:
#     --width-px <n>, --height-px <n>: the image size in pixels, both even (default 2560 x 1440).
#     --frame-count <n>: timed frames per view of the four ray-marched levels (default 100).
#     --view-s <n>: seconds Dark Forest holds each view in its window; it times all but the first (default 10).
#     --label <label>: a suffix for the run folder's name (letters, digits, - and _).
#     --dry-run: check the arguments and print the commands, without building or running anything.
#
# Example:
#     tools/bench_levels.sh --width-px 2560 --height-px 1440 --label quiet
#
# Must know:
#     Limitations:
#         - Dark Forest draws in a window of half the image size in points, at render scale 2 when the main display
#           has 1 pixel per point, so its last pass (FXAA and the HUD) runs at half the image size there.
#     Gotchas:
#         - A level that fails or times no views gets a row saying so, and the run carries on to the next level.
#     Side effects:
#         - Each ray-marched level also keeps its own run folder in `demos/<level>_rust/.artifacts/bench_views/`.
#     Performance:
#         - A run takes about 5 minutes; other programs on the GPU make it rough, so `summary.md` gives the load.
set -euo pipefail

COMMAND_LINE="$0 $*"
MATCH_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DEMOS_DIR="$(dirname "$MATCH_DIR")"
IMAGE_PX_PER_WINDOW_PT=2
WINDOWED_MAP=-
LEVELS=(
  "Dust dust_rust hyperbolic-wormholes dust"
  "Snow-Planet snow_planet_rust snow-planet snow-planet"
  "City-Sphere city_sphere_rust city-sphere city-sphere"
  "Dark-Forest dark_forest_rust dark-forest $WINDOWED_MAP"
  "Lunar-Surface warped_match_rust hyperbolic-wormholes lunar-surface"
)
JQ_MEDIAN='(.median_gpu_ms // .gpu_ms_median) * 10 | round / 10'
JQ_P99='(.p99_gpu_ms // .gpu_ms_p99) * 10 | round / 10'
JQ_ROWS=".views[] | \"| \\(\$level) | \\(.view) | \\($JQ_MEDIAN) | \\($JQ_P99) |\""
EXAMPLE="e.g. $0 --width-px 2560 --height-px 1440 --label quiet"

width_px=2560
height_px=1440
frame_count=100
view_s=10
label=""
is_dry_run=0
while [ $# -gt 0 ]; do
  case "$1" in
    --width-px) width_px="$2"; shift 2 ;;
    --height-px) height_px="$2"; shift 2 ;;
    --frame-count) frame_count="$2"; shift 2 ;;
    --view-s) view_s="$2"; shift 2 ;;
    --label) label="$2"; shift 2 ;;
    --dry-run) is_dry_run=1; shift ;;
    *) echo "unknown argument $1; $EXAMPLE" >&2; exit 2 ;;
  esac
done
for pair in "--width-px $width_px" "--height-px $height_px" "--frame-count $frame_count" "--view-s $view_s"; do
  [[ "${pair#* }" =~ ^[1-9][0-9]*$ ]] || { echo "$pair must be a positive whole number; $EXAMPLE" >&2; exit 2; }
done
(( width_px % IMAGE_PX_PER_WINDOW_PT == 0 && height_px % IMAGE_PX_PER_WINDOW_PT == 0 )) ||
  { echo "--width-px and --height-px must be even, got $width_px x $height_px; $EXAMPLE" >&2; exit 2; }
[[ "$label" =~ ^[A-Za-z0-9_-]*$ ]] || { echo "--label must be letters, digits, - and _, got $label; $EXAMPLE" >&2; exit 2; }

run_dir="$MATCH_DIR/.artifacts/bench_levels/$(date -u +%Y%m%d-%H%M%S)${label:+-$label}"
voxel_flags=(--bench-views --json --width-px "$width_px" --height-px "$height_px" --frame-count "$frame_count"
  --label bench-levels)
display_px_per_pt="$(system_profiler SPDisplaysDataType | awk -F': ' '
  /Resolution:/ {split($2, size, " "); pixels = size[1]}
  /UI Looks like:/ {split($2, size, " "); points = size[1]}
  /Main Display: Yes/ {print pixels / points; exit}')"
[[ "$display_px_per_pt" =~ ^[12]$ ]] ||
  { echo "the main display has $display_px_per_pt pixels per point; Dark Forest's timing needs 1 or 2" >&2; exit 1; }
window_flags=(--bench-views "$view_s" --width-pt $((width_px / IMAGE_PX_PER_WINDOW_PT))
  --height-pt $((height_px / IMAGE_PX_PER_WINDOW_PT)) --render-scale $((IMAGE_PX_PER_WINDOW_PT / display_px_per_pt)))
window_px="$((width_px / IMAGE_PX_PER_WINDOW_PT * display_px_per_pt))x$((height_px / IMAGE_PX_PER_WINDOW_PT * display_px_per_pt))"
if (( is_dry_run )); then
  echo "would build and run these, each in its demos/<folder>/, and write $run_dir:"
  for level in "${LEVELS[@]}"; do
    read -r name folder binary map <<< "$level"
    if [ "$map" = "$WINDOWED_MAP" ]; then
      echo "  $folder: ./.build/release/$binary ${window_flags[*]}"
    else
      echo "  $folder: ./.build/release/$binary --map $map ${voxel_flags[*]}"
    fi
  done
  exit 0
fi
[ ! -e "$run_dir" ] || { echo "$run_dir already exists; wait a second or pass another --label" >&2; exit 1; }
mkdir -p "$run_dir"
printf '%s\ncommit: %s\n' "$COMMAND_LINE" "$(git -C "$MATCH_DIR" describe --always --dirty)" > "$run_dir/command.txt"
{
  echo "machine: $(sysctl -n hw.model), $(sysctl -n machdep.cpu.brand_string), $(sysctl -n hw.ncpu) CPU cores," \
    "$(system_profiler SPDisplaysDataType | awk -F': ' '/Total Number of Cores/ {print $2; exit}') GPU cores," \
    "$(( $(sysctl -n hw.memsize) / 1073741824 )) GiB"
  echo "display: $(system_profiler SPDisplaysDataType | awk -F': ' '/Resolution/ {print $2; exit}')"
  echo "os: $(sw_vers -productName) $(sw_vers -productVersion) ($(sw_vers -buildVersion))"
  echo "rust: $(rustc --version)"
} > "$run_dir/system.txt"

echo "Building the five levels (log: $run_dir/build.log)..." >&2
for level in "${LEVELS[@]}"; do
  read -r name folder binary map <<< "$level"
  (cd "$DEMOS_DIR/$folder" && cargo build --release --bin "$binary") >> "$run_dir/build.log" 2>&1
done

load_at_start="$(sysctl -n vm.loadavg)"
rows=()
for level in "${LEVELS[@]}"; do
  read -r name folder binary map <<< "$level"
  echo "Timing $name..." >&2
  level_rows=""
  if [ "$map" = "$WINDOWED_MAP" ]; then
    log="$run_dir/$name.txt"
    if (cd "$DEMOS_DIR/$folder" && "./.build/release/$binary" "${window_flags[@]}") > "$log" 2>&1; then
      level_rows="$(sed -nE "s/^view ([A-Za-z]+) at ${window_px} px: .*GPU median ([0-9.]+) ms.*GPU p99 ([0-9.]+) ms.*/| ${name/-/ } | \1 | \2 | \3 |/p" "$log")"
    fi
  else
    log="$run_dir/$name.log"
    if (cd "$DEMOS_DIR/$folder" && "./.build/release/$binary" --map "$map" "${voxel_flags[@]}") \
      > "$run_dir/$name.json" 2> "$log"; then
      level_rows="$(jq -r --arg level "${name/-/ }" "$JQ_ROWS" "$run_dir/$name.json" || true)"
    fi
  fi
  if [ -z "$level_rows" ]; then
    echo "warning: $name timed no views; see $log" >&2
    level_rows="| ${name/-/ } | no views timed (see \`$(basename "$log")\`) | - | - |"
  fi
  rows+=("$level_rows")
done
{
  echo "# Bench views of the five levels at ${width_px}x${height_px} px"
  echo
  echo "GPU milliseconds per frame. Load averages (1, 5, 15 min): $load_at_start at the start, $(sysctl -n vm.loadavg) at the end."
  echo
  echo "| Level | View | Median GPU ms | p99 GPU ms |"
  echo "| --- | --- | --- | --- |"
  printf '%s\n' "${rows[@]}"
} > "$run_dir/summary.md"
cat "$run_dir/summary.md"
echo "Run folder: $run_dir"
