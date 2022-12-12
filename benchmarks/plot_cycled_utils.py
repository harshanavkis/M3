import os
import seaborn as sns
import pandas as pd
import matplotlib.pyplot as plt
from .constants import *
from .plot_utils import change_width

SYS_FREQ = int(CPU_FREQ.replace("GHz", "")) * 1e9
cpu_freq = int(CPU_FREQ.replace("GHz", ""))

# sns.set_palette("gnuplot")
palette = sns.color_palette("colorblind")
palette = [palette[-1], palette[1], palette[2]]
# palette = ["lightskyblue", "peachpuff", "lightgreen"]
# palette = ["black", "darkgrey", "white"]

# hatches = ["o", "+", "x"]
hatches = ["/", "o", ""]

def plot_fs_bench_cycles(cycle_fs, exp_res_path):
    df_list = []

    for l in cycle_fs:
        df_list.append(pd.DataFrame.from_dict(cycle_fs[l]))
    
    aggr_df = pd.concat(df_list, axis=0)
    read_df = aggr_df[aggr_df["Operation"].str.contains("Read")]
    write_df = aggr_df[aggr_df["Operation"].str.contains("Write")]

    cpu_freq = int(CPU_FREQ.replace("GHz", ""))

    read_df["Operation"] = read_df["Operation"].apply(lambda x: int(int(x.split("-")[-1]) / cpu_freq))
    write_df["Operation"] = write_df["Operation"].apply(lambda x: int(int(x.split("-")[-1]) / cpu_freq))

    # print(read_df)

    read_slowdown = list(read_df[(read_df["Kind"] == "Slowdown")]["Throughput [GiB/s]"].values)
    read_df = read_df[(read_df["Kind"] != "Slowdown")]
    read_df = read_df.replace("Non-secure", "M\u00b3")
    read_df = read_df.replace("Secure", "THAI")

    write_slowdown = list(write_df[(write_df["Kind"] == "Slowdown")]["Throughput [GiB/s]"].values)
    write_df = write_df[(write_df["Kind"] != "Slowdown")]
    write_df = write_df.replace("Non-secure", "M\u00b3")
    write_df = write_df.replace("Secure", "THAI")

    sns.set(font_scale=1, style='white')

    # read data
    plot = sns.catplot(
        kind = "bar",
        x = "Operation",
        y= "Throughput [GiB/s]",
        data = read_df,
        hue = "Kind",
        height=8,
        aspect=1,
        legend=False,
        palette=palette,
        edgecolor="k",
    )
    # change_width(plot.ax, .1)
    # for i, container in enumerate(plot.ax.containers):
    #     plot.ax.bar_label(container, fmt="%.2f", padding=2, rotation=45)
    read_slowdown = ["{0:.2f}X".format(i) for i in read_slowdown]

    for bars, hatch in zip(plot.ax.containers, hatches):
        for bar in bars:
            bar.set_hatch(hatch)

    plot.ax.bar_label(plot.ax.containers[1], labels=read_slowdown, fmt="%.2f", padding=8, fontsize=30, rotation="vertical")
    plot.ax.tick_params(axis='both', labelsize=30)
    plot.ax.set_xlabel("Interconnect latency (ns)", fontsize = 30)
    plot.ax.set_ylabel("Throughput [GiB/s]", fontsize = 30)

    plot.ax.legend(loc="upper right", fontsize=30, handletextpad=0.2, borderpad=0.3, edgecolor='k')
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    plt.minorticks_on()
    # plt.grid(which="minor", axis="y")

    plot.ax.figure.savefig(os.path.join(exp_res_path, "fs-throughput-cycles-read.pdf"), bbox_inches="tight")
    plot.ax.figure.savefig(os.path.join(exp_res_path, "fs-throughput-cycles-read.png"), bbox_inches="tight")

    plt.clf()
    # write data
    plot = sns.catplot(
        kind = "bar",
        x = "Operation",
        y= "Throughput [GiB/s]",
        data = write_df,
        hue = "Kind",
        height=8,
        aspect=1,
        legend=False,
        palette=palette,
        edgecolor="k"
    )
    # for i, container in enumerate(plot.ax.containers):
    #     plot.ax.bar_label(container, fmt="%.2f", padding=2, rotation=45)
    write_slowdown = ["{0:.2f}X".format(i) for i in write_slowdown]
    plot.ax.bar_label(plot.ax.containers[1], labels=write_slowdown, fmt="%.2f", padding=8, fontsize=30, rotation="vertical")

    for bars, hatch in zip(plot.ax.containers, hatches):
        for bar in bars:
            bar.set_hatch(hatch)

    plot.ax.legend(loc="upper right", fontsize=30, handletextpad=0.2, borderpad=0.3, edgecolor='k')
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    plt.minorticks_on()
    plot.ax.set_xlabel("Interconnect latency (ns)")
    plot.ax.tick_params(axis='both', labelsize=30)
    plot.ax.set_xlabel("Interconnect latency (ns)", fontsize = 30)
    plot.ax.set_ylabel("Throughput [GiB/s]", fontsize = 30)

    plot.ax.figure.savefig(os.path.join(exp_res_path, "fs-throughput-cycles-write.pdf"), bbox_inches="tight")
    plot.ax.figure.savefig(os.path.join(exp_res_path, "fs-throughput-cycles-write.png"), bbox_inches="tight")

    plt.clf()

