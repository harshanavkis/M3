#!/usr/bin/env python3
"""plot_apps.py [replay.csv] [out-dir]

Fig. B (applications, R1.3/R3.4) from the Part B replay runs (tag partB): per workload the
completion time of native M3, IronBus and the host-centric design, off-chip, with IronBus's
overhead and the host-centric slowdown over native annotated (the on-chip numbers stay in apps.csv).
Same style as benchmarks/plot_utils.py. Also writes apps.csv."""
import os, sys
import numpy as np
import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import seaborn as sns

csv = sys.argv[1] if len(sys.argv) > 1 else "/scratch/harshanavkis/ironbus/tools/replay-runs/replay.csv"
out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../benchmarks/exp-results")
FREQ = 2e9
WORKLOADS = [  # (trace, label)
    ("llama7b_inf_tp4_s512", "Llama-7B prefill\nTP4, 4 acc."), ("llama7b_inf_tp2pp2_s512", "Llama-7B prefill\nTP2·PP2, 4 acc."),
    ("llama7b_inf_tp4pp2_s512", "Llama-7B prefill\nTP4·PP2, 8 acc."), ("llama7b_inf_tp4_s1", "Llama-7B decode\nTP4, 4 acc."),
    ("llama7b_train_tp4", "Llama-7B train\nTP4, 4 acc."), ("llama7b_train_dp4", "Llama-7B train\nDP4, 4 acc."),
    ("llama7b_train_tp4pp2", "Llama-7B train\nTP4·PP2, 8 acc."), ("llama7b_train_dp8", "Llama-7B train\nDP8, 8 acc."),
    ("transformer_4", "Transformer\nhybrid, 4 acc."), ("transformer_8", "Transformer\nhybrid, 8 acc."),
    ("dlrm_4", "DLRM\n4 acc."), ("dlrm_8", "DLRM\n8 acc."), ("resnet50_4", "ResNet-50\n4 acc."), ("resnet50_8", "ResNet-50\n8 acc."),
]
palette = sns.color_palette("colorblind")

def style_axis(ax, labelsize=5):
    ax.tick_params(axis="both", which="major", labelsize=labelsize, direction="out", length=3, width=0.5, pad=2,
                   bottom=True, left=True, top=False, right=False)
    ax.tick_params(axis="both", which="minor", bottom=False, left=False, top=False, right=False)
    ax.spines["top"].set_visible(False); ax.spines["right"].set_visible(False)

df = pd.read_csv(csv)
b = df[(df.tag == "partB") & (df.k == 1)]
t = b.pivot_table(index=["trace", "lat"], columns="mode", values="total")
t["us_native"] = t["native"] / FREQ * 1e6
t["ironbus/native"] = t["ironbus"] / t["native"]
t["host/native"] = t["host"] / t["native"]
t["host/ironbus"] = t["host"] / t["ironbus"]
print(t[["us_native", "ironbus/native", "host/native", "host/ironbus"]].round(3).to_string())
t.to_csv(os.path.join(out, "apps.csv"))

names = [w for w, _ in WORKLOADS if w in t.index.get_level_values(0)]
labels = [l for w, l in WORKLOADS if w in names]
# the application traces are multi-device workloads: the figure shows the off-chip interconnect
# only (the on-chip numbers are in apps.csv / RESULTS.md)
LAT = 500
x = np.arange(len(names)); width = 0.27
fig, ax = plt.subplots(1, 1, figsize=(0.55 * len(names) + 0.8, 1.7))
systems = (("native", "M3 (native)", palette[-1], -1), ("ironbus", "IronBus", palette[1], 0), ("host", "Host-centric", palette[3], 1))
for mode, label, color, off in systems:
    vals = [t.loc[(w, LAT), mode] / FREQ * 1e6 if (w, LAT) in t.index else np.nan for w in names]
    bars = ax.bar(x + off * width, vals, width, color=color, edgecolor="k", linewidth=0.4, label=label)
    if mode != "native":
        for w, b, v in zip(names, bars, vals):
            nat = t.loc[(w, LAT), "native"]
            r = t.loc[(w, LAT), mode] / nat
            txt = ("+%.0f%%" % ((r - 1) * 100)) if mode == "ironbus" else ("%.1f×" % r)
            ax.text(b.get_x() + b.get_width() / 2, v * 1.08, txt, ha="center", va="bottom", fontsize=3.6, rotation=90)
ax.set_yscale("log", base=10)
ax.set_ylim(ax.get_ylim()[0], ax.get_ylim()[1] * 3)
style_axis(ax)
ax.set_ylabel("Time, off-chip (µs)", fontsize=5, labelpad=1)
ax.set_xticks(x); ax.set_xticklabels(labels, fontsize=4.5, rotation=45, ha="right")
ax.legend(fontsize=4, handletextpad=0.3, borderpad=0.3, edgecolor="k", loc="upper left", ncol=3)
plt.tight_layout(pad=0.3)
for ext in ("png", "pdf"):
    plt.savefig(os.path.join(out, "apps." + ext), bbox_inches="tight", dpi=200)
print("->", out)
