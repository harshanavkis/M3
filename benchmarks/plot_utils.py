import os
import seaborn as sns
import pandas as pd
import matplotlib.pyplot as plt
from .constants import *

SYS_FREQ = int(CPU_FREQ.replace("GHz", "")) * 1e9
palette = sns.color_palette("colorblind")
palette = [palette[-1], palette[1]]

# hatches = ["o", "+", "x"]
hatches = ["/", "o", ""]

def change_width(ax, new_value) :
    for i, patch in enumerate(ax.patches):
        current_width = patch.get_width()
        diff = current_width - new_value

        # we change the bar width
        patch.set_width(new_value)

        # we recenter the bar
        if i % 2==0:
            patch.set_x(patch.get_x() + (diff * .5))
        else:
            patch.set_x(patch.get_x() - (diff * .5))

def plot_ipc_benchmarks(ipc_non_secure, ipc_secure, exp_res_path, int_lat):
    plt.clf()
    secure_encr_latency = ipc_secure.pop("encr_latency")
    non_secure_encr_latency = ipc_non_secure.pop("encr_latency")

    # Collect only cycle data
    non_secure_data = {}
    for i in ipc_non_secure:
        non_secure_data[i] = ipc_non_secure[i]["time"]

    secure_data = {}
    for i in ipc_secure:
        secure_data[i] = ipc_secure[i]["time"]

    # Generate relative data
    relative_data = {}
    for i in non_secure_data:
        relative_data[i] = secure_data[i] / non_secure_data[i]

    # Clean up relative data
    plot_data = {}
    for i in relative_data:
        x_axis_data = i.split(")")[0].split(" * ")[-1]
        plot_data[x_axis_data] = relative_data[i]
    
    plot_data = pd.DataFrame(list(plot_data.items()), columns=["Bytes", "Relative slowdown"])
    # print(plot_data)
    plot = sns.catplot(
        kind="bar",
        x=plot_data.columns[0],
        y=plot_data.columns[1],
        data=plot_data,
    )

    ipc_plot_data = plot_data

    # Create plot
    for i, container in enumerate(plot.ax.containers):
        plot.ax.bar_label(container, fmt="%.2f", padding=2, rotation=45)
    plot.ax.set_xlabel("Bytes")
    plot.ax.set_ylabel("Relative slowdown")
    plot.ax.figure.savefig(os.path.join(exp_res_path, "ipc-plot-{}.pdf".format(int_lat)))
    plot.ax.figure.savefig(os.path.join(exp_res_path, "ipc-plot-{}.png".format(int_lat)))

    overhead_data = {}
    for i in secure_data:
        overhead_data[i.split(")")[0].split(" * ")[-1]] = secure_data[i] - non_secure_data[i]
    
    overhead_plot_data = pd.DataFrame(list(overhead_data.items()), columns=["Bytes", "Overheads"])
    plot_data = {}
    for i in secure_data:
        plot_data[i.split(")")[0].split(" * ")[-1]] = secure_data[i]
    
    plot_data = pd.DataFrame(list(plot_data.items()), columns=["Bytes", "Cycles"])

    # print(plot_data)
    # print(overhead_plot_data)

    # plot_data = pd.merge(plot_data, overhead_plot_data, on="Bytes")
    # print(plot_data)

    plt.clf()

    # ax = plt.subplots()
    # ax = sns.barplot(
    #     x=plot_data.columns[0],
    #     y=plot_data.columns[1],
    #     data=plot_data,
    #     color='b'
    # )
    ax = sns.barplot(
        x=overhead_plot_data[overhead_plot_data.columns[0]],
        y=overhead_plot_data[overhead_plot_data.columns[1]],
        # color='r',
    )

    for i, container in enumerate(ax.containers):
        ax.bar_label(container, fmt="%d", padding=2, fontsize=20, rotation=45)

    ax.set_xlabel("Bytes")
    ax.set_ylabel("Cycles")
    # ax.legend(labels=["Encryption", "IPC"])
    ax.figure.savefig(os.path.join(exp_res_path, "ipc-overhead-plot-{}.pdf".format(int_lat)))
    ax.figure.savefig(os.path.join(exp_res_path, "ipc-overhead-plot-{}.png".format(int_lat)))

    return ipc_plot_data.to_dict("list")

