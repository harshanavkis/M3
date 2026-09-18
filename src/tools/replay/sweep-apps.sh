#!/usr/bin/env bash
# Part B of the trace-replay plan (R1.3/R3.4, docs/revision/trace-replay-plan.md): application
# traces (LLM inference/training, Transformer, DLRM, ResNet-50) at their fixed layouts.
#   usage: sweep-apps.sh [-p parallel] [trace ...]   (default: all traces below, -p 24, small first)
# x {native, ironbus, host} x {on-chip 0, off-chip 500} -> tag partB
set -u
par=24
while getopts "p:" o; do case $o in p) par=$OPTARG;; esac; done; shift $((OPTIND-1))
traces=("$@")
[ ${#traces[@]} -eq 0 ] && traces=(dlrm_4 dlrm_8 llama7b_inf_tp4_s1 resnet50_4 resnet50_8 llama7b_inf_tp2pp2_s512 \
    llama7b_inf_tp4pp2_s512 llama7b_inf_tp4_s512 llama7b_train_tp4pp2 transformer_4 llama7b_train_dp4 \
    llama7b_train_tp4 llama7b_train_dp8 transformer_8)
here=$(dirname "$0")
S=${REPLAY_OUT:-/scratch/harshanavkis/ironbus/tools/replay-runs}
T=/scratch/harshanavkis/ironbus/M3/src/fs/bench/traces
throttle() { while [ "$(jobs -rp | wc -l)" -ge "$par" ]; do sleep 5; done; }
for t in "${traces[@]}"; do
    n=$(ls $T/$t.*.m3t | wc -l)
    for lat in 0 500; do
        for mode in native ironbus host; do
            throttle
            $here/runreplay.sh partB $t $n $mode $lat &
        done
    done
done
wait
echo "== results"
for f in $S/out/partB/*.log; do
    r=$(grep -o -E 'total: [0-9]+ cycles' "$f" | head -1)
    printf "%-40s %s\n" "$(basename $f .log)" "${r:-FAILED}"
done
