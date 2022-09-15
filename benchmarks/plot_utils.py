import os
import seaborn as sns
import matplotlib.pyplot as plt

def plot_ipc_benchmarks(ipc_non_secure, ipc_secure, exp_res_path):
    plt.clf()
    secure_encr_latency = ipc_secure.pop("encr_latency")
    non_secure_encr_latency = ipc_non_secure.pop("encr_latency")

    # Collect only cycle data
    non_secure_data = {}
    for i in ipc_non_secure:
        non_secure_data[i] = ipc_non_secure[i]["time"]
    print(non_secure_data)

    secure_data = {}
    for i in ipc_secure:
        secure_data[i] = ipc_secure[i]["time"]
    print(secure_data)

    # Generate relative data
    relative_data = {}
    for i in non_secure_data:
        relative_data[i] = secure_data[i] / non_secure_data[i]
    print(relative_data)

    # Clean up relative data
    plot_data = {}
    for i in relative_data:
        x_axis_data = i.split(")")[0].split(" * ")[-1]
        plot_data[x_axis_data] = relative_data[i]
    
    print(plot_data)

    # Create plot
    plot = sns.barplot(x=list(plot_data.keys()), y=[plot_data[i] for i in plot_data])
    plot.set_xlabel("Bytes")
    plot.set_ylabel("Relative slowdown")
    plot.figure.savefig(os.path.join(exp_res_path, "ipc-plot.png"))

def plot_read_write_benchmarks(rw_non_secure, rw_secure, exp_res_path):
    plt.clf()
    secure_encr_latency = rw_secure.pop("encr_latency")
    non_secure_encr_latency = rw_non_secure.pop("encr_latency")

    # Collect only cycle data
    read_non_secure = {}
    read_unaligned_non_secure = {}
    write_non_secure = {}
    write_unaligned_non_secure = {}
    for i in rw_non_secure:
        if "read unaligned" in i:
            read_unaligned_non_secure[i] = rw_non_secure[i]["time"]
        elif "read" in i:
            read_non_secure[i] = rw_non_secure[i]["time"]
        elif "write unaligned" in i:
            write_unaligned_non_secure[i] = rw_non_secure[i]["time"]
        elif "write" in i:
            write_non_secure[i] = rw_non_secure[i]["time"]
    # print(read_non_secure)
    # print(read_unaligned_non_secure)
    # print(write_non_secure)
    # print(write_unaligned_non_secure)

    read_secure = {}
    read_unaligned_secure = {}
    write_secure = {}
    write_unaligned_secure = {}
    for i in rw_secure:
        if "read unaligned" in i:
            read_unaligned_secure[i] = rw_secure[i]["time"]
        elif "read" in i:
            read_secure[i] = rw_secure[i]["time"]
        elif "write unaligned" in i:
            write_unaligned_secure[i] = rw_secure[i]["time"]
        elif "write" in i:
            write_secure[i] = rw_secure[i]["time"]
    # print(read_secure)
    # print(read_unaligned_secure)
    # print(write_secure)
    # print(write_unaligned_secure)
    
    # Plotting only aligned reads
    read_relative_data = {}
    for i in read_non_secure:
        read_relative_data[i] = read_secure[i] / read_non_secure[i]
    
    read_plot_data = {}
    for i in read_relative_data:
        x_axis_data = i.split(" ")[-2]
        # print(x_axis_data)
        read_plot_data[x_axis_data] = read_relative_data[i]
    
    plot = sns.barplot(x=list(read_plot_data.keys()), y=[read_plot_data[i] for i in read_plot_data])
    plot.set_xlabel("Bytes")
    plot.set_ylabel("Relative slowdown")
    plot.figure.savefig(os.path.join(exp_res_path, "read-plot.png"))
    
    # Plotting only aligned writes
    write_relative_data = {}
    for i in write_non_secure:
        write_relative_data[i] = write_secure[i] / write_non_secure[i]
    
    write_plot_data = {}
    for i in write_relative_data:
        x_axis_data = i.split(" ")[-2]
        # print(x_axis_data)
        write_plot_data[x_axis_data] = write_relative_data[i]
    print(write_plot_data)
    
    plot = sns.barplot(x=list(write_plot_data.keys()), y=[write_plot_data[i] for i in write_plot_data])
    plot.set_xlabel("Bytes")
    plot.set_ylabel("Relative slowdown")
    plot.figure.savefig(os.path.join(exp_res_path, "write-plot.png"))

def plot_app_benchmarks(exp_dict, exp_res_path):
    plt.clf()
    plot_data = {}

    plot_data["sqlite"] = exp_dict["sqlite-secure"]["time"] / exp_dict["sqlite-non-secure"]["time"]
    plot_data["find"] = exp_dict["find-secure"]["time"] / exp_dict["find-non-secure"]["time"]
    plot_data["tar"] = exp_dict["tar-secure"]["time"] / exp_dict["tar-non-secure"]["time"]
    plot_data["untar"] = exp_dict["untar-secure"]["time"] / exp_dict["untar-non-secure"]["time"]

    # print(plot_data)

    plot = sns.barplot(x=list(plot_data.keys()), y=[plot_data[i] for i in plot_data])
    plot.set_xlabel("")
    plot.set_ylabel("Relative slowdown")
    plot.figure.savefig(os.path.join(exp_res_path, "apps-plot.png"))

def plot_syscall_benchmarks(syscall_non_secure, syscall_secure, exp_res_path):
    plt.clf()
    secure_encr_latency = syscall_secure.pop("encr_latency")
    non_secure_encr_latency = syscall_non_secure.pop("encr_latency")

    plot_data = {}

    for i in syscall_non_secure:
        plot_data[i.split(" ")[-1]] = syscall_secure[i]["time"] / syscall_non_secure[i]["time"]
    
    print(plot_data)

    plot = sns.barplot(x=list(plot_data.keys()), y=[plot_data[i] for i in plot_data], color="k")
    plot.set_xlabel("")
    plot.set_ylabel("Relative slowdown")
    plot.set_xticklabels(list(plot_data.keys()), rotation=30, fontsize=6)
    plot.figure.savefig(os.path.join(exp_res_path, "syscall-plot.png"))

def plot_fs_benchmarks(fs_non_secure, fs_secure, exp_res_path):
    plt.clf()
    secure_encr_latency = fs_secure.pop("encr_latency")
    non_secure_encr_latency = fs_non_secure.pop("encr_latency")

    plot_data = {}

    for i in fs_non_secure:
        plot_data[i.split(" ")[1]] = fs_secure[i]["time"] / fs_non_secure[i]["time"]
    
    print(plot_data)

    plot = sns.barplot(x=list(plot_data.keys()), y=[plot_data[i] for i in plot_data], color="k")
    plot.set_xlabel("")
    plot.set_ylabel("Relative slowdown")
    plot.figure.savefig(os.path.join(exp_res_path, "fs-plot.png"))
