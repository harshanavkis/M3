#!/usr/bin/env python3
"""collect.py [out-dir] [csv-dir]

Parses the trace-replay run logs (tools/replay-runs/out/<tag>/<trace>-<N>-<mode>-<lat>[-k<k>].log)
into CSVs: one row per (tag, trace, N, mode, lat, k instances, g groups, instance/group) with the total cycles (max over
ranks), per-rank mean send/recv/comp cycles, bytes per rank and the coordinator's setup times.
Default: out-dir = tools/replay-runs/out, csv-dir = tools/replay-runs (replay.csv)."""
import glob, os, re, sys, csv

out_dir = sys.argv[1] if len(sys.argv) > 1 else "/scratch/harshanavkis/ironbus/tools/replay-runs/out"
csv_dir = sys.argv[2] if len(sys.argv) > 2 else os.path.dirname(out_dir.rstrip("/"))

NAME = re.compile(r"^(?P<trace>.+?)-(?P<n>\d+)-(?P<mode>native|ironbus|host)-(?P<lat>\d+)(?:-k(?P<k>\d+))?(?:-g(?P<g>\d+))?\.log$")
RANK = re.compile(r"rank (\d+): total (\d+) comp (\d+) send (\d+) recv (\d+) ops (\d+) bytes (\d+)")
TOTAL = re.compile(r"replay (\S+) ranks=(\d+) mode=(\w+)(?: inst=(\d+))?: total: (\d+) cycles \(wall (\d+) cycles\)")
SETUP = re.compile(r"setup (\S+) ranks=(\d+) mode=(\w+) inst=(\d+): activities (\d+) channels (\d+) cycles")

rows = []
for f in sorted(glob.glob(os.path.join(out_dir, "*", "*.log"))):
    m = NAME.match(os.path.basename(f))
    if not m:
        continue
    tag = os.path.basename(os.path.dirname(f))
    text = open(f, errors="ignore").read()
    if "exit=0" not in text:
        continue
    setups = {int(s.group(4)): (int(s.group(5)), int(s.group(6))) for s in SETUP.finditer(text)}
    # per-rank lines belong to the instance whose total line follows them
    pending = []
    for line in text.splitlines():
        r = RANK.search(line)
        if r:
            pending.append(tuple(int(x) for x in r.groups()))
            continue
        t = TOTAL.search(line)
        if t:
            inst = int(t.group(4) or 0)
            ranks = pending; pending = []
            n = len(ranks) or int(m.group("n"))
            mean = lambda i: sum(r[i] for r in ranks) / n if ranks else 0
            rows.append({
                "tag": tag, "trace": m.group("trace"), "pattern": re.sub(r"_\d+$", "", m.group("trace")),
                "N": int(m.group("n")), "mode": m.group("mode"), "lat": int(m.group("lat")),
                "k": int(m.group("k") or 1), "g": int(m.group("g") or 1), "inst": inst,
                "total": int(t.group(5)), "wall": int(t.group(6)),
                "comp": mean(2), "send": mean(3), "recv": mean(4), "bytes": mean(6),
                "setup_activities": setups.get(inst, (0, 0))[0], "setup_channels": setups.get(inst, (0, 0))[1],
            })
os.makedirs(csv_dir, exist_ok=True)
path = os.path.join(csv_dir, "replay.csv")
with open(path, "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
    w.writeheader(); w.writerows(rows)
print("%d rows -> %s" % (len(rows), path))
