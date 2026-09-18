#!/usr/bin/env bash
# Part A of the trace-replay plan (R1.1/R3.1, docs/revision/trace-replay-plan.md): collective
# patterns vs. number of tiles, plus the k-instance concurrency sub-run.
#   usage: sweep-collectives.sh [-p parallel] [patterns|concurrency|all]   (default: all, -p 24)
# Patterns: {all_reduce,all_gather,reduce_scatter,all_to_all,chain} x N={2,4,8,16}
#           x {native,ironbus,host} x {on-chip 0, off-chip 500}    -> tag partA
# Concurrency: k={1,2,4} instances of all_reduce N=4, ironbus, on/off-chip -> tag partA-conc
# Results: tools/replay-runs/out/partA*/<name>.log ("replay ... total: X cycles", "setup ...")
set -u
par=24
while getopts "p:" o; do case $o in p) par=$OPTARG;; esac; done; shift $((OPTIND-1))
what=${1:-all}
here=$(dirname "$0")
S=${REPLAY_OUT:-/scratch/harshanavkis/ironbus/tools/replay-runs}
throttle() { while [ "$(jobs -rp | wc -l)" -ge "$par" ]; do sleep 5; done; }

if [ "$what" = all ] || [ "$what" = patterns ]; then
    for n in 2 4 8 16; do
        for pat in all_reduce all_gather reduce_scatter all_to_all chain; do
            for lat in 0 500; do
                for mode in native ironbus host; do
                    throttle
                    $here/runreplay.sh partA ${pat}_$n $n $mode $lat &
                done
            done
        done
    done
fi
if [ "$what" = all ] || [ "$what" = concurrency ]; then
    for lat in 0 500; do
        for k in 1 2 4; do
            throttle
            REPLAY_INSTANCES=$k $here/runreplay.sh partA-conc all_reduce_4 4 ironbus $lat &
        done
    done
fi
wait

echo "== results (max over ranks; concurrency: per instance)"
for f in $S/out/partA/*.log $S/out/partA-conc/*.log; do
    [ -f "$f" ] || continue
    r=$(grep -o -E 'replay .*total: [0-9]+ cycles' "$f" | sed -E 's/replay //; s/ cycles//' | tr '\n' ';')
    s=$(grep -o -E 'setup .*: activities [0-9]+ channels [0-9]+' "$f" | sed -E 's/.*: //' | head -1)
    printf "%-36s %s  [%s]\n" "$(basename $f .log)" "${r:-FAILED}" "$s"
done