def plot_app_cycles(cycle_app, exp_res_path):
    df_list = []

    for l in cycle_app:
        df_list.append(pd.DataFrame.from_dict(cycle_app[l]))
    
    aggr_df = pd.concat(df_list, axis=0)

    aggr_df["Cycles"] = aggr_df["Cycles"].apply(lambda x: "{}ns".format(int(int(x) / cpu_freq)))

    sns.set(font_scale=2, style='white')

    # read data
    plot = sns.catplot(
        kind = "bar",
        x = "Application",
        y= "Relative slowdown",
        data = aggr_df,
        hue = "Cycles",
        height=7,
        aspect=3,
        legend=False,
        palette=palette,
        edgecolor="k"
    )
    for i, container in enumerate(plot.ax.containers):
        plot.ax.bar_label(container, fmt="%.2fX", padding=8, fontsize=40, rotation="vertical")

    for bars, hatch in zip(plot.ax.containers, hatches):
        for bar in bars:
            bar.set_hatch(hatch)

    plt.legend(loc="upper right", ncol=3, bbox_to_anchor=(1, 1.38), fontsize=40, handletextpad=0.2, borderpad=0.3, edgecolor='k', columnspacing=0.8)
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    # plt.minorticks_on()
    plot.ax.set_xlabel("Application", labelpad = 10, fontsize=40)
    plot.ax.set_ylabel("Relative slowdown", labelpad = 10, fontsize=40)
    plot.ax.tick_params(axis='both', labelsize=40)
    # plt.setp(plot.ax.patches, linewidth=2)
    plot.ax.figure.savefig(os.path.join(exp_res_path, "app-cycles.pdf"), bbox_inches="tight")
    plot.ax.figure.savefig(os.path.join(exp_res_path, "app-cycles.png"), bbox_inches='tight')

    plt.clf()

def plot_read_cycles(cycle_read, exp_res_path):
    # print(cycle_read)
    df_list = []

    for l in cycle_read:
        df_list.append(pd.DataFrame.from_dict(cycle_read[l]))

    sns.set(font_scale=5, style='white')

    aggr_df = pd.concat(df_list, axis=0)

    # print(aggr_df)

    slowdown = list(aggr_df[(aggr_df["Kind"] == "Slowdown")]["Throughput [GiB/s]"].values)
    aggr_df = aggr_df[(aggr_df["Kind"] != "Slowdown")]

    # print(slowdown)

    # read data
    plot = sns.catplot(
        kind = "bar",
        x = "Bytes",
        y= "Throughput [GiB/s]",
        data = aggr_df,
        hue = "Kind",
        height=20,
        aspect=4,
        legend=False,
        palette=sns.color_palette("Pastel1")[:3],
        edgecolor="k"
    )
    # for i, container in enumerate(plot.ax.containers):
    #     plot.ax.bar_label(container, labels=["", slowdown[i], ""], fmt="%.2f", padding=2, fontsize=55, rotation=35)
    slowdown = ["{0:.2f}x".format(i) for i in slowdown]
    plot.ax.bar_label(plot.ax.containers[1], labels=slowdown, fmt="%.2f", padding=2, fontsize=55, rotation=35)
    
    plot.ax.legend(loc="upper right", ncol=3)
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    plt.minorticks_on()
    # plot.ax.xticks(rotation=45)

    plot.ax.figure.savefig(os.path.join(exp_res_path, "read-cycles.pdf"))
    plot.ax.figure.savefig(os.path.join(exp_res_path, "read-cycles.png"))

    plt.clf()