def plot_read_write_benchmarks(rw_non_secure, rw_secure, exp_res_path, int_lat):
    plt.clf()
    secure_encr_latency = rw_secure.pop("encr_latency")
    non_secure_encr_latency = rw_non_secure.pop("encr_latency")

    # Collect only cycle data
    read_non_secure = {}
    read_var_ns = {}
    read_unaligned_non_secure = {}
    write_non_secure = {}
    write_var_ns = {}
    write_unaligned_non_secure = {}
    for i in rw_non_secure:
        if "read unaligned" in i:
            read_unaligned_non_secure[i.split(" ")[-2]] = rw_non_secure[i]["time"]
        elif "read" in i:
            read_non_secure[i.split(" ")[-2]] = rw_non_secure[i]["time"]
            read_var_ns[i.split(" ")[-2]] = rw_non_secure[i]["variance"]
        elif "write unaligned" in i:
            write_unaligned_non_secure[i.split(" ")[-2]] = rw_non_secure[i]["time"]
        elif "write" in i:
            write_non_secure[i.split(" ")[-2]] = rw_non_secure[i]["time"]
            write_var_ns[i.split(" ")[-2]] = rw_non_secure[i]["variance"]

    read_secure = {}
    read_var_sec = {}
    read_unaligned_secure = {}
    write_secure = {}
    write_var_sec = {}
    write_unaligned_secure = {}
    for i in rw_secure:
        if "read unaligned" in i:
            read_unaligned_secure[i.split(" ")[-2]] = rw_secure[i]["time"]
        elif "read" in i:
            read_secure[i.split(" ")[-2]] = rw_secure[i]["time"]
            read_var_sec[i.split(" ")[-2]] = rw_secure[i]["variance"]
        elif "write unaligned" in i:
            write_unaligned_secure[i.split(" ")[-2]] = rw_secure[i]["time"]
        elif "write" in i:
            write_secure[i.split(" ")[-2]] = rw_secure[i]["time"]
            write_var_sec[i.split(" ")[-2]] = rw_secure[i]["variance"]
    
    # Plot throughputs: GiB/s
    data_size = 2 / 1024
    read_ns_throughput = {}
    read_sec_throughput = {}
    for i in read_non_secure:
        read_ns_throughput[i] = (data_size * SYS_FREQ) / (read_non_secure[i])
        read_sec_throughput[i] = (data_size * SYS_FREQ) / (read_secure[i])

    write_ns_throughput = {}
    write_sec_throughput = {}
    for i in write_non_secure:
        write_ns_throughput[i] = (data_size * SYS_FREQ) / (write_non_secure[i])
        write_sec_throughput[i] = (data_size * SYS_FREQ) / (write_secure[i])

    for i in read_non_secure:
        read_var_ns[i] = [
            (data_size * SYS_FREQ) / (read_non_secure[i] - read_var_ns[i]),
            (data_size * SYS_FREQ) / read_non_secure[i],
            (data_size * SYS_FREQ) / (read_non_secure[i] + read_var_ns[i])
            ]
        
        read_var_sec[i] = [
            (data_size * SYS_FREQ) / (read_secure[i] - read_var_sec[i]),
            (data_size * SYS_FREQ) / read_secure[i],
            (data_size * SYS_FREQ) / (read_secure[i] + read_var_sec[i])
            ]
        
        write_var_ns[i] = [
            (data_size * SYS_FREQ) / (write_non_secure[i] - write_var_ns[i]),
            (data_size * SYS_FREQ) / write_non_secure[i],
            (data_size * SYS_FREQ) / (write_non_secure[i] + write_var_ns[i])
            ]
        
        write_var_sec[i] = [
            (data_size * SYS_FREQ) / (write_secure[i] - write_var_sec[i]),
            (data_size * SYS_FREQ) / write_secure[i],
            (data_size * SYS_FREQ) / (write_secure[i] + write_var_sec[i])
            ]
    
    # import pdb; pdb.set_trace()

    read_throughput = pd.DataFrame(
        {
            "Bytes": list(read_ns_throughput.keys()) + list(read_ns_throughput.keys()) + list(read_ns_throughput.keys()),
            "Throughput [GiB/s]": list(read_ns_throughput.values()) + (list(read_sec_throughput.values())) + [read_ns_throughput[i] / read_sec_throughput[i] for i in read_ns_throughput],
            "Kind": ["Non-secure" for _ in range(0, len(list(read_ns_throughput.values())))] + ["Secure" for _ in range(0, len(list(read_ns_throughput.values())))] + ["Slowdown" for _ in range(0, len(list(read_ns_throughput.values())))]
        }
    )

    read_slowdown = list(read_throughput[(read_throughput["Kind"] == "Slowdown")]["Throughput [GiB/s]"].values)
    read_throughput = read_throughput[(read_throughput["Kind"] != "Slowdown")]
    read_throughput = read_throughput.replace("Non-secure", "M\u00b3")
    read_throughput = read_throughput.replace("Secure", "THAI")

    # print(read_throughput)
    plot = sns.catplot(
        kind = "bar",
        x = "Bytes",
        y= "Throughput [GiB/s]",
        data = read_throughput,
        hue = "Kind",
        edgecolor="k",
        legend=False,
        height=8,
        aspect=1,
        palette=palette
    )

    read_slowdown = ["{0:.2f}X".format(i) for i in read_slowdown]
    plot.ax.bar_label(plot.ax.containers[1], labels=read_slowdown, fmt="%.2f", padding=8, fontsize=30, rotation='vertical', label_type="edge")

    for bars, hatch in zip(plot.ax.containers, hatches):
        for bar in bars:
            bar.set_hatch(hatch)
    
    plot.ax.legend(loc="upper left", fontsize=30, handletextpad=0.2, borderpad=0.3, edgecolor='k')
    plot.ax.tick_params(axis='both', labelsize=30)
    plot.ax.set_xlabel("Bytes", fontsize = 30)
    plot.ax.set_ylabel("Throughput [GiB/s]", fontsize = 30)
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    # plt.minorticks_on()

    plot.figure.savefig(os.path.join(exp_res_path, "read-throughput-{}.pdf".format(int_lat)), bbox_inches="tight")
    plot.figure.savefig(os.path.join(exp_res_path, "read-throughput-{}.png".format(int_lat)), bbox_inches="tight")
    plt.clf()

    write_throughput = pd.DataFrame(
        {
            "Bytes": list(write_ns_throughput.keys()) + list(write_ns_throughput.keys()) + list(write_ns_throughput.keys()),
            "Throughput [GiB/s]": list(write_ns_throughput.values()) + (list(write_sec_throughput.values())) + [write_ns_throughput[i] / write_sec_throughput[i] for i in write_ns_throughput],
            "Kind": ["Non-secure" for _ in range(0, len(list(write_ns_throughput.values())))] + ["Secure" for _ in range(0, len(list(write_ns_throughput.values())))] + ["Slowdown" for _ in range(0, len(list(write_ns_throughput.values())))]
        }
    )
    write_slowdown = list(write_throughput[(write_throughput["Kind"] == "Slowdown")]["Throughput [GiB/s]"].values)
    print(write_throughput)
    write_throughput = write_throughput[(write_throughput["Kind"] != "Slowdown")]
    write_throughput = write_throughput.replace("Non-secure", "M\u00b3")
    write_throughput = write_throughput.replace("Secure", "THAI")

    plot = sns.catplot(
        kind = "bar",
        x = "Bytes",
        y= "Throughput [GiB/s]",
        data = write_throughput,
        hue = "Kind",
        edgecolor="k",
        legend=False,
        height=8,
        aspect=1,
        palette=palette
    )
    write_slowdown = ["{0:.2f}X".format(i) for i in write_slowdown]
    print(write_slowdown)
    for bars, hatch in zip(plot.ax.containers, hatches):
        for bar in bars:
            bar.set_hatch(hatch)
    
    plot.ax.bar_label(plot.ax.containers[1], labels=write_slowdown, fmt="%.2f", padding=8, fontsize=33, rotation='vertical')
    # for i, container in enumerate(plot.ax.containers):
    #     plot.ax.bar_label(container, fmt="%.2f", padding=2, rotation=45)
    plot.ax.legend(loc="upper left", bbox_to_anchor=(0, 1.1), fontsize=33, handletextpad=0.2, borderpad=0.3, edgecolor='k', columnspacing=0.8, ncol=2)
    plot.ax.tick_params(axis='both', labelsize=33)
    plot.ax.set_xlabel("Bytes", fontsize = 33)
    plot.ax.set_ylabel("Throughput [GiB/s]", fontsize = 33)
    # plt.grid(which="major", axis="y")
    # plt.grid(which="minor", axis="y", alpha=0.5)
    # plt.minorticks_on()

    plot.figure.savefig(os.path.join(exp_res_path, "write-throughput-{}.pdf".format(int_lat)), bbox_inches="tight")
    plot.figure.savefig(os.path.join(exp_res_path, "write-throughput-{}.png".format(int_lat)), bbox_inches="tight")
    plt.clf()

    return (read_throughput.to_dict("list"), write_throughput.to_dict("list"))

