#!/usr/bin/env bash
# Converts all traces under tools/traces into replay programs under tools/m3t.
# Run inside: nix-shell -p python3 gcc zlib --run 'source $TOOLS/env.sh && convert-all.sh'
# (TOOLS defaults to /scratch/harshanavkis/ironbus/tools: venv, traces; output in $TOOLS/m3t)
set -e
T=${TOOLS:-/scratch/harshanavkis/ironbus/tools}
C=/scratch/harshanavkis/ironbus/M3/src/tools/chakra2m3/chakra2m3.py
OUT=$T/m3t; rm -rf $OUT; mkdir -p $OUT/collectives $OUT/apps
SCALE=${SCALE:-0.0625}
echo "== Part A: collectives (1 MiB, full scale) =="
for coll in all_reduce all_gather reduce_scatter all_to_all; do for n in 2 4 8 16; do
  python $C --et $T/traces/collectives/${coll}_${n}/$coll --name ${coll}_${n} --out $OUT/collectives --bytes-scale 1
done; done
for n in 2 4 8 16; do python $C --pattern chain --num-ranks $n --size 1048576 --iterations 4 --name chain_$n --out $OUT/collectives; done
echo "== Part B: applications (inference unscaled; training/text scaled by $SCALE) =="
# LLM inference: forward pass only (prefill: seq 512, decode: seq 1)
for l in llama7b_tp4_s512 llama7b_tp4_s1 llama7b_tp2pp2_s512 llama7b_tp4pp2_s512; do
  python $C --et $T/traces/stage/$l/$l --name ${l/llama7b_/llama7b_inf_} --out $OUT/apps --bytes-scale 1 --forward-only --drop-ops w
done
# LLM training (batch 8, seq 2048)
for l in llama7b_train_tp4 llama7b_train_dp4 llama7b_train_dp8 llama7b_train_tp4pp2; do
  python $C --et $T/traces/stage/$l/$l --name $l --out $OUT/apps --bytes-scale $SCALE
done
# ASTRA-sim 1.0 workloads (measured per-layer compute)
for n in 4 8; do
  python $C --text $T/traces/astra1/Transformer_HybridParallel.txt --num-ranks $n --tp 4 --name transformer_$n --out $OUT/apps --bytes-scale $SCALE
  python $C --text $T/traces/astra1/DLRM_HybridParallel.txt --num-ranks $n --name dlrm_$n --out $OUT/apps --bytes-scale $SCALE
  python $C --text $T/traces/astra1/Resnet50_DataParallel.txt --num-ranks $n --name resnet50_$n --out $OUT/apps --bytes-scale $SCALE
done