def plot_write_cycles(cycle_write, exp_res_path):
    df_list = []

    for l in cycle_write:
        df_list.append(pd.DataFrame.from_dict(cycle_write[l]))

    aggr_df = pd.concat(df_list, axis=0)

    slowdown = list(aggr_df[(aggr_df["Kind"] == "Slowdown")]["Throughput [GiB/s]"].values)
    aggr_df = aggr_df[(aggr_df["Kind"] != "Slowdown")]

    sns.set(font_scale=5, style='white')

    # read data
    plot = sns.catplot(
        kind = "bar",
        x = "Bytes",
        y= "Throughput [GiB/s]",
        data = aggr_df,
        hue = "Kind",
        height=20,
        aspect=4,
        legend=False,
        palette=sns.color_palette("Pastel1")[:3],
        edgecolor="k"
    )
    # for i, container in enumerate(plot.ax.containers):
    #     plot.ax.bar_label(container, fmt="%.2f", padding=2, fontsize=55, rotation=35)
    slowdown = ["{0:.2f}x".format(i) for i in slowdown]
    plot.ax.bar_label(plot.ax.containers[1], labels=slowdown, fmt="%.2f", padding=2, fontsize=55, rotation=35)
    
    plot.ax.legend(loc="upper right", ncol=3)
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    plt.minorticks_on()
    
    # plt.xticks(rotation=45)

    plot.ax.figure.savefig(os.path.join(exp_res_path, "write-cycles.pdf"))
    plot.ax.figure.savefig(os.path.join(exp_res_path, "write-cycles.png"))

def plot_ipc_cycles(cycle_ipc, exp_res_path):
    # print(cycle_ipc)
    plt.clf()
    df_list = []

    for l in cycle_ipc:
        df_list.append(pd.DataFrame.from_dict(cycle_ipc[l]))

    aggr_df = pd.concat(df_list, axis=0)
    aggr_df["Kind"] = aggr_df["Kind"].apply(lambda x: "{}ns".format(int(int(x) / cpu_freq)))
    # print(aggr_df)

    sns.set(font_scale=4, style='white')

    plot = sns.catplot(
        kind = "bar",
        x = "Bytes",
        y= "Relative slowdown",
        hue="Kind",
        data = aggr_df,
        height=7,
        aspect=3,
        legend=False,
        palette=palette,
        edgecolor="k"
    )
    for i, container in enumerate(plot.ax.containers):
        plot.ax.bar_label(container, fmt="%.2fX", padding=8, fontsize=40, rotation="vertical")

    for bars, hatch in zip(plot.ax.containers, hatches):
        for bar in bars:
            bar.set_hatch(hatch)
    
    plot.ax.legend(loc="upper center", ncol=4, bbox_to_anchor=(0.5, 1.65), fontsize=40, handletextpad=0.2, borderpad=0.3, edgecolor='k', columnspacing=0.8)
    plot.ax.set_xlabel("Bits", labelpad = 10, fontsize=40)
    plot.ax.set_ylabel("Relative slowdown", labelpad = 10, fontsize=40)
    plot.ax.tick_params(axis='both', labelsize=40)
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    plt.minorticks_on()
    
    plot.ax.figure.savefig(os.path.join(exp_res_path, "ipc-cycles.pdf"), bbox_inches='tight')
    plot.ax.figure.savefig(os.path.join(exp_res_path, "ipc-cycles.png"), bbox_inches='tight')