def plot_app_benchmarks(exp_dict, exp_res_path, int_lat):
    plt.clf()
    plot_data = {}

    plot_data["sqlite"] = exp_dict["sqlite-secure"]["time"][0] / exp_dict["sqlite-non-secure"]["time"][0]
    plot_data["find"] = exp_dict["find-secure"]["time"][0] / exp_dict["find-non-secure"]["time"][0]
    plot_data["tar"] = exp_dict["tar-secure"]["time"][0] / exp_dict["tar-non-secure"]["time"][0]
    plot_data["untar"] = exp_dict["untar-secure"]["time"][0] / exp_dict["untar-non-secure"]["time"][0]
    plot_data["imgproc"] = exp_dict["imgproc-secure"]["imgproc.cc: imgproc-dir (1 chains)"]["time"] / exp_dict["imgproc-non-secure"]["imgproc.cc: imgproc-dir (1 chains)"]["time"]

    plot_data = pd.DataFrame(list(plot_data.items()), columns=["Application", "Relative slowdown"])

    plot = sns.catplot(
        kind="bar",
        x=plot_data.columns[0],
        y=plot_data.columns[1],
        data=plot_data,
    )

    for i, container in enumerate(plot.ax.containers):
        plot.ax.bar_label(container, fmt="%.2f", padding=2, rotation=45)
    plot.ax.set_xlabel("")
    plot.ax.set_ylabel("Relative slowdown")
    plot.figure.savefig(os.path.join(exp_res_path, "apps-plot-{}.pdf".format(int_lat)))
    plot.figure.savefig(os.path.join(exp_res_path, "apps-plot-{}.png".format(int_lat)))

    app_plot_data = plot_data

    plt.clf()

    plot_data = {}
    plot_data["sqlite"] = exp_dict["sqlite-secure"]["time"][0] - exp_dict["sqlite-non-secure"]["time"][0]
    plot_data["find"] = exp_dict["find-secure"]["time"][0] - exp_dict["find-non-secure"]["time"][0]
    plot_data["tar"] = exp_dict["tar-secure"]["time"][0] - exp_dict["tar-non-secure"]["time"][0]
    plot_data["untar"] = exp_dict["untar-secure"]["time"][0] - exp_dict["untar-non-secure"]["time"][0]
    plot_data["imgproc"] = exp_dict["imgproc-secure"]["imgproc.cc: imgproc-dir (1 chains)"]["time"] - exp_dict["imgproc-non-secure"]["imgproc.cc: imgproc-dir (1 chains)"]["time"]

    plot_data = pd.DataFrame(list(plot_data.items()), columns=["Application", "Overhead"])
    
    plot = sns.catplot(
        kind="bar",
        x=plot_data.columns[0],
        y=plot_data.columns[1],
        data=plot_data,
    )

    plot.ax.set_xlabel("")
    plot.ax.set_ylabel("Overhead in cycles")
    plot.figure.savefig(os.path.join(exp_res_path, "apps-plot-overhead-{}.pdf".format(int_lat)))
    plot.figure.savefig(os.path.join(exp_res_path, "apps-plot-overhead-{}.png".format(int_lat)))

    return app_plot_data.to_dict("list")

