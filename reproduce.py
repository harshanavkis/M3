#!/usr/bin/env python3

import sys
import os
import pathlib
import json
import subprocess
from matplotlib import pyplot as plt
from benchmarks.check_result import parse_output, parse_apps_output, parse_mlapp_output
from benchmarks.plot_utils import plot_ipc_benchmarks, plot_read_write_benchmarks, plot_app_benchmarks, plot_syscall_benchmarks, plot_fs_benchmarks, plot_linux_baseline, plot_ipc_breakdown, plot_tdisp_sim, plot_tcb_breakdown, plot_dma_pipelining
from benchmarks.plot_cycled_utils import *
from benchmarks.constants import *

# INT_LATENCY = ["0", "250", "500", "1000", "1500"]
# INT_LATENCY = ["0", "500", "1000"]
# INT_LATENCY = ["0", "250", "500"]
INT_LATENCY = ["0", "500"]
# INT_LATENCY = ["0"]

CPU_FREQ = "2GHz"
MEM_FREQ = "2GHz"
PAR_PIPE = "1"

# Latency (cycles)
LINUX_SYSCALL = 293

# Latency (cycles)
LINUX_FS_READ_LAT = 1842410
LINUX_FS_WRITE_LAT = 4220592

# Throughput (GiB/s)
LINUX_SYS_FREQ = int(CPU_FREQ.replace("GHz", "")) * 1e9
LINUX_FS_SIZE = (2 * LINUX_SYS_FREQ) / 1024
LINUX_FS_READ_THRU = LINUX_FS_SIZE / LINUX_FS_READ_LAT
LINUX_FS_WRITE_THRU = LINUX_FS_SIZE / LINUX_FS_WRITE_LAT

SNAPSHOT_FILE_NAME = "snapshot-{}-{}-{}.json".format(CPU_FREQ, MEM_FREQ, PAR_PIPE)

