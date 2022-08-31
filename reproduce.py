#!/usr/bin/env python3

import os
import pathlib
import json
import subprocess
from benchmarks.check_result import parse_output

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

def run_gem5(exp, out_file):
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

def main():
    # Used to resume experiments if the script stops unexpectedly
    with open("snapshot.json", "w+") as f:
        try:
            completed_exp = json.loads(f)
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
        "remote-ipc-secure": run_remote_ipc_secure
    }

    for b, f in benchmark_list.items():
        if b in completed_exp:
            continue
        res = f(exp_res_path)
        completed_exp[b] = res
        # Snapshot current completed experiments before moving to next one
        with open("snapshot.json", "w") as f:
            json.dump(completed_exp, f)

if __name__ == "__main__":
    main()
