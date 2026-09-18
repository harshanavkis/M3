#!/usr/bin/env bash
# E2 of the engine-scaling matrix (R1.2, docs/revision/RESULTS.md section 4): full-duplex
# collectives vs. the number of AES-GCM engines per link direction and the link bandwidth.
#   usage: sweep-engines.sh [trace ...]       (default: all_reduce_4 all_reduce_8)
# Runs, off-chip (250 ns), in parallel:
#   32 GB/s: native | IronBus 1 shared engine | 1 per direction (platform) | 2 per direction
#   64 GB/s: native | 1 per direction | 2 per direction
# Results: tools/replay-runs/out/eng-<config>/<trace>-<N>-<mode>-500.log ("replay ... total: X cycles")
set -u
traces=("$@"); [ ${#traces[@]} -eq 0 ] && traces=(all_reduce_4 all_reduce_8)
here=$(dirname "$0")
for t in "${traces[@]}"; do
    n=${t##*_}
    # 32 GB/s link (platform: M3_GEM5_XBAR_WIDTH=16 B/cycle)
    $here/runreplay.sh eng-32-native  $t $n native  500 &
    $here/runreplay.sh eng-32-shared1 $t $n ironbus 500 M3_GEM5_CRYPTO_ENGINES=1 M3_GEM5_CRYPTO_SHARED=1 &
    $here/runreplay.sh eng-32-dir1    $t $n ironbus 500 M3_GEM5_CRYPTO_ENGINES=1 &
    $here/runreplay.sh eng-32-dir2    $t $n ironbus 500 M3_GEM5_CRYPTO_ENGINES=2 &
    # 64 GB/s link (32 B/cycle), 32 packets in flight for the doubled bandwidth-delay product
    $here/runreplay.sh eng-64-native  $t $n native  500 M3_GEM5_XBAR_WIDTH=32 M3_GEM5_INFLIGHT=32 M3_GEM5_BUFCOUNT=32 &
    $here/runreplay.sh eng-64-dir1    $t $n ironbus 500 M3_GEM5_XBAR_WIDTH=32 M3_GEM5_INFLIGHT=32 M3_GEM5_BUFCOUNT=32 M3_GEM5_CRYPTO_ENGINES=1 &
    $here/runreplay.sh eng-64-dir2    $t $n ironbus 500 M3_GEM5_XBAR_WIDTH=32 M3_GEM5_INFLIGHT=32 M3_GEM5_BUFCOUNT=32 M3_GEM5_CRYPTO_ENGINES=2 &
done
wait
S=${REPLAY_OUT:-/scratch/harshanavkis/ironbus/tools/replay-runs}
for t in "${traces[@]}"; do
    for c in 32-native 32-shared1 32-dir1 32-dir2 64-native 64-dir1 64-dir2; do
        f=$(ls $S/out/eng-$c/$t-*-500.log 2>/dev/null | head -1)
        printf "%-14s %-14s %s\n" "$t" "$c" "$(grep -o 'total: [0-9]* cycles' "$f" 2>/dev/null || echo FAILED)"
    done
done
