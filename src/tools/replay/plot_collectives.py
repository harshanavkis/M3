#!/usr/bin/env python3
"""plot_collectives.py [replay.csv] [out-dir]

Fig. A (scaling, R1.1/R3.1) from the Part A replay runs (tag partA): per pattern, completion
time vs. N for native / IronBus / host-centric (off-chip, log scale) and IronBus's overhead vs.
native (on-chip and off-chip). Fig. A2: HAL setup (channel creation) vs. N and the k-instance
concurrency run (tag partA-conc). Same style as benchmarks/plot_utils.py."""
import os, sys
import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import seaborn as sns
from matplotlib.ticker import NullFormatter

csv = sys.argv[1] if len(sys.argv) > 1 else "/scratch/harshanavkis/ironbus/tools/replay-runs/replay.csv"
out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../benchmarks/exp-results")
FREQ = 2e9
PATTERNS = [("all_reduce", "All-reduce"), ("all_gather", "All-gather"), ("reduce_scatter", "Reduce-scatter"),
            ("all_to_all", "All-to-all"), ("chain", "PP chain")]
NS = [2, 4, 8, 16]
palette = sns.color_palette("colorblind")
COLORS = {"native": palette[-1], "ironbus": palette[1], "host": palette[3]}
LABELS = {"native": "M3 (native)", "ironbus": "IronBus", "host": "Host-centric"}
MARKERS = {"native": "o", "ironbus": "s", "host": "^"}

def style_axis(ax, labelsize=5):
    ax.tick_params(axis="both", which="major", labelsize=labelsize, direction="out", length=3, width=0.5, pad=2,
                   bottom=True, left=True, top=False, right=False)
    ax.tick_params(axis="both", which="minor", bottom=False, left=False, top=False, right=False)
    ax.spines["top"].set_visible(False); ax.spines["right"].set_visible(False)

df = pd.read_csv(csv)
a = df[(df.tag == "partA") & (df.k == 1)].copy()
a["us"] = a.total / FREQ * 1e6
tab = a.pivot_table(index=["pattern", "N", "lat"], columns="mode", values="us")
tab["ironbus/native"] = tab["ironbus"] / tab["native"]; tab["host/native"] = tab["host"] / tab["native"]
print(tab.round(3).to_string())
tab.to_csv(os.path.join(out, "collectives.csv"))

# ---- Fig. A: completion time vs. N per pattern, off-chip, native / IronBus / host-centric -------
fig, axes = plt.subplots(1, len(PATTERNS), figsize=(1.45 * len(PATTERNS), 1.35))
for col, (pat, title) in enumerate(PATTERNS):
    ax = axes[col]
    sub = a[(a.pattern == pat) & (a.lat == 500)]
    for mode in ("native", "ironbus", "host"):
        d = sub[sub["mode"] == mode].sort_values("N")
        ax.plot(d.N, d.us, marker=MARKERS[mode], markersize=2.5, linewidth=0.8, color=COLORS[mode], label=LABELS[mode])
    ax.set_xscale("log", base=2); ax.set_yscale("log", base=10)
    ax.set_xticks(NS); ax.set_xticklabels([str(n) for n in NS])
    ax.yaxis.set_minor_formatter(NullFormatter())
    style_axis(ax); ax.set_title(title, fontsize=6, pad=2)
    ax.set_xlabel("Accelerator tiles N", fontsize=5, labelpad=1)
    if col == 0:
        ax.set_ylabel("Time, off-chip (µs)", fontsize=5, labelpad=1)
        ax.legend(fontsize=4, handletextpad=0.3, borderpad=0.3, edgecolor="k", loc="upper left")
plt.tight_layout(pad=0.3)
for ext in ("png", "pdf"):
    plt.savefig(os.path.join(out, "collectives." + ext), bbox_inches="tight", dpi=200)

# ---- Fig. A2: HAL setup vs. N, tenants (runtime, setup) — off-chip, M3 / IronBus / host ------
plt.clf()
LAT = 500
fig, axes = plt.subplots(1, 3, figsize=(5.2, 1.4))
# (a) channel setup vs. N for the all-reduce (the collective of all three panels): channels
# follow the pattern's edges — a ring needs N channels, host-centric 2N (to and from the relay)
ax = axes[0]
for mode in ("native", "ironbus", "host"):
    sset = a[(a.pattern == "all_reduce") & (a["mode"] == mode) & (a.lat == LAT)].sort_values("N")
    ax.plot(sset.N, sset.setup_channels / FREQ * 1e3, marker=MARKERS[mode], markersize=2.5, linewidth=0.8, color=COLORS[mode], label=LABELS[mode])
