# Chakra Trace Replay on IronBus/gem5 — Implementation & Run Plan

Goal: replay Chakra execution traces (ETs) on our gem5/M3 prototype in three modes — **native M3** (no crypto), **IronBus** (AIU-encrypted direct P2P), **host-centric** (every transfer bounced through a host tile with per-link encryption) — in two experiment parts:

- **Part A — Scalability (R1.1, R3.1): communication patterns only.** All-reduce, all-gather, reduce-scatter, all-to-all and a PP send/recv chain for N = 2, 4, 8, 16 accelerator tiles, no compute. Isolates per-hop cost vs. N, HAL setup time and endpoint/key growth. Plus a concurrency sub-run: k = 1, 2, 4 independent instances sharing one HAL (the "tenants" half of R1.1).
- **Part B — End-to-end applications (R1.3, R3.4): full traces at a fixed layout** (N = 4 and 8): LLM prefill / decode / training (STAGE), Transformer / DLRM / ResNet-50 training (ASTRA-sim 1.0 traces), next to the existing four apps.

Deliverables for the paper (§7): a workload table; Fig. A "scaling": per-collective time vs. N for native/IronBus/host-centric (+ HAL setup time vs. N, concurrency); Fig. B "applications": overhead vs. native and speedup vs. host-centric, on-chip and off-chip; Fig. "scale-factor convergence" (justifies replaying scaled-down traffic).

Everything below reuses what exists: `M3_ENCR_LATENCY=0/15` already switches native↔IronBus, `M3_INT_TRA_LATENCY` switches on-chip↔off-chip, `M3_PAR_PIPE` selects the pipelined AES engine, `M3_CORES` sets the tile count, and `facever`/`distinf` already show the app pattern (busy-wait compute + `MemGate` DMA + `SendGate` notify + named gates in the boot XML).

---

## 0. Pipeline

```
 trace sources ──► chakra2m3 (python) ──► per-rank op files (.m3t) ──► src/fs/bench/traces/ (bench.img)
   ASTRA-sim 1.0 text │                                                        │
   STAGE (llama/gpt)  │  lower collectives → ring P2P                          ▼
   [real PyTorch ET]  │  scale bytes/compute, topo-sort per rank     tracereplay (Rust, m3 lib) on gem5
                      │                                              modes: native | ironbus | hostcentric
                      ▼                                                        │
                 manifest.json (N, bytes, expected ring volume)                ▼
                                                            run matrix (scripts) → parse `total:` → plots
```

## 0b. Calibrate the off-chip link first (day 1–2) — prerequisite for both parts

Findings from the paper's own logs (`M3/benchmarks/exp-results`, `constants.py`): runs used 2 GHz tile/sys clocks; `--int-transfer-latency` is in TCU cycles at the tile clock, so the plotted "Off-chip" (`-500`) is **250 ns**, not the 500 ns the text claims (`-1000` logs exist). Native DMA bandwidth with 4 KiB commands: on-chip 7.1/14.8 GB/s (read/write), 250 ns 2.6/3.5 GB/s, 500 ns 1.6/1.9 GB/s — 10–25× below PCIe Gen4/5 (≈25/50 GB/s effective). Cause: bandwidth-delay product (synchronous 4 KiB commands, `buf_count = 4` × 2 KiB in flight), not the crossbar (16 B/cycle @ 2 GHz = 32 GB/s). Why it matters: the AIU AES-GCM pipe is 16 B/cycle = 32 GB/s; on a slow link the crypto never bottlenecks, so the security tax is understated, and R1.2's multi-engine question is moot.

**Measured (sweep of 2026-09-17, `M3_GEM5_BUFCOUNT`/`M3_GEM5_PKTSIZE` overrides added to `tcu_fs.py`):** `buf_count` 4 → 16 changes nothing (7.10/14.78 GB/s on-chip, 1.58/1.90 at L=1000, identical to the cycle) because READ/WRITE commands are stop-and-wait per ≤2 KiB packet (`cmds.cc:316`); 4 KiB packets give 2.63/3.60 GB/s at L=1000 (linear in packet size). Crypto model: (a) memory tiles are never attested (`tilemng.rs:117`, loop commented out) so DRAM read responses are charged **zero** crypto cost — reads only pay the 1-block request header (+34 cycles/packet); (b) the charged per-packet delay (157 cycles for 2 KiB, pp=1) largely overlaps the crossbar payload occupancy on-chip (+46 observed), but appears in full off-chip (+157/+284); (c) no engine occupancy across packets and no IV/MAC bytes on the wire.

