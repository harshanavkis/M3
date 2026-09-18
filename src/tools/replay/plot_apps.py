#!/usr/bin/env python3
"""plot_apps.py [replay.csv] [out-dir]

Fig. B (applications, R1.3/R3.4) from the Part B replay runs (tag partB): per workload, IronBus's
overhead vs. native (on-chip and off-chip) and the host-centric design's slowdown vs. IronBus.
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
x = np.arange(len(names)); width = 0.38
fig, axes = plt.subplots(2, 1, figsize=(0.55 * len(names) + 0.8, 2.6), sharex=True)
for ax, key, ylab, ref in ((axes[0], "ironbus/native", "IronBus overhead\nvs. native (%)", 0),
                           (axes[1], "host/ironbus", "Host-centric slowdown\nvs. IronBus (×)", 1)):
    for i, (lat, lab, color, hatch) in enumerate(((0, "on-chip", palette[1], ""), (500, "off-chip", palette[1], "//"))):
        vals = []
        for w in names:
            v = t.loc[(w, lat), key] if (w, lat) in t.index else np.nan
            vals.append((v - 1) * 100 if key == "ironbus/native" else v)
        bars = ax.bar(x + (i - 0.5) * width, vals, width, color=color if i == 0 else "white", edgecolor=color,
                      hatch=hatch, linewidth=0.6, label=lab)
    style_axis(ax); ax.set_ylabel(ylab, fontsize=5, labelpad=1)
    ax.set_ylim(0, np.nanmax([ax.get_ylim()[1], 1]) * 1.05)
    if ref:
        ax.axhline(1, color="k", linewidth=0.4, linestyle=":")
axes[0].legend(fontsize=4, handletextpad=0.3, borderpad=0.3, edgecolor="k", loc="upper left", ncol=2)
axes[1].set_xticks(x); axes[1].set_xticklabels(labels, fontsize=4.5, rotation=45, ha="right")
plt.tight_layout(pad=0.3)
for ext in ("png", "pdf"):
    plt.savefig(os.path.join(out, "apps." + ext), bbox_inches="tight", dpi=200)
print("->", out)
