"""Scratch: list a hyperbolic level's objects by zone with Fermi positions and auto names."""
import collections, sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from annotate import load, fermi_pos

def collect(level, radius_m, zone_edge_m, cluster_names=("hesco block",), cluster_gap_m=5.0):
    d = load(level)
    singles = []
    clusters = collections.defaultdict(list)
    for s in d["structures"]:
        u, v = fermi_pos(s["frame"], radius_m)
        if s["name"] in cluster_names:
            clusters[s["name"]].append((u, v))
        else:
            singles.append((s["name"], u, v))
    walls = []
    for name, pts in clusters.items():
        groups = []
        for p in sorted(pts):
            for g in groups:
                if any(abs(p[0] - q[0]) <= cluster_gap_m and abs(p[1] - q[1]) <= cluster_gap_m for q in g):
                    g.append(p); break
            else:
                groups.append([p])
        merged = True
        while merged:
            merged = False
            for i in range(len(groups)):
                for j in range(i + 1, len(groups)):
                    if any(abs(p[0] - q[0]) <= cluster_gap_m and abs(p[1] - q[1]) <= cluster_gap_m for p in groups[i] for q in groups[j]):
                        groups[i] += groups.pop(j); merged = True; break
                if merged: break
        for g in groups:
            us = [p[0] for p in g]; vs = [p[1] for p in g]
            walls.append((name, (min(us) + max(us)) / 2, (min(vs) + max(vs)) / 2, (max(us) - min(us)), (max(vs) - min(vs)), len(g)))
    def zone(u):
        return "blue" if u >= zone_edge_m else "red" if u <= -zone_edge_m else "middle"
    warps = [(w["name"], *fermi_pos(w["frame"], radius_m)) for w in d.get("warps", [])]
    spawns = [(s["team"] + " spawn", *fermi_pos(s["frame"], radius_m)) for s in d["spawns"]]
    return singles, walls, warps, spawns, zone

if __name__ == "__main__":
    level, radius, edge = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
    singles, walls, warps, spawns, zone = collect(level, radius, edge)
    for z in ("blue", "middle", "red"):
        print("==", z)
        for name, u, v in sorted(singles + warps + spawns, key=lambda t: (-t[1], t[2])):
            if zone(u) == z:
                print(f"   {name:24s} u={u:7.1f} v={v:6.1f}")
        for name, u, v, du, dv, n in sorted(walls, key=lambda t: (-t[1], t[2])):
            if zone(u) == z:
                print(f"   {name} wall ×{n:2d}       u={u:7.1f} v={v:6.1f} size {du:.0f}×{dv:.0f}")