Decision needed: **(A)** TCU change — pipelined multi-packet DMA commands; **keep the per-packet crypto formula `L + (B−1) + L`** (it matches the store-and-forward AIU described in §4.2) but add what pipelining makes necessary: per-AIU engine occupancy (packets in flight queue on one AES-GCM pipe at 1 block/cycle = 32 GB/s at 2 GHz; `k` pipes for the R1.2 sweep — one suffices for Gen4-class 25 GB/s, Gen5-class 50 GB/s needs two), 32 B IV+MAC on the wire (already claimed in the text), and attest memory tiles (fixes the zero-cost DRAM reads); then calibrate to Gen4-class (250 ns, ≥ 25 GB/s; crossbar 16 B/cycle = 32 GB/s peak, ranks on SPM tiles since cached tiles absorb reads at ~4 B/cycle) and re-run existing benchmarks (~2–3 days + minutes per run). **(B)** keep stop-and-wait with 4 KiB packets and the ~10–20× ratio scaling (no code change; crypto never bottlenecks, so R1.2's multi-engine question gets only a latency-side answer). Recommendation: A.

Do: (1) **keep L = 500 cycles (250 ns) — the latency behind the paper's plots — as the PCIe/off-chip setting** and fix the text ("500 ns" → "250 ns", state the unit in Methodology); (2) make that config PCIe-class in bandwidth: at 250 ns one-way, Gen4-class 25 GB/s needs only ~16 KB in flight ≈ 8 packets of 2 KiB per command (path A below), targeting ≥ 25 GB/s (Gen5-class 50 GB/s would need a 32 B crossbar — optional); (3) re-run the existing micro/app benchmarks under it (minutes each) so the whole paper uses one platform; (4) use the measured `BW_sim` of that config in §3 — expect a compute-scaling factor of ~1–2 instead of ~20.

Reference GPU for the roofline/link (ASTRA-sim ships only NVLink/ICI systems and H100 numbers): primary **A100 PCIe** (312 TFLOPS dense FP16, ~1.9 TB/s, PCIe Gen4 x16 = 32 GB/s per direction) or **L40S** (~362 TFLOPS dense FP16, 864 GB/s, Gen4 x16, no NVLink); second point **H100 PCIe** (~756 TFLOPS dense FP16, 2 TB/s, Gen5 x16 = 64 GB/s). Verify datasheet values before they go in the paper.

## 1. Environment (day 1)

- M3/gem5: existing `nix-shell` in `M3/` (already builds and runs `boot/hello.xml`).
- Chakra tooling: separate shell `nix-shell -p python3 python3Packages.protobuf protobuf python3Packages.pandas` in `astra-sim/extern/graph_frontend/chakra`, then `pip install --user -e .` (generates `et_def_pb2.py`; pins `protobuf==5.*`). Provides `chakra_converter` and the `protolib` reader we import in `chakra2m3`.
- STAGE: `git clone https://github.com/astra-sim/symbolic_tensor_graph` into `ironbus/stage`, `pip install -r requirments.txt` (numpy, sympy, protobuf, pandas). Outputs Chakra v0.0.4 ETs (`--chakra_schema_version v0.0.4`, same schema as our vendored Chakra) or JSON (`--chakra_schema_version json`, easier to parse if protobuf versions fight).

## 2. Trace acquisition (days 1–2)

**Part A — communication patterns** — ASTRA-sim ships them: `astra-sim/examples/workload/microbenchmarks/{all_reduce,all_gather,reduce_scatter,all_to_all}/{4,8,16}npus_1MB/*.et`, and `generator_scripts/<coll>.py --npus-count N --coll-size BYTES` for any N/size (N=2, and 256 KB–4 MB sizes). The PP chain is a 2-line hand-written ET (SEND/RECV between consecutive ranks) or just the `chakra2m3 --pattern chain` shortcut.

**Part B — training (real measured per-layer compute, real collective sizes)** — ASTRA-sim 1.0 text workloads (verified downloadable):
```
B=https://raw.githubusercontent.com/astra-sim/astra-sim/ASTRA-sim-1.0/inputs/workload
for w in Transformer_HybridParallel DLRM_HybridParallel Resnet50_DataParallel microAllReduce microAllToAll; do curl -sLO $B/$w.txt; done
chakra_converter Text --input Transformer_HybridParallel.txt --output tf_hp --num-npus 4 --num-passes 1
```
Units: comm sizes in bytes; compute times in cycles (per the ASTRA-sim 1.0 README). `Transformer_HybridParallel` = TP group of 4, 57 layers, 8 MB fwd all-reduce / 32 MB weight-grad all-reduce per layer; `DLRM` = all-to-all embedding + all-reduce MLP; `Resnet50` = pure DP all-reduce.

**Part B — LLM (synthetic, standard tool)** — STAGE, Llama-7B-class dims:
```
python main.py --output_dir gen/llama7b_tp4 --output_name llama7b_tp4.%d.et --comm_group_file cg.json \
  --model_type llama --dmodel 4096 --dff 11008 --head 32 --kvhead 32 --num_stacks 32 --dvocal 32000 \
  --batch 1 --seq 512 --tp 4 --pp 1 --dp 1 --weight_sharded 0
```
Layouts: TP=2/4/8 (all-reduce per layer), PP=2/4 (activation send/recv between stages), TP2×PP2, DP=2/4 with weight sharding for training. STAGE emits training (fwd+bwd) graphs; **inference prefill** = forward-pass subgraph (filter nodes by name prefix in `chakra2m3`; the forward pass is a prefix of the graph, so dependencies stay closed); **decode** = same forward graph generated with `--seq 1` (per-token step; KV-cache reads are local, so only the TP all-reduces/PP activations cross the bus). Report prefill latency (seq=512) and per-token latency (seq=1). STAGE compute nodes carry FLOPs/tensor sizes, not durations → convert with the same roofline ASTRA-sim uses (`peak-perf`, `local-mem-bw` from `inputs/system/analytical/hgx_h100_8gpu.json`: 700 TFLOPS, 3350 GB/s) → seconds → cycles at our simulated clock.

**Optional (only if a multi-GPU box is available)**: PyTorch ET + Kineto → `chakra_trace_link` → `chakra_converter PyTorch`. Not on the critical path.

## 3. `chakra2m3` converter (days 2–5) — `M3/src/tools/chakra2m3/`

Input: `<prefix>.<rank>.et` (or STAGE JSON). Output: `<name>.<rank>.m3t` per rank + `<name>.json` manifest.

**Op format (little-endian, fixed 24 B records; engine stays trivial):**
```
op  u8   0=COMP 1=SEND 2=RECV 3=BARRIER 4=END
peer u8   rank (SEND dst / RECV src)
tag  u16  matches a SEND to its RECV (step id), lets a rank hold several in-flight peers
bytes u64 payload bytes (SEND/RECV); cycles for COMP
id   u64  ET node id (for debugging/trace-back)
```

**What is taken from the trace (per rank) and what is not.** Taken: (1) compute durations, (2) communication ops with peer and size — P2P `SEND/RECV` as-is, collectives lowered over their `comm_group`/`pg_name` (TP group ≠ world), (3) ordering from `ctrl/data_deps`, (4) node ids for trace-back. Not taken: tensor contents (dummy payload; AES-GCM cost is content-independent), `MEM_LOAD/STORE` nodes (local HBM traffic, inside the roofline time, never on the bus), `start_time` (dependency-driven replay), op names. Link latency/bandwidth, AIU crypto cost, HAL setup and host-relay hops are produced by gem5, not fed in.

**Compute time → cycles (state once in §7 Methodology):**
- STAGE nodes carry `num_ops` (FLOPs) and `tensor_size` (bytes). Reference time via ASTRA-sim's own roofline (`system/Roofline.cc`): `OI = num_ops/tensor_size`, `perf = min(peak, mem_bw·OI)`, `t_real = num_ops/perf`, with `peak`/`mem_bw` of the reference GPU (§0b; ASTRA-sim's own H100 file uses 700 TFLOPS / 3350 GB/s).
- Text traces: `t_real` = their cycles at the reference clock; real PyTorch ETs: `duration_micros`.
- Ratio-preserving scaling to our clock: `cycles = t_real · (BW_ref / BW_sim) · f_sim`, with `BW_ref` = the reference GPU's PCIe link (32 GB/s Gen4 for A100 PCIe/L40S, 64 GB/s Gen5 for H100 PCIe), `BW_sim` = measured native-M3 DMA throughput of the calibrated config (§0b; on-chip and off-chip separately), `f_sim = 2 GHz`. Rationale: encryption overhead is governed by the compute:communication ratio; naively using H100 compute times against a few-GB/s simulated link would make every workload communication-bound. One knob (`--ref-bw`), plus a `--comp-scale` ×0.5/×2 sensitivity sweep on one workload.