def generate_env_vars(
    mem_freq = MEM_FREQ,
    encr_latency = "0",
    par_pipe = PAR_PIPE,
    rng_latency = "0",
    sign_latency = "0",
    sign_ver_latency = "0",
    int_transfer_latency = "0",
    cfg_file = os.path.join("config", "default.py")
    ):
    env_var = {}

    # Constant environment variables
    os.environ["M3_BUILD"] = "release"
    os.environ["M3_TARGET"] = "gem5"
    os.environ["M3_ISA"] = "riscv"
    os.environ["LD_LIBRARY_PATH"] = os.path.join(
        os.path.realpath("."), "build/cross-riscv/lib/"
        )
    os.environ["M3_FS"] = "bench.img"

    # Benchmark specific environment variables
    os.environ["M3_GEM5_CPUFREQ"] = CPU_FREQ
    os.environ["M3_GEM5_MEMFREQ"] = mem_freq
    os.environ["M3_ENCR_LATENCY"] = encr_latency
    os.environ["M3_PAR_PIPE"] = par_pipe
    os.environ["M3_RNG_LATENCY"] = rng_latency
    os.environ["M3_SIGN_LATENCY"] = sign_latency
    os.environ["M3_SIGN_VER_LATENCY"] = sign_ver_latency
    os.environ["M3_INT_TRA_LATENCY"] = "{}".format(int_transfer_latency)
    os.environ["M3_GEM5_CFG"] = cfg_file
    for k, v in PLATFORM_ENV.items():
        os.environ[k] = v

    env_var["M3_BUILD"] = os.environ["M3_BUILD"]
    env_var["M3_TARGET"] = os.environ["M3_TARGET"]
    env_var["M3_ISA"] = os.environ["M3_ISA"]
    env_var["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    env_var["M3_FS"] = os.environ["M3_FS"]
    env_var["M3_GEM5_CPUFREQ"] = os.environ["M3_GEM5_CPUFREQ"]
    env_var["M3_GEM5_MEMFREQ"] = os.environ["M3_GEM5_MEMFREQ"]
    env_var["M3_ENCR_LATENCY"] = os.environ["M3_ENCR_LATENCY"]
    env_var["M3_PAR_PIPE"] = os.environ["M3_PAR_PIPE"]
    env_var["M3_RNG_LATENCY"] = os.environ["M3_RNG_LATENCY"]
    env_var["M3_SIGN_LATENCY"] = os.environ["M3_SIGN_LATENCY"]
    env_var["M3_SIGN_VER_LATENCY"] = os.environ["M3_SIGN_VER_LATENCY"]
    env_var["M3_INT_TRA_LATENCY"] = os.environ["M3_INT_TRA_LATENCY"]
    env_var["M3_GEM5_CFG"] = os.environ["M3_GEM5_CFG"]
    for k in PLATFORM_ENV:
        env_var[k] = os.environ[k]

    return dict(env_var)

# set by main(): skip the build step in each run (the build is done once up front) and use a
# separate output directory per run, so that runs can execute in parallel
SKIP_BUILD = False

def run_gem5(exp, out_file, apps=False, mlapp=False):
    cmd = [os.path.join(os.path.realpath("."), "b")]
    if SKIP_BUILD:
        cmd.append("-n")
        os.environ["M3_OUT"] = os.path.join(
            os.path.realpath("."), "run", os.path.basename(out_file).replace(".log", ""))
        pathlib.Path(os.environ["M3_OUT"]).mkdir(parents=True, exist_ok=True)
    cmd += ["run", os.path.join(os.path.realpath("."), "boot/bench-{}.xml".format(exp))]

    with open(out_file, "w+") as f:
        subprocess.run(
            cmd,
            stdout=f,
            stderr=f,
            env=os.environ,
        )
    
    if apps:
        time = parse_apps_output(out_file)
        # return {"time": time}
        return time
    
    if mlapp:
        return parse_mlapp_output(out_file)

    res = parse_output(out_file)

    perf_res = {}

    for name, p in res.perfs.items():
        perf_res[name] = {
            "time": p.time,
            "unit": p.unit,
            "variance": p.variance,
            "runs": p.runs
        }

    return perf_res

def run_remote_ipc_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "remote-ipc-non-secure-{}.log".format(int_latency))

    perf_res = run_gem5("remote-ipc", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_remote_ipc_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "remote-ipc-secure-{}.log".format(int_latency))

    perf_res = run_gem5("remote-ipc", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_read_write_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "read_write_non_secure-{}.log".format(int_latency))

    perf_res = run_gem5("encr-mgate", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_read_write_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "read_write_secure-{}.log".format(int_latency))

    perf_res = run_gem5("encr-mgate", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_syscall_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "syscall_non_secure-{}.log".format(int_latency))

    perf_res = run_gem5("encr-syscall", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_syscall_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "syscall_secure-{}.log".format(int_latency))

    perf_res = run_gem5("encr-syscall", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_fs_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "fs_non_secure-{}.log".format(int_latency))

    perf_res = run_gem5("fs", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_fs_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "fs_secure-{}.log".format(int_latency))

    perf_res = run_gem5("fs", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_sqlite_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "sqlite_non_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-sqlite", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 0

    return perf_res

def run_sqlite_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "sqlite_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-sqlite", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 15

    return perf_res

def run_find_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "find_non_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-find", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 0

    return perf_res

def run_find_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "find_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-find", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 15

    return perf_res

def run_tar_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "tar_non_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-tar", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 0

    return perf_res

def run_tar_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "tar_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-tar", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 15

    return perf_res

def run_untar_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "untar_non_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-untar", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 0

    return perf_res

def run_untar_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency)

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "untar_secure-{}.log".format(int_latency))

    perf_res = {"time": []}
    for i in range(5):
        time = run_gem5("fstrace-untar", out_file, apps=True)
        perf_res["time"].append(time)
    perf_res["encr_latency"] = 15

    return perf_res

def run_imgproc_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "imgproc_non_secure-{}.log".format(int_latency))

    perf_res = run_gem5("imgproc", out_file, apps=False)
    perf_res["encr_latency"] = 0

    return perf_res

def run_imgproc_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "imgproc_secure-{}.log".format(int_latency))

    perf_res = run_gem5("imgproc", out_file, apps=False)
    perf_res["encr_latency"] = 15

    return perf_res

