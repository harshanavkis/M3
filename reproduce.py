#!/usr/bin/env python3

import os
import pathlib
import json
import subprocess
from benchmarks.check_result import parse_output, parse_apps_output
from benchmarks.plot_utils import plot_ipc_benchmarks, plot_read_write_benchmarks, plot_app_benchmarks, plot_syscall_benchmarks, plot_fs_benchmarks

def generate_env_vars(
    mem_freq = "1GHz",
    encr_latency = "0",
    par_pipe = "1",
    rng_latency = "0",
    sign_latency = "0",
    sign_ver_latency = "0",
    ):
    # Constant environment variables
    os.environ["M3_BUILD"] = "release"
    os.environ["M3_TARGET"] = "gem5"
    os.environ["M3_ISA"] = "riscv"
    os.environ["LD_LIBRARY_PATH"] = os.path.join(
        os.path.realpath("."), "build/cross-riscv/lib/"
        )
    os.environ["M3_FS"] = "bench.img"

    # Benchmark specific environment variables
    os.environ["M3_ENCR_LATENCY"] = encr_latency
    os.environ["M3_PAR_PIPE"] = par_pipe
    os.environ["M3_RNG_LATENCY"] = rng_latency
    os.environ["M3_SIGN_LATENCY"] = sign_latency
    os.environ["M3_SIGN_VER_LATENCY"] = sign_ver_latency

def run_gem5(exp, out_file, apps=False):
    cmd = [
            os.path.join(os.path.realpath("."), "b"),
            "run",
            os.path.join(os.path.realpath("."), "boot/bench-{}.xml".format(exp))
        ]

    with open(out_file, "w+") as f:
        subprocess.run(
            cmd,
            stdout=f,
            stderr=f,
            env=os.environ,
        )
    
    if apps:
        time = parse_apps_output(out_file)
        return {"time": int(time) + 1}

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

def run_remote_ipc_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "remote-ipc-non-secure.log")

    perf_res = run_gem5("remote-ipc", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_remote_ipc_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "remote-ipc-secure.log")

    perf_res = run_gem5("remote-ipc", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_read_write_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "read_write_non_secure.log")

    perf_res = run_gem5("encr-mgate", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_read_write_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "read_write_secure.log")

    perf_res = run_gem5("encr-mgate", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_syscall_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "syscall_non_secure.log")

    perf_res = run_gem5("encr-syscall", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_syscall_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "syscall_secure.log")

    perf_res = run_gem5("encr-syscall", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_fs_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "fs_non_secure.log")

    perf_res = run_gem5("fs", out_file)
    perf_res["encr_latency"] = 0

    return perf_res

def run_fs_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "fs_secure.log")

    perf_res = run_gem5("fs", out_file)
    perf_res["encr_latency"] = 15

    return perf_res

def run_sqlite_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "sqlite_non_secure.log")

    perf_res = run_gem5("fstrace-sqlite", out_file, apps=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_sqlite_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "sqlite_secure.log")

    perf_res = run_gem5("fstrace-sqlite", out_file, apps=True)
    perf_res["encr_latency"] = 15

    return perf_res

def run_find_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "find_non_secure.log")

    perf_res = run_gem5("fstrace-find", out_file, apps=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_find_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "find_secure.log")

    perf_res = run_gem5("fstrace-find", out_file, apps=True)
    perf_res["encr_latency"] = 15

    return perf_res

def run_tar_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "tar_non_secure.log")

    perf_res = run_gem5("fstrace-tar", out_file, apps=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_tar_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "tar_secure.log")

    perf_res = run_gem5("fstrace-tar", out_file, apps=True)
    perf_res["encr_latency"] = 15

    return perf_res

def run_untar_non_secure(exp_res_path):
    generate_env_vars()

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "untar_non_secure.log")

    perf_res = run_gem5("fstrace-untar", out_file, apps=True)
    perf_res["encr_latency"] = 0

    return perf_res

def run_untar_secure(exp_res_path):
    generate_env_vars(encr_latency="15")

    # record experiment data i.e kernel and application logs
    out_file = os.path.join(exp_res_path, "untar_secure.log")

    perf_res = run_gem5("fstrace-untar", out_file, apps=True)
    perf_res["encr_latency"] = 15

    return perf_res

def main():
    # Used to resume experiments if the script stops unexpectedly
    with open("snapshot.json", "r+") as f:
        try:
            completed_exp = json.load(f)
        except TypeError:
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
        "sqlite-non-secure": run_sqlite_non_secure,
        "sqlite-secure": run_sqlite_secure,
        "find-non-secure": run_find_non_secure,
        "find-secure": run_find_secure,
        "tar-non-secure": run_tar_non_secure,
        "tar-secure": run_tar_secure,
        "untar-non-secure": run_untar_non_secure,
        "untar-secure": run_untar_secure,
    }

    for b, f in benchmark_list.items():
        if b in completed_exp:
            print("Skipping: {}".format(b))
            continue
        res = f(exp_res_path)
        completed_exp[b] = res
        # Snapshot current completed experiments before moving to next one
        with open("snapshot.json", "w") as f:
            f.truncate(0)
            json.dump(completed_exp, f)
    
    # Plot IPC benchmarks
    plot_ipc_benchmarks(
        completed_exp["remote-ipc-non-secure"],
        completed_exp["remote-ipc-secure"],
        exp_res_path
    )

    # Plot read write benchmarks
    plot_read_write_benchmarks(
        completed_exp["read-write-non-secure"],
        completed_exp["read-write-secure"],
        exp_res_path
    )

    # Plot syscall benchmarks
    plot_syscall_benchmarks(
        completed_exp["syscall-non-secure"],
        completed_exp["syscall-secure"],
        exp_res_path
    )

    # Plot fs benchmarks
    plot_fs_benchmarks(
        completed_exp["fs-non-secure"],
        completed_exp["fs-secure"],
        exp_res_path
    )

    # Plot application benchmarks
    plot_app_benchmarks(
        completed_exp,
        exp_res_path
    )

if __name__ == "__main__":
    main()