**Lowering rules:**
- `COMP_NODE`: `t_real` (rule above) × scale → `COMP cycles`.
- `COMM_SEND/RECV_NODE`: `SEND(dst, bytes)` / `RECV(src, bytes)` (PP activations).
- `COMM_COLL_NODE` over the node's comm group (from STAGE `comm_group.json`; whole world for text traces):
  - all-reduce S: ring reduce-scatter + ring all-gather = 2(N−1) steps of S/N to (r+1)%N from (r−1)%N.
  - all-gather / reduce-scatter: (N−1) ring steps of S/N.
  - all-to-all: (N−1) pairwise steps, rank r sends S/N to (r+k)%N and receives from (r−k)%N.
  - Every ring step is emitted as `SEND` then `RECV` on the same tag; correctness (no deadlock) argued in §4.
- Dependencies: ASTRA-sim issues a node when all `ctrl/data_deps` are done. We emit each rank's ops in a deterministic topological order (Kahn, ties by node id) so the per-rank program is a valid sequential schedule. Collectives are synchronizing across ranks anyway; independent compute that ASTRA-sim would overlap with communication is *not* overlapped by a sequential rank (conservative for us — say so).
- Scaling: `--bytes-scale 1/s` on every comm size, `--comp-scale 1/s` on every compute duration (keeps compute:comm ratio), `--layers k` to replay the first k layers (window). Manifest records raw and scaled totals.