def plot_syscall_benchmarks(syscall_non_secure, syscall_secure, exp_res_path, int_lat):
    plt.clf()
    secure_encr_latency = syscall_secure.pop("encr_latency")
    non_secure_encr_latency = syscall_non_secure.pop("encr_latency")

    plot_data = {}

    for i in syscall_non_secure:
        plot_data[i.split(" ")[-1]] = syscall_secure[i]["time"] / syscall_non_secure[i]["time"]

    xlabels = plot_data.keys()
    plot_data = pd.DataFrame(list(plot_data.items()), columns=["Syscall", "Relative slowdown"])

    plot = sns.catplot(
        kind="bar",
        x=plot_data.columns[0],
        y=plot_data.columns[1],
        data=plot_data,
    )

    plot.ax.set_xlabel("")
    plot.ax.set_ylabel("Relative slowdown")
    plot.ax.set_xticklabels(list(xlabels), rotation=30, fontsize=6)
    plot.figure.savefig(os.path.join(exp_res_path, "syscall-plot-{}.pdf".format(int_lat)))
    plot.figure.savefig(os.path.join(exp_res_path, "syscall-plot-{}.png".format(int_lat)))

def plot_fs_benchmarks(fs_non_secure, fs_secure, exp_res_path, int_lat):
    plt.clf()
    secure_encr_latency = fs_secure.pop("encr_latency")
    non_secure_encr_latency = fs_non_secure.pop("encr_latency")

    # Throughput in GiB/s
    file_size = (2 * SYS_FREQ) / 1024
    read_data = []
    write_data = []
    for i in fs_non_secure:
        if "read" in i:
            read_data.append(file_size / fs_non_secure[i]["time"])
        else:
            write_data.append(file_size / fs_non_secure[i]["time"])
    for i in fs_secure:
        if "read" in i:
            read_data.append(file_size / fs_secure[i]["time"])
        else:
            write_data.append(file_size / fs_secure[i]["time"])
    labels = ["Non-secure", "Secure", "Non-secure", "Secure", "Slowdown", "Slowdown"]
    kind = ["Read", "Read", "Write", "Write", "Read", "Write"]

    slowdown = [read_data[0] / read_data[1], write_data[0] / write_data[1]]

    plot_data = pd.DataFrame(
        {
            "Throughput [GiB/s]": read_data + write_data + slowdown,
            "Operation": kind,
            "Kind": labels
        }
    )

    # print(plot_data)

    plot = sns.catplot(
        kind = "bar",
        x = "Operation",
        y= "Throughput [GiB/s]",
        data = plot_data,
        hue = "Kind"
    )

    for i, container in enumerate(plot.ax.containers):
        plot.ax.bar_label(container, fmt="%.2f", padding=2, rotation=45)

    plot.ax.figure.savefig(os.path.join(exp_res_path, "fs-throughput-{}.pdf".format(int_lat)))
    plot.ax.figure.savefig(os.path.join(exp_res_path, "fs-throughput-{}.png".format(int_lat)))

    return plot_data.to_dict('list')

    # plt.clf()

    # for i in fs_non_secure:
    #     plot_data[i.split(" ")[1]] = fs_secure[i]["time"] / fs_non_secure[i]["time"]
    
    # plot_data = pd.DataFrame(list(plot_data.items()), columns=["Operation", "Relative slowdown"])
    # plot = sns.catplot(
    #     kind="bar",
    #     x=plot_data.columns[0],
    #     y=plot_data.columns[1],
    #     data=plot_data,
    # )

    # plot.ax.set_xlabel("")
    # plot.ax.set_ylabel("Relative slowdown")
    # plot.ax.figure.savefig(os.path.join(exp_res_path, "fs-plot.pdf"))