def run_facever_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "facever_non_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("facever", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_facever_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "facever_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("facever", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 15

    return perf_res

def run_img_class_smv_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "img_class_smv_non_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("img-class-smv", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_img_class_smv_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "img_class_smv_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("img-class-smv", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 15

    return perf_res

def run_img_class_systolic_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "img_class_systolic_non_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("img-class-systolic", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_img_class_systolic_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "img_class_systolic_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("img-class-systolic", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 15

    return perf_res

def run_img_class_distinf_non_secure(exp_res_path, int_latency):
    generate_env_vars(int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "img_class_distinf_non_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("img-class-dist", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_img_class_distinf_secure(exp_res_path, int_latency):
    generate_env_vars(encr_latency="15", int_transfer_latency=int_latency, cfg_file=os.path.join(os.path.realpath("."), "config/accels.py"))

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "img_class_distinf_secure-{}.log".format(int_latency))

    perf_res = {}

    perf_res["time"] = run_gem5("img-class-dist", out_file, apps=False, mlapp=True)
    perf_res["encr_latency"] = 15

    return perf_res

# DMA pipelining sweep (bench-p2p-spm: device-to-device transfers between two SPM tiles) with
# the number of packets in flight per command; the 64 GB/s variant (32 B crossbar) additionally
# varies the number of AES-GCM engines per AIU.
P2P_INFLIGHT = [1, 2, 4, 8, 16]
P2P_ENGINES = [1, 2, 4]
P2P_XBAR_WIDTHS = [8, 16, 32]  # 16, 32, 64 GB/s; beyond that the tile datapath (~61 GB/s) limits, not the link

def run_p2p_dma(exp_res_path, int_latency, inflight, secure, engines=None, xbar_width=None):
    encr = "15" if secure else "0"
    generate_env_vars(encr_latency=encr, int_transfer_latency=int_latency)
    os.environ["M3_GEM5_INFLIGHT"] = str(inflight)
    os.environ["M3_GEM5_BUFCOUNT"] = str(max(4, inflight))
    if engines is not None:
        os.environ["M3_GEM5_CRYPTO_ENGINES"] = str(engines)
    if xbar_width is not None:
        os.environ["M3_GEM5_XBAR_WIDTH"] = str(xbar_width)

    name = "p2p_dma_{}_{}".format("secure" if secure else "non_secure", inflight)
    if engines is not None:
        name += "_eng{}".format(engines)
    if xbar_width is not None:
        name += "_xbar{}".format(xbar_width)
    out_file = os.path.join(exp_res_path, "{}-{}.log".format(name, int_latency))

    perf_res = run_gem5("p2p-spm", out_file)
    perf_res["encr_latency"] = int(encr)
    perf_res["inflight"] = inflight
    perf_res["engines"] = engines
    perf_res["xbar_width"] = xbar_width
    return perf_res

def p2p_dma_benchmarks():
    benchs = {}
    for n in P2P_INFLIGHT:
        benchs["p2p-dma-non-secure-{}".format(n)] = \
            (lambda n: lambda path, lat: run_p2p_dma(path, lat, n, False))(n)
        benchs["p2p-dma-secure-{}".format(n)] = \
            (lambda n: lambda path, lat: run_p2p_dma(path, lat, n, True))(n)
    # link bandwidth sweep (crossbar width in bytes/cycle at 2 GHz: 8 -> 16 GB/s ... 64 -> 128
    # GB/s): native and IronBus with 1, 2 and 4 engines, enough packets in flight for all links
    for w in P2P_XBAR_WIDTHS:
        benchs["p2p-dma-link{}-non-secure".format(w)] = \
            (lambda w: lambda path, lat: run_p2p_dma(path, lat, 64, False, xbar_width=w))(w)
        for k in P2P_ENGINES:
            benchs["p2p-dma-link{}-secure-eng{}".format(w, k)] = \
                (lambda w, k: lambda path, lat: run_p2p_dma(path, lat, 64, True, engines=k, xbar_width=w))(w, k)
    return benchs

BENCHMARKS = {}

def _run_one(bench, lat, exp_res_path):
    return BENCHMARKS[bench](exp_res_path, lat)

def main():
    # Used to resume experiments if the script stops unexpectedly
    try:
        with open("snapshot.json", "r+") as f:
            try:
                completed_exp = json.load(f)
            except Exception as e:
                print(e)
                # sys.exit(0)
                completed_exp = {}
    except Exception as e:
        print(e)
        completed_exp = {}
    
    # Create directory to hold experiment data
    exp_res_path = os.path.join(
            os.path.realpath("."), "benchmarks/exp-results"
        )
    pathlib.Path(exp_res_path).mkdir(
            parents = True,
            exist_ok = True
        )

    benchmark_list = {
        "remote-ipc-non-secure": run_remote_ipc_non_secure,
        "remote-ipc-secure": run_remote_ipc_secure,
        "read-write-non-secure": run_read_write_non_secure,
        "read-write-secure": run_read_write_secure,
        "syscall-non-secure": run_syscall_non_secure,
        "syscall-secure": run_syscall_secure,
        "fs-non-secure": run_fs_non_secure,
        "fs-secure": run_fs_secure,
        # "sqlite-non-secure": run_sqlite_non_secure,
        # "sqlite-secure": run_sqlite_secure,
        # "find-non-secure": run_find_non_secure,
        # "find-secure": run_find_secure,
        # "tar-non-secure": run_tar_non_secure,
        # "tar-secure": run_tar_secure,
        # "untar-non-secure": run_untar_non_secure,
        # "untar-secure": run_untar_secure,
        "imgproc-non-secure": run_imgproc_non_secure,
        "imgproc-secure": run_imgproc_secure,
        "facever-non-secure": run_facever_non_secure,
        "facever-secure": run_facever_secure,
        # "img-class-smv-non-secure": run_img_class_smv_non_secure,
        # "img-class-smv-secure": run_img_class_smv_secure,
        "img-class-systolic-non-secure": run_img_class_systolic_non_secure,
        "img-class-systolic-secure": run_img_class_systolic_secure,
        "img-class-distinf-non-secure": run_img_class_distinf_non_secure,
        "img-class-distinf-secure": run_img_class_distinf_secure,
    }
    benchmark_list.update(p2p_dma_benchmarks())
    BENCHMARKS.update(benchmark_list)

    # Build once, then run up to N experiments concurrently (each in its own process, since the
    # benchmark functions configure the environment globally). Default: one gem5 per core, at
    # most 32; --parallel 1 runs sequentially (building before every run as before).
    parallel = min(32, os.cpu_count() or 1)
    if "--parallel" in sys.argv:
        parallel = int(sys.argv[sys.argv.index("--parallel") + 1])

    todo = []
    for l in INT_LATENCY:
        if l not in completed_exp:
                completed_exp[l] = {}
        env_vars = generate_env_vars(int_transfer_latency=l)
        completed_exp[l]["ENV_VAR"] = env_vars

        for b, f in benchmark_list.items():
            if l in completed_exp:
                if b in completed_exp[l]:
                    print("Skipping: {}, {}".format(b, l))
                    continue
            todo.append((b, l))

    def save_snapshot():
        for name in ("snapshot.json", SNAPSHOT_FILE_NAME):
            with open(name, "w") as f:
                f.truncate(0)
                json.dump(completed_exp, f)

    if parallel > 1 and len(todo) > 0:
        global SKIP_BUILD
        # build once up front
        generate_env_vars()
        subprocess.run([os.path.join(os.path.realpath("."), "b")], env=os.environ, check=True)
        SKIP_BUILD = True

        import multiprocessing
        with multiprocessing.get_context("fork").Pool(parallel) as pool:
            results = pool.starmap(_run_one, [(b, l, exp_res_path) for b, l in todo])
        for (b, l), res in zip(todo, results):
            completed_exp[l][b] = res
        save_snapshot()
    else:
        for b, l in todo:
            completed_exp[l][b] = benchmark_list[b](exp_res_path, l)
            # Snapshot current completed experiments before moving to next one
            save_snapshot()
    
    # Grouped cycle data
    cycle_fs = {}
    cycle_app = {}
    cycle_read = {}
    cycle_write = {}
    cycle_ipc = {}
    cycle_acc_apps = {"Time [ns]": [], "Application": [], "Slowdown": []}
    linux_baselines = {}

    for l in INT_LATENCY:
        # Plot IPC benchmarks
        cycle_ipc[l] = plot_ipc_benchmarks(
            completed_exp[l]["remote-ipc-non-secure"],
            completed_exp[l]["remote-ipc-secure"],
            exp_res_path,
            l
        )

        cycle_ipc[l]["Kind"] = [l] * len(cycle_ipc[l]["Bytes"])

        # Plot read write benchmarks
        cycle_read[l], cycle_write[l] = plot_read_write_benchmarks(
            completed_exp[l]["read-write-non-secure"],
            completed_exp[l]["read-write-secure"],
            exp_res_path,
            l
        )
        # print(cycle_read)
        for i, _ in enumerate(cycle_read[l]["Bytes"]):
            cycle_read[l]["Bytes"][i] = "{}-{}".format(cycle_read[l]["Bytes"][i], l)

        for i, _ in enumerate(cycle_write[l]["Bytes"]):
            cycle_write[l]["Bytes"][i] = "{}-{}".format(cycle_write[l]["Bytes"][i], l)

        # Plot syscall benchmarks
        # plot_syscall_benchmarks(
        #     completed_exp[l]["syscall-non-secure"],
        #     completed_exp[l]["syscall-secure"],
        #     exp_res_path,
        #     l
        # )

        # Plot fs benchmarks        
        cycle_fs[l] = plot_fs_benchmarks(
            completed_exp[l]["fs-non-secure"],
            completed_exp[l]["fs-secure"],
            exp_res_path,
            l
        )
        # print(cycle_fs)
        for i, op in enumerate(cycle_fs[l]["Operation"]):
            cycle_fs[l]["Operation"][i] = "{}-{}".format(cycle_fs[l]["Operation"][i], l)

        # Plot application benchmarks
        # cycle_app[l] = plot_app_benchmarks(
        #     completed_exp[l],
        #     exp_res_path,
        #     l
        # )
        # cycle_app[l]["Cycles"] = [l] * len(cycle_app[l]["Application"])
        # print(cycle_app) 

        plt.close("all")

    # Plot cycled filesystem benchmark
    (fs_read_df, fs_write_df) = plot_fs_bench_cycles(cycle_fs, exp_res_path)

    # Plot cycled app benchmark
    # plot_app_cycles(cycle_app, exp_res_path)

    # Plot cycled read benchmark
    plot_read_cycles(cycle_read, exp_res_path)

    # Plot cycled write benchmark
    plot_write_cycles(cycle_write, exp_res_path)

    # Plot cycled IPC benchmark
    plot_ipc_cycles(cycle_ipc, exp_res_path)

    # Plot DMA pipelining (packets in flight, crypto engines)
    plot_dma_pipelining(completed_exp, INT_LATENCY, P2P_INFLIGHT, P2P_ENGINES, P2P_XBAR_WIDTHS, exp_res_path)

    # Process the filesystem data
    # print("Bitch")
    # print(fs_read_df)
    # print(fs_write_df)

    for i in range(len(fs_read_df["Operation"])):
        cycle_acc_apps["Application"].append("m3fs-read")
        cycle_acc_apps["Slowdown"].append(fs_read_df["Throughput [GiB/s]"][i])
        cycle_acc_apps["Time [ns]"].append(fs_read_df["Operation"][i].split("-")[-1])
    
    for i in range(len(fs_write_df["Operation"])):
        cycle_acc_apps["Application"].append("m3fs-write")
        cycle_acc_apps["Slowdown"].append(fs_write_df["Throughput [GiB/s]"][i])
        cycle_acc_apps["Time [ns]"].append(fs_write_df["Operation"][i].split("-")[-1])

    # Plot accelerated applications
    for l in INT_LATENCY:
        for i in range(0, 4):
            cycle_acc_apps["Time [ns]"].append(l)

        cycle_acc_apps["Application"].append("imgproc")
        cycle_acc_apps["Slowdown"].append(completed_exp[l]["imgproc-secure"]["imgproc.cc: imgproc-dir (1 chains)"]["time"] / completed_exp[l]["imgproc-non-secure"]["imgproc.cc: imgproc-dir (1 chains)"]["time"])

        cycle_acc_apps["Application"].append("facever")
        cycle_acc_apps["Slowdown"].append(float(completed_exp[l]["facever-secure"]["time"]) / float(completed_exp[l]["facever-non-secure"]["time"]))

        # cycle_acc_apps["Application"].append("img-class-smv")
        # cycle_acc_apps["Slowdown"].append(float(completed_exp[l]["img-class-smv-secure"]["time"]) / float(completed_exp[l]["img-class-smv-non-secure"]["time"]))

        # cycle_acc_apps["Application"].append("systolic")
        cycle_acc_apps["Application"].append("imgclass")
        cycle_acc_apps["Slowdown"].append(float(completed_exp[l]["img-class-systolic-secure"]["time"]) / float(completed_exp[l]["img-class-systolic-non-secure"]["time"]))

        # cycle_acc_apps["Application"].append("dist")
        cycle_acc_apps["Application"].append("dist")
        cycle_acc_apps["Slowdown"].append(float(completed_exp[l]["img-class-distinf-secure"]["time"]) / float(completed_exp[l]["img-class-distinf-non-secure"]["time"]))

        # cycle_acc_apps["Application"].append("m3fs-write")

    # print(cycle_acc_apps)
    plot_acc_app_cycles(cycle_acc_apps, exp_res_path)

    print("Linux read/write")
    # print(cycle_fs)
    m3fs_read_thru = cycle_fs['0']['Throughput [GiB/s]'][0]
    thai_read_thru = cycle_fs['0']['Throughput [GiB/s]'][1]
    m3fs_write_thru = cycle_fs['0']['Throughput [GiB/s]'][2]
    thai_write_thru = cycle_fs['0']['Throughput [GiB/s]'][3]
    # print(m3fs_read_thru)
    # print(m3fs_write_thru)
    # print(thai_read_thru)
    # print(thai_write_thru)
    # print(LINUX_FS_READ_THRU)
    # print(LINUX_FS_WRITE_THRU)

    # print("Linux syscalls")
    m3_syscall_no_op = completed_exp["0"]["syscall-non-secure"]["bsyscall.rs: noop"]["time"]
    thai_syscall_no_op = completed_exp["0"]["syscall-secure"]["bsyscall.rs: noop"]["time"]
    # print(m3_syscall_no_op)
    # print(thai_syscall_no_op)
    # print(LINUX_SYSCALL)

    plot_linux_baseline(
        LINUX_SYSCALL, m3_syscall_no_op, thai_syscall_no_op,
        LINUX_FS_READ_THRU, m3fs_read_thru, thai_read_thru,
        LINUX_FS_WRITE_THRU, m3fs_write_thru, thai_write_thru,
        exp_res_path
    )

    breakdown_ipc = {}
    breakdown_ipc["non-secure"] = completed_exp["0"]["remote-ipc-non-secure"]
    breakdown_ipc["secure"] = completed_exp["0"]["remote-ipc-secure"]

    plot_ipc_breakdown(breakdown_ipc, exp_res_path)

    breakdown_tdisp_sim = {}
    breakdown_tdisp_sim["Host-centric"] = [1137, 2257, 3423]
    breakdown_tdisp_sim["IronBus"] = [566, 1127, 1693]
    plot_tdisp_sim(breakdown_tdisp_sim, exp_res_path)

    breakdown_tcb = {}
    breakdown_tcb["Kernel"] = [2.513, 3.068, 0.057, 0.033]
    breakdown_tcb["Firmware"] = [0.029, 0.045, 0.045, 0]
    plot_tcb_breakdown(breakdown_tcb, exp_res_path)
    

if __name__ == "__main__":
    main()