**Tests (`pytest`):** ring volume = 2(N−1)/N·S per rank for all-reduce; every SEND has a matching RECV with the same tag/bytes on the peer; topological order respects deps; text-converter microAllReduce (1 MB, 4 NPUs) round-trips to the expected 6 steps of 256 KB.

## 4. `tracereplay` app (days 4–9) — `M3/src/apps/bench/tracereplay/` (Rust, `m3` lib)

Register like `facever`: dir with `build.py` (`env.m3_rust_exe(gen, out='tracereplay')`), `Cargo.toml`, add to `src/apps/bench/build.py` `dirs` and to the root `Cargo.toml` members. Trace files go under `src/fs/bench/traces/` so they land in `bench.img` (64K blocks = 256 MB; enough for tens of MB of traces).

**Roles (one binary, selected by argv):**
- `coordinator <trace-name> <N> <mode>`: runs on the app tile; reads the manifest; allocates `N` accelerator tiles (`Tile::get("core")` like `facever`) plus one for the host relay in `hostcentric` mode; creates per-rank inbound buffers (`MemGate::new(bufsz, RW)`, sized to the largest single transfer, default 2 MiB) and notify gates (`RecvGate::new(order 11, msg order 6)` → 32 slots of 64 B); delegates to rank r: its own rgate, one `SendGate` (credits 1) + one `MemGate` (W) per peer it talks to (from the manifest's peer list, so a TP=4 rank doesn't get 15 gates); passes selectors through `data_sink()` exactly like `facever`. Starts all ranks, sends "go", waits, prints `total: <cycles>` plus per-rank `comm: / compute: / wait:` lines.
- `rank r`: loads `<name>.<r>.m3t` from m3fs into memory (once, before "go"), then executes the op list:
  - `COMP c`: busy-wait `c` cycles (`CycleInstant`, as in `facever`).
  - `SEND(dst, bytes, tag)`: `mgate[dst].write(buf[..bytes], 0)` (DMA into dst's inbound buffer; > 2 KiB is chunked by the TCU/AIU) then `sgate[dst].send(notify{tag, bytes})`.
  - `RECV(src, bytes, tag)`: `recv_msg(rgate)`, check `{src, tag, bytes}`, ack (returns the sender's credit). Out-of-order notifies from other peers are parked in a small pending list.
  - `BARRIER`: implemented in the converter as an all-gather of 0 bytes (no extra engine code).
- `hostrelay`: loop `recv notify{src,dst,tag,bytes}` → `mgate[dst].write(staging[src][..bytes], 0)` → `sgate[dst].send(notify)`. In `hostcentric` mode ranks' `SEND` targets the relay's staging buffer and its rgate instead of the peer; both hops are AIU-encrypted channels, i.e. exactly the 2N−2 vs N−1 transfer model of the existing host-centric benchmark, now at real data volumes.

**No-deadlock argument:** credits=1 per (src,dst) pair and one inbound region per pair; a ring step k on every rank is `SEND k` then `RECV k`; `SEND k+1` needs the credit returned by the successor's `RECV k` ack, which every rank performs before its own `SEND k+1`; the DMA for step k+1 therefore cannot overwrite unread data. For all-to-all, the pairwise schedule (r+k, r−k) has the same property per pair.

**Modes = env vars, no code paths:** native `M3_ENCR_LATENCY=0`; IronBus `M3_ENCR_LATENCY=15 M3_PAR_PIPE=1` (as in the paper runs; `=0` for the store-and-forward sensitivity); off-chip `M3_INT_TRA_LATENCY=500` (500 cycles = 250 ns at 2 GHz, the paper's plotted setting); clocks `M3_GEM5_CPUFREQ=2GHz M3_GEM5_MEMFREQ=2GHz` as in `benchmarks/constants.py`; `hostcentric` is an argv mode that adds the relay.

**Boot XML:** `src/tools/gen-replay-boot.py <trace> <N> <mode>` emits `boot/replay-<trace>-<N>-<mode>.xml` from the `bench-facever.xml` skeleton with `<tiles type="core" count="N+1(+1)"/>` and `<mount fs="m3fs" path="/"/>`. Run with `M3_CORES=N+8` (kernel, root, m3fs, pager, coordinator, relay, slack).

## 5. Sanity checks before the matrix (day 9–10)

1. `microAllReduce` (1 MB, N=4) native: per-rank bytes moved = 1.5 MB; time ≈ 6 × (256 KB / measured DMA throughput from §7 read/write plots). Compare against the expected ring cost.
2. `microAllReduce` IronBus vs native overhead should sit in the range of the existing IPC/DMA microbenchmarks (5–23 %); off-chip should be smaller than on-chip.
3. N=2 PP send/recv with 512 B messages in hostcentric mode should reproduce the ≈2× of Fig. "host-centric comparison".
4. Scale-factor sweep on `Transformer_HybridParallel` (s = 1/256, 1/64, 1/16, 1 layer window): overhead ratios should converge — this becomes the figure that justifies scaled replay.

## 6. Run matrix (days 10–14; runs are independent → run 20–30 gem5 processes in parallel, 48 cores / 2 TB)

**Part A — scalability (communication patterns)**

| Dimension | Values |
|---|---|
| Pattern | all-reduce, all-gather, reduce-scatter, all-to-all, PP chain |
| N | 2, 4, 8, 16 tiles |
| Size | 1 MB per collective (plus 256 KB and 4 MB for all-reduce only) |
| Mode | native, ironbus, hostcentric |
| Interconnect | on-chip, off-chip |
| Concurrency | k = 1, 2, 4 instances of all-reduce N=4 sharing one HAL (ironbus only) |

≈ 5×4×3×2 + extras ≈ 140 short runs (each moves ≤ 2·1 MB per rank; minutes each).

**Part B — end-to-end applications**

| Dimension | Values |
|---|---|
| Workload | llama7b prefill (seq 512), llama7b decode (seq 1), llama7b train (DP), Transformer_HybridParallel, DLRM_HybridParallel, Resnet50_DataParallel |
| Layout / N | N = 4 (TP4 / DP4) and N = 8 (TP4×PP2 / DP8) |
| Mode | native, ironbus, hostcentric (+ ironbus with `M3_PAR_PIPE=1` for one workload) |
| Interconnect | on-chip, off-chip |
| Scale | 1/16 by default; 1/64 and 1/4 (or a 1-layer window) on Transformer_HybridParallel for the convergence figure |

≈ 6×2×3×2 + extras ≈ 80 runs.

Runner: `src/tools/run-replay.sh` loops over the matrix, sets `M3_OUT=run/<cfg>` per run, and `xargs -P 24`. Each run's `log.txt` holds `total:` and per-rank lines. Budget: the existing `facever` run (2 tiles, ~0.3 MB moved) takes **2:02 wall-clock**, of which ~215 ms of the 222 ms simulated is boot + attestation of 22 tiles (a fixed cost); the app itself is 1.45 M cycles. So per-run cost is dominated by boot; expect ~5–30 min per Part-B run at 1/16 scale and the whole matrix in well under a day of machine time.

## 7. Analysis (days 14–16) — `src/tools/parse-replay.py`

Parse `total:` and per-rank `comm/compute/wait` → CSV → plots (matplotlib, same style as `plots/app-bench`):
1. Fig. A (scaling): per-collective completion time vs. N for native/IronBus/host-centric (lines, one panel per pattern, on-chip + off-chip); a second panel with HAL setup time (attestation + channel creation) vs. N and the k-instances concurrency result.
2. Fig. B (applications): overhead vs. native per workload (bar, on-chip/off-chip) with the existing four apps on the same axis, and speedup vs. host-centric (bar).
3. Scale-factor convergence (line) — goes in the paper or the response letter.
4. Workload table: name, source, layout, N, bytes moved per rank (raw and replayed), compute:comm ratio.

## 8. Risks

- **gem5 wall time**: mitigated by the A/B split (N=16 only in Part A with 1 MB collectives), scaled traffic and layer windows in Part B, and parallel runs.
- **Endpoints/keystore**: a rank needs 1 rgate + 2 per peer; TP16 all-to-all = 31 EPs ≪ 192 per AIU. Fine.
- **Message size**: notifies are 64 B; all payload goes via DMA (chunked by the AIU, 2 KiB buffer) — matches the existing read/write benchmarks.
- **Compute-time units**: text traces are "cycles" of an unspecified NPU clock; STAGE gives FLOPs. We fix one roofline and one reference link bandwidth (§3), preserve the compute:comm ratio, and show sensitivity with `--comp-scale` ×0.5/×2 and `--ref-bw` (PCIe vs. NVLink) on one workload.
- **Sequential-rank conservatism**: no compute/comm overlap within a rank → our absolute times are pessimistic for all three modes equally; overhead ratios are what we report.
- **Reviewer pushback "still simulated"**: pre-empt with the convergence figure, the calibrated AIU latencies (OpenTitan), and the trace provenance table.

## 9. Timeline (≈ 3 weeks)

- Days 1–2: environment, download/generate traces, inspect ET stats (nodes, bytes per collective).
- Days 2–5: `chakra2m3` + tests; manifests for all workloads.
- Days 4–9: `tracereplay` (coordinator, rank, relay), boot generator, first N=2/4 runs.
- Days 9–10: sanity checks (§5), fix scaling factors.
- Days 10–14: Part A matrix first (fast, and its N=2/4 all-reduce doubles as the sanity check), then Part B; meanwhile write §7 methodology/workload table text.
- Days 14–16: parse, plot, write results paragraphs; decide what goes in the paper vs. the response letter.

---

# Status (2026-09-18, updated)

**Step 1 — tile config: done.** `config/default.py` takes `M3_GEM5_SPM` (number of SPM core tiles, default 4). `M3_CORES=24 M3_GEM5_SPM=18` gives tiles 1–5 cached, 6–23 SPM; verified with `hello.xml` and `bench-p2p-spm.xml` (29 GiB/s, as with the default config). Commit `b870486fa`.

**Step 2 — tooling and traces: done** (all outside the repos, under `/scratch/harshanavkis/ironbus/tools/`):
- `venv/` (Python 3.13; protobuf 7.36, numpy, sympy, pandas, tqdm, networkx, pydot, graphviz). Use inside `nix-shell -p python3 gcc zlib` with `source tools/env.sh` (sets `LD_LIBRARY_PATH` for the binary wheels).
- Chakra: protobuf bindings generated with nix `protoc` (`schema/protobuf/et_def_pb2.py` in the vendored copy under `astra-sim/extern/graph_frontend/chakra`); exposed as package `chakra` via `tools/chakra-pkg` (symlinks + `chakra.pth`) — avoids the grpc-based setup.py and the HolisticTraceAnalysis dependency. `venv/bin/chakra_converter` wraps `chakra.src.converter.converter`.
- STAGE: `tools/stage` (clone of astra-sim/symbolic_tensor_graph, run from that directory).
- Traces (`tools/traces/`):
  - `collectives/<coll>_<N>/<coll>.<rank>.et` for all_reduce, all_gather, reduce_scatter, all_to_all × N = 2, 4, 8, 16, 1 MiB each (ASTRA-sim generator scripts; need `PYTHONPATH=astra-sim`).
  - `astra1/*.txt`: ASTRA-sim 1.0 workloads (Transformer_HybridParallel, DLRM_HybridParallel, Resnet50_DataParallel, MLP_HybridParallel_Data_Model, microAllReduce/AllToAll) plus `.et` conversions for DLRM/ResNet/microAllReduce at 4 NPUs. `chakra_converter Text` does **not** support `HYBRID_TRANSFORMER` and has no comm groups (every collective spans all NPUs), so `chakra2m3` reads the text format directly for these workloads (TP groups of 4 for fwd/ig comms, DP groups across for wg comms; DLRM: all-to-all embedding over all ranks).
  - `stage/llama7b_<layout>/`: Llama-7B dims (dmodel 4096, dff 11008, 32 heads, vocab 32000), **4 layers** (`--num_stacks 4`, replay window), batch 1, seq 512 (prefill) or seq 1 (decode); layouts tp4, tp2pp2, dp4 (N=4), tp4pp2, dp8 (N=8). STAGE emits Chakra v0.0.4 protobuf ETs regardless of the `json` option; comm groups in `<name>.json` (`pg_name` → ranks). STAGE uses TP+SP by default, so TP collectives appear as all-gather/reduce-scatter pairs. Rank 0 of `llama7b_tp4_s512` (4 layers, training graph): 96 nodes, 22 collectives, 272 MiB moved, 222 GFLOP.
- Compute time: text traces carry cycles in `duration_micros`-free COMP nodes (converter puts the cycle count in `duration_micros`); STAGE COMP nodes carry `num_ops`/`tensor_size` (roofline in `chakra2m3`).

Next: step 3, `chakra2m3` (`M3/src/tools/chakra2m3/`).

**Steps 3–4 (2026-09-18): `chakra2m3` and `tracereplay` implemented** (commits `8e63739a1`, `e83549457`). All 34 workloads convert and verify; native/ironbus replays run. Run one configuration with `src/tools/replay/runreplay.sh <tag> <trace> <ranks> <mode> <lat> [ENV=val]` (outputs under `tools/replay-runs/out/<tag>/`; `M3_GEM5_DBG` defaults to `TcuCredits`, i.e., no TCU trace — a run takes ~3 min instead of ~15).

**Host mode and measurement fixes (2026-09-18, later):**
- *Selector collision* (`ALLOC_EP failed: Selector already in use`): channels delegated to running members went in with the coordinator's selector numbers, which collided with selectors the member allocates itself (EPs on first use). Now delegated with `delegate_to` into a reserved range (`CHAN_SEL` = 2000+) that the Go message carries.
- *Relay deadlock*: the relay had one notification gate per destination with one credit, shared by all sources; a notification parked at the destination (waiting for a later RECV) blocked the one the destination was waiting for. Now the relay→rank gates have `ranks` credits and the relay keeps one notification in flight per (source, destination) pair — the verifier's model — using the destination's reply (the `done` reply echoes the message) on one reply gate per destination (≤ 32 slots per receive buffer is a TCU limit: 32-bit occupied/unread masks). The relay credits the source as soon as the data is copied (a host with buffers) and forwards eagerly; sequential, one copy at a time. libm3: `RGateArgs::replies(false)` creates a receive gate without reply endpoints (a reply-only gate costs 1 EP instead of slots+1).
- *Lost transfers before Go*: the coordinator sent Go to the relay last, so ranks' first transfers reached the relay while it was still waiting for Go and were dropped. Now a READY/START barrier: members activate all channels (`MemGate/SendGate::activate`, the latter made public), report READY, the coordinator releases the ranks with START; ranks leave only after a final STOP (all credits back before teardown).
- *Channel setup was inside the measurement*: gates were activated lazily on first use, i.e., ~6 kernel syscalls per rank, serialized at the kernel, inside the timed region — ~130k of the 265k cycles of all_reduce N=4 native. **All numbers before this fix (264k/355k/270k, Llama +44 %) are void.**

**Results with the fixed replay (all_reduce N=4, 1 MiB, off-chip, cycles):** native 128.5k (24.6 GB/s per rank, 84 % of the link), IronBus 1 engine 225.9k (+76 %), host-centric 809.7k (6.3× native; relay forwards 24 transfers sequentially). IronBus 2 engines 133.3k (+3.7 %): one engine per direction is what a full-duplex collective needs (R1.2).

Open: all_to_all N=8 was ~10× slower than expected in both modes (native 1.69M cycles for 0.9 MiB per rank) while all_reduce N=8 was fine — re-check with the fixed replay (the lazy activations may explain part of it), then pairwise schedule vs. crossbar/TCU behaviour.
