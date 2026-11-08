"""Frame-time statistics of a `viewer --frame-log` CSV: python scripts/frame_stats.py FILE [LIMIT_MS]
Prints frames, median, p99, spikes over the limit (default 20 ms), and the worst spikes with what each frame spent."""
import csv, sys
rows = list(csv.DictReader(open(sys.argv[1])))
limit = float(sys.argv[2]) if len(sys.argv) > 2 else 20.0
dt = sorted(float(r["dt"]) for r in rows)
n = len(dt)
spikes = [r for r in rows if float(r["dt"]) > limit]
print(f"frames {n}  median {dt[n//2]:.1f}  p99 {dt[int(n*0.99)]:.1f}  p99.9 {dt[int(n*0.999)]:.1f}  worst {dt[-1]:.1f} ms;  over {limit:.0f} ms: {len(spikes)}")
cols = ["update", "overlay", "upload", "scene", "draw", "textures", "show"]
tot = {c: sum(float(r[c]) for r in spikes) for c in cols}
print("spend in spike frames (ms total): " + "  ".join(f"{c} {v:.0f}" for c, v in tot.items()))
cause = {}
for r in spikes:
    c = max(cols, key=lambda k: float(r[k]))
    cause[c] = cause.get(c, 0) + 1
print("biggest part per spike: " + "  ".join(f"{k} {v}" for k, v in sorted(cause.items(), key=lambda x: -x[1])))
# What ran in the game's update (`phases`, parts of half a millisecond or more) in the spike frames, summed by name.
ph = {}
for r in spikes:
    for item in (r.get("phases") or "").split("|"):
        if ":" in item:
            k, v = item.rsplit(":", 1)
            ph[k] = ph.get(k, 0.0) + float(v)
if ph:
    print("update phases in spike frames (ms total): " + "  ".join(f"{k} {v:.0f}" for k, v in sorted(ph.items(), key=lambda x: -x[1])[:8]))
if "build" in rows[0]:
    print("draw split in spike frames (ms total): build " + f"{sum(float(r['build']) for r in spikes):.0f}  submit {sum(float(r['submit']) for r in spikes):.0f}")
for r in sorted(spikes, key=lambda r: -float(r["dt"]))[:8]:
    print("  frame %s dt %.0f: " % (r["frame"], float(r["dt"])) + " ".join(f"{c} {float(r[c]):.0f}" for c in cols) + f"  new_tex {r['new_textures']} meshes {r['meshes']} fight {r['fight']} {r['module']}")
