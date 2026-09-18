#!/usr/bin/env bash
# usage: runreplay.sh <tag> <trace> <ranks> <native|ironbus|host> <int_lat> [extra env...]
# REPLAY_INSTANCES=k runs k independent instances at the same time (one coordinator each)
tag=$1; trace=$2; ranks=$3; mode=$4; lat=$5; shift 5
k=${REPLAY_INSTANCES:-1}
S=${REPLAY_OUT:-/scratch/harshanavkis/ironbus/tools/replay-runs}
cd /scratch/harshanavkis/ironbus/M3
export M3_BUILD=release M3_TARGET=gem5 M3_ISA=riscv LD_LIBRARY_PATH=build/cross-riscv/lib/ M3_FS=bench.img
export M3_GEM5_CPUFREQ=2GHz M3_GEM5_MEMFREQ=2GHz M3_PAR_PIPE=1 M3_RNG_LATENCY=0 M3_SIGN_LATENCY=0 M3_SIGN_VER_LATENCY=0
export M3_GEM5_INFLIGHT=16 M3_GEM5_BUFCOUNT=16 M3_GEM5_CRYPTO_ENGINES=1 M3_GEM5_CRYPTO_WIRE=32B
export M3_INT_TRA_LATENCY=$lat
# no TCU trace (gem5.log) by default: it only costs wall time
export M3_GEM5_DBG=${M3_GEM5_DBG:-TcuCredits}
if [ "$mode" = native ]; then export M3_ENCR_LATENCY=0; else export M3_ENCR_LATENCY=15; fi
members=$ranks; [ "$mode" = host ] && members=$((ranks+1))
export M3_CORES=$((k*members+k+6)) M3_GEM5_SPM=$((k*members+1))
for kv in "$@"; do export "$kv"; done
name=$trace-$ranks-$mode-$lat; [ "$k" -gt 1 ] && name=$name-k$k
export M3_OUT=$S/out/$tag/$name
mkdir -p $M3_OUT
xml=$M3_OUT/boot.gen.xml
python3 src/tools/gen-replay-boot.py $trace $ranks $mode $xml $k > /dev/null
./b -n run $xml > $S/out/$tag/$name.log 2>&1
echo "exit=$?" >> $S/out/$tag/$name.log