ax.set_xscale("log", base=2); ax.set_xticks(NS); ax.set_xticklabels([str(n) for n in NS])
ax.set_ylim(0, ax.get_ylim()[1] * 1.15)
style_axis(ax); ax.set_title("(a) Channel setup (capabilities + keys) vs. tiles", fontsize=6, pad=2)
ax.set_xlabel("Accelerator tiles N", fontsize=5, labelpad=1); ax.set_ylabel("Setup time (ms)", fontsize=5, labelpad=1)
ax.legend(fontsize=4, handletextpad=0.3, borderpad=0.3, edgecolor="k", loc="upper left")

c = df[df.tag == "partA-conc"].copy()
# single-tenant points of M3 and host-centric are the Part A runs (one coordinator, one relay)
c = pd.concat([c, df[(df.tag == "partA") & (df.trace == "all_reduce_4") & (df["mode"] != "ironbus") & (df.k == 1) & (df.g == 1)]], ignore_index=True)
c = c[c.lat == LAT]
c["us"] = c.total / FREQ * 1e6
# tenants: M3 and IronBus = k independent coordinators (column k); host-centric = k groups
# sharing one relay under one coordinator (column g) — a single host for all tenants
c["tenants"] = c[["k", "g"]].max(axis=1)
chk = c[(c["mode"] == "ironbus") & (c.g > 1)]
if len(chk):
    print("IronBus with groups under one coordinator (consistency check vs. instances):")
    print(chk.groupby("tenants").us.max().to_string())
c = c[((c["mode"] != "host") & (c.g == 1)) | ((c["mode"] == "host") & (c.k == 1))]
c = c.drop_duplicates(subset=["mode", "tenants", "inst"])
g = c.groupby(["mode", "tenants"]).agg(us=("us", "mean"), us_max=("us", "max"), setup=("setup_channels", "mean")).reset_index()
print(g.to_string(index=False))
g.to_csv(os.path.join(out, "collectives-concurrency.csv"), index=False)
ks = sorted(g.tenants.unique())
# (b) runtime per tenant
ax = axes[1]
for mode in ("native", "ironbus", "host"):
    sel = g[g["mode"] == mode].sort_values("tenants")
    ax.plot(sel.tenants, sel.us_max, marker=MARKERS[mode], markersize=2.5, linewidth=0.8, color=COLORS[mode], label=LABELS[mode])
ax.set_xscale("log", base=2); ax.set_xticks(ks); ax.set_xticklabels([str(k) for k in ks])
ax.set_yscale("log", base=10); ax.yaxis.set_minor_formatter(NullFormatter())
style_axis(ax); ax.set_title("(b) Runtime per tenant", fontsize=6, pad=2)
ax.set_xlabel("Concurrent tenants k", fontsize=5, labelpad=1); ax.set_ylabel("Time per tenant (µs)", fontsize=5, labelpad=1)
ax.legend(fontsize=4, handletextpad=0.3, borderpad=0.3, edgecolor="k", loc="center right")
# (c) channel setup per tenant (own coordinator each): M3, IronBus
ax = axes[2]
for mode in ("native", "ironbus"):
    sel = g[g["mode"] == mode].sort_values("tenants")
    ax.plot(sel.tenants, sel.setup / FREQ * 1e3, marker=MARKERS[mode], markersize=2.5, linewidth=0.8, color=COLORS[mode], label=LABELS[mode])
ax.set_xscale("log", base=2); ax.set_xticks(ks); ax.set_xticklabels([str(k) for k in ks])
ax.set_ylim(0, ax.get_ylim()[1] * 1.15); style_axis(ax); ax.set_title("(c) Channel setup per tenant", fontsize=6, pad=2)
ax.set_xlabel("Concurrent tenants k", fontsize=5, labelpad=1); ax.set_ylabel("Setup per tenant (ms)", fontsize=5, labelpad=1)
ax.legend(fontsize=4, handletextpad=0.3, borderpad=0.3, edgecolor="k", loc="upper left")
plt.tight_layout(pad=0.3)
for ext in ("png", "pdf"):
    plt.savefig(os.path.join(out, "collectives-setup." + ext), bbox_inches="tight", dpi=200)
print("->", out)
