# IronBus evaluation — platform, methodology and results (TACO revision)

Working notes for §7 of the revised paper. §2–§7 were measured on 2026-09-17/18 with
`python3 reproduce.py` (raw numbers in `snapshot.json`; `snapshot-eng1.json` = the same before the
per-direction engine model, `benchmarks/snapshot-stopwait.json` = the original submission), logs
in `benchmarks/exp-results/`, figures alongside. §8–§9 (trace replay) were measured on 2026-09-18
with the `src/tools/replay/sweep-*.sh` scripts (raw numbers in `tools/replay-runs/replay.csv`,
outside the repo). Code: M3 branch `sec-acc`, gem5 submodule branch `gem5-sec-acc` (see `git log`
for the exact commits; the engine model changed on 2026-09-18, see §1 and §4).
Units: cycles at 2 GHz; throughput in GiB/s in the figures (the text below uses GB/s where it
compares with link rates: 1 GiB/s = 1.074 GB/s).

## 1. Platform (Methodology text)

- **Simulator**: gem5, RISC-V tiles at 2 GHz (`DerivO3CPU` for the user tiles), 23 tiles
  (`config/default.py`: 11 cached + 4 scratchpad (SPM) RISC-V tiles, 2 ROT13 and 1 KECACC
  accelerator tiles, serial/NIC, one memory tile); `config/accels.py` for the accelerator
  applications (16 cached tiles, 4 indirect-copy, 4 copy and 1 ROT13 accelerator tiles).
- **Memory tile**: one DDR4-2400 x64 channel (19.2 GB/s peak), i.e., CU↔DRAM traffic is bounded
  at ~10.7 GB/s reads / 14.8 GB/s writes for every configuration; on cached tiles additionally by
  the cache-fill path (~7 GB/s reads).
- **Interconnect**: crossbar with 16 B/cycle at 2 GHz = 32 GB/s per link (Gen4-x16 raw,
  between Gen4 and Gen5 effective). **On-chip**: no additional latency. **Off-chip**: 500 cycles
  = 250 ns one-way latency per packet in each direction (the value behind all plots; the original
  text said 500 ns in error). A 32 B/cycle crossbar (64 GB/s, Gen5-x16 raw) is used as a
  sensitivity point only.
- **TCU/AIU**: NoC packets of at most 2 KiB; DMA commands keep up to **16 packets in flight**
  (`max_inflight_packets`); messages are single packets. Tiles with virtual memory issue
  page-granular (4 KiB) DMA commands, SPM tiles hand whole transfers to the TCU.
- **AIU crypto model**: per packet, `L + (B−1) + L` cycles with `L = 15` (AES-GCM pipeline
  latency from OpenTitan) and `B` = 16-byte blocks: the sender encrypts the whole packet
  (store-and-forward), the receiver decrypts in parallel with reception and releases the packet
  after tag verification (pp=1, "parallel pipelined"). The AIU has **one AES-GCM engine per
  link direction** (as in PCIe IDE implementations: a Tx engine encrypts outgoing packets, an Rx
  engine decrypts incoming ones; each does one 16 B block per cycle = 32 GB/s at 2 GHz, and
  in-flight packets of the same direction queue for it; `M3_GEM5_CRYPTO_ENGINES=k` = k per
  direction, `M3_GEM5_CRYPTO_SHARED=1` = the naive single pool for both directions, §4 E2).
  Every encrypted packet carries **32 B of IV+MAC** on the wire. Memory tiles have AIUs too (attested at boot, after the user tiles), so DRAM read
  responses are encrypted at the memory side and decrypted at the requester.
- **Attestation cost**: 10.2 M cycles (5.1 ms) per tile, one-time at boot; 23 tiles → ~117 ms.
- **Baselines**: *M3* (native) = the same platform with encryption disabled (unprotected NoC);
  *host-centric* = every device-to-device transfer bounced through a host tile with per-link
  encryption (2 hops instead of 1; §6 `bremote_ipc` chain tests, §8–§9 the replay's relay). In the
  replay the host is one tile of the same kind as the accelerators (scratchpad, 32 GB/s link) that
  receives each transfer into a per-source buffer and forwards it to the destination, one copy at a
  time, crediting the source as soon as its data is copied and keeping one notification in flight
  per (source, destination) pair; it has no software cost beyond the copy and no IOMMU/root-complex
  cost, i.e., it is a *favourable* host model.
- Environment (`benchmarks/constants.py`): `M3_GEM5_CPUFREQ=2GHz M3_GEM5_MEMFREQ=2GHz
  M3_PAR_PIPE=1 M3_ENCR_LATENCY=15 (IronBus) / 0 (M3) M3_INT_TRA_LATENCY=0|500
  M3_GEM5_INFLIGHT=16 M3_GEM5_BUFCOUNT=16 M3_GEM5_CRYPTO_ENGINES=1 (per direction)
  M3_GEM5_CRYPTO_WIRE=32B`.

### Trace replay (§8–§9): methodology

- **What is replayed.** Chakra execution traces are converted per rank into a program of COMP /
  SEND / RECV ops (`src/tools/chakra2m3`, one program per accelerator tile). Taken from the trace:
  compute durations, communication ops with peer and size (point-to-point as is; collectives
  lowered over their communication group: ring reduce-scatter + all-gather for all-reduce, ring
  all-gather / reduce-scatter, pairwise all-to-all), dependency order. **Not taken:** tensor
  contents (dummy payload; AES-GCM cost is content-independent) and `MEM_LOAD/STORE` nodes — an
  accelerator's reads of weights, activations and KV cache from its *own* HBM never cross the
  interconnect, so they are part of the compute time, not of the bus traffic. **Compute time** is
  taken from the trace where it carries cycles (ASTRA-sim 1.0 traces: Transformer, DLRM, ResNet-50)
  and otherwise from a roofline on an A100-class reference (312 TFLOP/s dense FP16, 1935 GB/s
  HBM: `t = ops / min(peak, HBM bandwidth × arithmetic intensity)`; STAGE Llama traces) — so
  decode (seq 1) is memory-bound in its compute phases and prefill compute-bound — and scaled by
  the ratio of the reference link (32 GB/s) to the simulated one so that the compute-to-
  communication ratio of the trace is preserved; the rank busy-waits for it. Traces are replayed
  as a window (Llama-7B: 4 of 32 layers, batch 1, seq 512 prefill / seq 1 decode; training at
  batch 8, seq 2048; ASTRA-sim traces with their traffic scaled down; sizes per rank in
  `trace-replay-plan.md`). A converter-side verifier checks every program set for deadlock freedom
  under the replay's protocol.
- **How it runs.** The coordinator (a cached tile) starts one activity per rank on scratchpad
  accelerator tiles (plus the relay in host mode), loads the programs into the ranks' buffers,
  creates the channels — per (source, destination) pair one memory gate into the destination's
  inbound slot and one notification send gate with one credit — and delegates them; the ranks
  activate all channels, report ready, and are released together. A SEND writes the data into the
  peer's slot with a DMA command (up to 16 packets in flight) and sends a 40 B notification; a
  RECV waits for the matching notification and acknowledges it with a reply, which returns the
  sender's credit — one transfer in flight per pair, transfers larger than 1 MiB are split. In
  host mode the source's channel goes to the relay and the relay's to the destination (see
  Baselines). Modes are environment settings: native `M3_ENCR_LATENCY=0`, IronBus 15,
  on-chip/off-chip `M3_INT_TRA_LATENCY=0|500`.
- **What is measured.** Per rank the cycles from the start signal to its last op (and the split
  into compute / send / receive waits); the reported completion time is the maximum over ranks.
  Channel creation, endpoint activation and program loading are *before* the start signal and are
  reported separately as setup time (§8: "HAL setup"), since they belong to the HAL's one-time
  cost, not to the collective. N is the number of accelerator tiles (ranks); k concurrent
  instances are k independent coordinators with their own tiles sharing one kernel/HAL.
  Run-to-run noise ≈ 1 % (phase alignment of the ranks; gem5 is deterministic otherwise).

### What changed relative to the original submission (for the response letter)

1. Memory-tile AIUs are attested and charged (they were in the runs behind the submitted
   figures, but the tree had lost it; the revision's tree reproduces the submitted numbers to
   ≤ 0.01 % for single-packet DMA and ≤ 1.6 % for the applications when the new features are
   switched off).
2. DMA commands pipeline up to 16 packets (previously stop-and-wait per 2 KiB packet, which
   bounded any DMA to packet/RTT, e.g. 2.6 GB/s off-chip — no real DMA engine works that way).
3. In-flight packets queue for the AIU's AES-GCM engines, **one per link direction** (a single
   engine shared by both directions was the model until 2026-09-18 and costs +61–74 % on
   collectives, §4); IV+MAC bytes are counted on the wire.
4. Off-chip latency is 250 ns, not 500 ns (text error).
5. New experiments: collectives vs. N and tenants (§8), application traces (§9), engine scaling
   (§4 E1/E2), all from trace replay on the same platform.

Effect on the submitted numbers (old → new, IronBus/M3 slowdown): single-packet DMA and IPC
unchanged; 4 KiB DMA reads 1.35 → 1.18 (on-chip), 1.12 → 1.08 (off-chip); 4 KiB writes
1.26 → 1.10 / 1.16 → 1.22; m3fs read 1.23 → 1.16 / 1.12 → 1.11, write 1.18 → 1.11 /
1.14 → 1.17; imgproc 1.45 → 1.45 / 1.11 → 1.13; facever, imgclass, dist unchanged. Absolute
throughput of 4 KiB-granular DMA rises for both systems (m3fs off-chip +50 %).

## 2. IPC (message passing) — `bremote_ipc`, Fig. ipc

Client/server pingpong across two tiles, request of 8 B–2 KiB payload (message = header +
payload, one packet), reply of 8 B; 1000 iterations.

| payload | on-chip M3 / IronBus (cycles) | off-chip M3 / IronBus |
|---|---|---|
| 64 B | 245 / 310 (1.27×) | 1245 / 1310 (1.05×) |
| 512 B | 249 / 316 (1.27×) | 1249 / 1316 (1.05×) |
| 2 KiB | 379 / 461 (1.22×) | 1379 / 1461 (1.06×) |
| 8 KiB (4 msgs) | 656 / 784 (1.20×) | 1656 / 1784 (1.08×) |

Explanation: a round trip pays the request's and the reply's crypto latency (2 × (2L + B − 1) + 2
wire cycles); for small messages that is ~65 cycles on a 245-cycle round trip (27 %); off-chip
the 1000 cycles of link latency dominate (5–8 %). Unchanged from the submission except +2
cycles/packet for the IV+MAC bytes.

## 3. DMA to DRAM, per-packet cost — `bencrmgate`, Fig. read/write

A cached tile reads/writes 2 MiB of DRAM with 512 B–4 KiB per command (512 B–2 KiB = one packet
per command; 4 KiB = two packets, pipelined). GiB/s, M3 / IronBus (slowdown):

| command | on-chip read | on-chip write | off-chip read | off-chip write |
|---|---|---|---|---|
| 512 B | 4.10 / 2.88 (1.43×) | 10.33 / 5.10 (2.03×) | 0.77 / 0.71 (1.07×) | 0.87 / 0.80 (1.09×) |
| 1 KiB | 5.55 / 4.04 (1.37×) | 14.37 / 7.78 (1.85×) | 1.41 / 1.29 (1.10×) | 1.71 / 1.53 (1.11×) |
| 2 KiB | 6.37 / 4.90 (1.30×) | 13.77 / 10.45 (1.32×) | 2.41 / 2.14 (1.13×) | 3.26 / 2.80 (1.16×) |
| 4 KiB | 7.19 / 6.07 (1.18×) | 13.77 / 12.54 (1.10×) | 3.69 / 3.43 (1.08×) | 5.87 / 4.82 (1.22×) |

Explanation: with one packet per command the crypto latency of a packet (e.g., 157 cycles for
2 KiB) is fully exposed on every command; on-chip that is a large fraction of the ~600-cycle
command, off-chip it is small against 2 × 250 ns. Reads pay the request header (1 block) at the
requester and the response (B blocks) at the memory tile; writes pay the data at the requester and
the ack header at the memory tile. The 4 KiB row shows the first effect of pipelining (2 packets).
Absolute rates are bounded by the DRAM channel and, for reads on a cached tile, by the cache-fill
path — these are not link limits.

## 4. Device-to-device DMA: pipelining and crypto engines — `bp2pspm`, Fig. dma-pipelining (new)

Two SPM tiles; one reads/writes 2 MiB in the other's scratchpad with 1 MiB commands (no DRAM
involved), varying the number of packets in flight per command; then, with 64 packets in flight,
varying the link bandwidth (crossbar width) and the number of AES-GCM engines per AIU. Writes
shown (reads within 1 %).

| packets in flight | on-chip M3 / IronBus (GiB/s) | off-chip M3 / IronBus |
|---|---|---|
| 1 | 29.6 / 12.3 (2.4×) | 3.4 / 2.9 (1.17×) |
| 2 | 29.6 / 18.9 | 6.8 / 5.8 |
| 4 | 29.6 / 29.4 | 13.6 / 11.6 |
| 8 | 29.6 / 29.4 | 26.9 / 21.2 |
| 16 | 29.6 / 29.4 (1.005×) | 29.1 / 29.0 (**1.005×**) |

| link (GB/s) | M3 | IronBus 1 engine | 2 engines | 4 engines |
|---|---|---|---|---|
| 16 | 14.7 | 14.7 | 14.7 | 14.7 |
| 32 (platform) | 29.1 | 29.0 | 29.0 | 29.0 |
| 64 | 56.8 | **28.9** | 56.2 | 56.5 |

Explanation: with stop-and-wait DMA, every 2 KiB packet exposes its store-and-forward crypto
latency: on-chip a packet is transmitted in ~130 cycles but encrypted/verified in 157 + 2, so
IronBus reaches only 41 % of M3; off-chip the 500-cycle round trip dominates both. With 4 (on-chip)
or 16 (off-chip, bandwidth-delay product: 32 GB/s × ~0.5 µs ≈ 16 KiB) packets in flight the
latency is hidden and IronBus runs at the link rate, 0.5 % below M3 (the IV+MAC bytes). An AES-GCM
engine processes one 16 B block per cycle, i.e. 32 GB/s at 2 GHz: on a 32 GB/s link one engine is
never the bottleneck, on a 64 GB/s link one engine halves IronBus's throughput and two engines
restore it (−1 %). Beyond 64 GB/s the tile's own datapath (~61 GB/s) limits M3 and IronBus alike.
Take-aways for the paper: (i) the AIU must keep packets in flight — this is what makes the
security tax vanish for bulk transfers; (ii) the number of engines an AIU needs is
⌈link bandwidth / 32 GB/s⌉ at 2 GHz (R1.2, performance half; area half from OpenTitan synthesis).

**Engine model (since 2026-09-18):** engines are counted *per link direction*, as in PCIe IDE
implementations (Tx encrypts outgoing packets, Rx decrypts incoming; `M3_GEM5_CRYPTO_ENGINES=k`
= k per direction; `M3_GEM5_CRYPTO_SHARED=1` = the old single pool for both directions). The
single-stream numbers above are unaffected (only one direction is busy per tile), so the table
reads "engines per direction". Platform: 1 per direction.

**Engine-scaling matrix (R1.2) — the configurations to run and report, kept in sync with
`reproduce.py` (E1) and `src/tools/replay/sweep-engines.sh` (E2):**

| | workload | link (GB/s) | engines | answers |
|---|---|---|---|---|
| E1 | `bp2pspm` single stream, 64 in flight | 16, 32, 64 | 1, 2, 4 per direction; native | engines needed = ⌈link/32 GB/s⌉ per direction |
| E2a | all_reduce N=4, N=8 (replay, off-chip) | 32 | native; 1 shared; 1/dir; 2/dir | full duplex needs one per direction; 2/dir buys nothing at 32 GB/s |
| E2b | all_reduce N=4, N=8 (replay, off-chip) | 64 | native; 1/dir; 2/dir | count scales with link rate, not with N |
| E3 | OpenTitan AES-GCM synthesis | — | k = 1, 2, 4 | area/power = base + 2k × AES |

**E2 results (all_reduce, off-chip, 1 MiB, cycles; `sweep-engines.sh`, 2026-09-18):**

| | link | native | 1 shared | 1 per direction (platform) | 2 per direction |
|---|---|---|---|---|---|
| N=4 | 32 GB/s | 128.5k | 223.8k (+74 %) | 138.3k (+7.6 %) | 130.5k (+1.5 %) |
| N=4 | 64 GB/s | 96.1k | — | 142.0k (+48 %) | 99.9k (+3.9 %) |
| N=8 | 32 GB/s | 186.4k | 300.4k (+61 %) | 204.6k (+9.8 %) | 192.5k (+3.3 %) |
| N=8 | 64 GB/s | 140.1k | — | 216.9k (+55 %) | 158.9k (+13 %) |
| N=16 | 32 GB/s | 274.7k | 380.6k (+39 %) | 315.1k (+14.7 %) | 290.8k (+5.9 %) |
| N=16 | 64 GB/s | 234.9k | — | 347.7k (+48 %) | 243.1k (+3.5 %) |
| all_to_all N=16 | 32 GB/s | 144.7k | 201.4k (+39 %) | 163.0k (+12.6 %) | 151.5k (+4.7 %) |
| all_to_all N=16 | 64 GB/s | 134.9k | — | 176.0k (+30 %) | 136.2k (+1.0 %) |

(2 shared engines, the old "eng2" pool, N=4 at 32 GB/s: 133.3k, +3.7 %.) Reading: a shared
engine costs +61–74 %; the count scales with the link rate, not with N (same overhead within 2
points at N=4 and N=8); an engine at exactly line rate leaves a residual 8–10 % (bursts of
in-flight packets and the 40 B notifications queue behind data on the Rx engine), 2× headroom
brings it to 2–4 %. Recommendation for the paper: ⌈link / 32 GB/s⌉ engines per direction, one
more for the last few percent — priced by E3.

## 5. OS service and application workloads — Fig. app-bench

m3fs (in-memory file system service on another tile; client reads/writes a 2 MiB file in 4 KiB
chunks through trusted message and memory channels) and the four accelerator applications
(`config/accels.py`).

| workload | on-chip M3 → IronBus | off-chip M3 → IronBus |
|---|---|---|
| m3fs read (GiB/s) | 5.71 → 4.93 (1.16×) | 3.24 → 2.91 (1.11×) |
| m3fs write (GiB/s) | 1.98 → 1.79 (1.11×) | 0.86 → 0.73 (1.17×) |
| imgproc (FFT→mul→iFFT chain, 640 KB) | 1.37× (was 1.45× with one shared engine) | 1.11× (was 1.13×) |
| facever (GPU, 256 images) | 1.06× | 1.04× |
| imgclass (systolic array) | 1.01× | 1.01× |
| dist (2 systolic arrays) | 1.01× | 1.00× |

Explanation: m3fs and imgproc move data in 4 KiB chunks over cached tiles (2 packets per command)
and are dominated by the per-packet cost of §3 plus capability/metadata messages; the three
compute-heavy applications spend < 5 % of their time on the interconnect. Off-chip, imgproc's
overhead drops from 37 % to 11 % because link latency, not crypto, dominates. imgproc is the one
application whose stage tiles receive and send at the same time: with one AES-GCM engine per
direction (§4, since 2026-09-18) it is 5.7 % (on-chip) / 1.9 % (off-chip) faster than with the
single shared engine; the other three and the OS services are identical (single direction per
tile). (The submitted text's
absolute m3fs numbers — 5.3/4.3 and 2.16/1.92 GiB/s reads — must be replaced by the values above.)

## 6. Host-centric comparison — `bremote_ipc` chain tests, Fig. host-centric

Chains of 2–4 stages exchanging 512 B; host-centric routes every stage through a central tile
with per-link encryption (2N−2 transfers), IronBus uses direct channels (N−1). On-chip:
1123/2251/3378 cycles (host-centric) vs. 558/1113/1688 (IronBus) for N = 2/3/4 → 2.0× at every
length. Unchanged from the submission.

## 7. Trusted control channel creation (unchanged)

10.2 M cycles per tile (ECDSA signature generation 762 k, verification 482 k cycles — OpenTitan
numbers), one-time at boot.

## 8. Collectives vs. number of tiles and tenants — trace replay Part A, Fig. collectives (new)

`src/tools/replay/sweep-collectives.sh` (2026-09-18): all-reduce, all-gather, reduce-scatter,
all-to-all (1 MiB each) and a PP send/recv chain, N = 2, 4, 8, 16 SPM tiles, native / IronBus /
host-centric, on-chip and off-chip; plus k = 1, 2, 4 concurrent all-reduce N=4 instances sharing
one kernel/HAL. `collect.py` → `tools/replay-runs/replay.csv`, `plot_collectives.py` →
`benchmarks/exp-results/collectives.{csv,pdf,png}` and `collectives-setup.{pdf,png}`.

| pattern (off-chip) | N=2 | N=4 | N=8 | N=16 |
|---|---|---|---|---|
| all-reduce, native (µs) | 38.0 | 64.4 | 93.3 | 137.3 |
| IronBus / host vs. native | 1.03× / 2.9× | 1.08× / 4.4× | 1.11× / 7.7× | 1.15× / 15.0× |
| all-gather | 1.03× / 3.8× | 1.07× / 4.8× | 1.06× / 7.7× | 1.15× / 14.0× |
| reduce-scatter | 1.03× / 3.8× | 1.07× / 4.8× | 1.07× / 7.7× | 1.14× / 13.9× |
| all-to-all | 1.03× / 3.8× | 1.06× / 4.8× | 1.10× / 7.8× | 1.13× / 14.3× |
| PP chain | 1.00× / 1.3× | 1.02× / 2.3× | 1.01× / 3.1× | 1.01× / 3.8× |

On-chip IronBus is 1.04× (N=2) to 1.20–1.25× (N=16); host 3.9× to 15–16×. The IronBus overhead
grows with N because the per-step transfer shrinks (1 MiB/N: 64 KiB at N=16, each command paying
the crypto pipeline fill, cf. §4: −0.3 % at 1 MiB, −4 % at 64 KiB commands) and because the
line-rate engines queue under many simultaneous senders (§4 E2: 2 engines per direction bring
N=8 from +10 % to +3 %); it is in the send phase (recv waits are small). Host-centric scales with
N: one relay serializes all 2·N·(N−1)/N transfers.

Tenants (k = 1, 2, 4 concurrent all-reduce N=4; Fig. collectives-setup b): IronBus (k
independent coordinators, `REPLAY_INSTANCES=k`) per-tenant time 69.2 / 69.2 / 69.1 µs off-chip
(60.1 / 60.2 / 60.4 on-chip) — no interference once channels exist. Host-centric with **one host
relaying for all tenants** (`REPLAY_GROUPS=k`: k groups under one coordinator sharing the relay):
280 / 554 / 1108 µs off-chip, 250 / 498 / 1004 µs on-chip — per-tenant time grows linearly with
k (2× and 4× on-chip), the single host being the shared data-path bottleneck; a rank of tenant k
waits for the relay to serve the other tenants' transfers (96 transfers through one relay at
k = 4). IronBus with 4 groups under one coordinator gives the same 69.2 µs as 4 coordinators
(consistency check). IronBus channel setup per tenant 0.27 / 0.53 / 1.06 ms off-chip (0.15 /
0.28 / 0.45 on-chip): the kernel serializes the tenants' channel-creation syscalls — the HAL's
cost is one-time setup, not runtime, which is the answer to "a single HAL is centralized"
(R3.1). Channel setup vs. N (all-reduce, IronBus): 0.05 / 0.27 / 1.2 /
5.0 ms off-chip for N = 2 / 4 / 8 / 16 — quadratic, N(N−1) channels of two gates each, ~10 µs
per channel (activity creation + program load is separate and dominated by loading: 26 ms for
N=4 off-chip). This is the R1.1 "tenants / key storage" half.

Run-to-run noise ≈ 1 % (ring phase alignment); all setup syscalls are outside the measured replay.

## 9. Application traces — trace replay Part B, Fig. apps (new)

`src/tools/replay/sweep-apps.sh` (2026-09-18): 14 Chakra workloads replayed at their layouts
(STAGE Llama-7B, 4 layers, batch 1, seq 512 prefill / seq 1 decode, training at batch 8 / seq 2048;
ASTRA-sim 1.0 Transformer hybrid, DLRM hybrid, ResNet-50 DP — traffic scaled, see
`trace-replay-plan.md`), native / IronBus / host-centric, on-chip and off-chip; `plot_apps.py` →
`benchmarks/exp-results/apps.{csv,pdf,png}`. The figure shows the off-chip interconnect only —
these are multi-device workloads — the on-chip numbers are in the table below and in `apps.csv`.

| workload (N = accelerator tiles) | native off-chip (µs) | IronBus vs. native on / off | host vs. IronBus on / off |
|---|---|---|---|
| Llama-7B prefill TP4 (4) | 1138 | 1.03× / 1.05× | 2.8× / 2.8× |
| Llama-7B prefill TP2·PP2 (4) | 1306 | 1.01× / 1.01× | 1.6× / 1.6× |
| Llama-7B prefill TP4·PP2 (8) | 1071 | 1.04× / 1.05× | 3.0× / 3.0× |
| Llama-7B decode TP4 (4) | 199 | 1.06× / 1.04× | 1.8× / 2.8× |
| Llama-7B train TP4 (4) | 5171 | 1.02× / 1.02× | 2.8× / 2.7× |
| Llama-7B train DP4 (4) | 3624 | 1.01× / 1.01× | 2.0× / 2.1× |
| Llama-7B train TP4·PP2 (8) | 5017 | 1.01× / 1.02× | 2.9× / 2.9× |
| Llama-7B train DP8 (8) | 4158 | 1.02× / 1.01× | 5.8× / 5.9× |
| Transformer hybrid (4) | 18844 | 1.00× / 1.01× | 1.2× / 1.3× |
| Transformer hybrid (8) | 21414 | 1.01× / 1.01× | 2.2× / 2.2× |
| DLRM (4) | 106 | 1.27× / 1.04× | 3.4× / 3.9× |
| DLRM (8) | 223 | 1.21× / 1.07× | 8.6× / 8.2× |
| ResNet-50 DP (4) | 986 | 1.16× / 1.10× | 3.3× / 3.6× |
| ResNet-50 DP (8) | 1823 | 1.26× / 1.09× | 6.8× / 7.8× |

Reading: on the LLM and Transformer workloads (large, pipelined tensor exchanges) IronBus costs
0–6 %; DLRM and ResNet-50 (embedding exchanges of ~2.5 KiB, gradient chunks of ~11 KiB) pay the
per-packet cost of §3 — 4–10 % off-chip, 16–27 % on-chip where link latency does not hide the
crypto store-and-forward. Host-centric is 1.6–8.6× slower than IronBus, growing with the number
of accelerators and the communication share (DP8, DLRM 8). Compute is replayed from the traces
(busy-wait), so the ratios are conservative for compute-heavier runs (Transformer: 1.2–2.2×).

## 10. AIU hardware cost — E3 (R1.2), Table aiu-cost (new)

**Question.** What does the AIU cost in hardware (area, storage) and what does each additional
crypto engine cost (R1.2)? The comparison that answers it is against a TDISP/IDE-capable device
port — the device side of the host-centric designs the paper argues against — because IDE already
brings line-rate AES-GCM and a keystore, and SPDM/CMA already brings the root of trust. The DTU
(M3's interface unit) and a Rocket core serve as size anchors.

**Methodology.** The AIU has no RTL of its own; its cost is estimated from open RTL that
implements the functions of §4 of the paper, synthesized out-of-context with Vivado 2023.2
(`synth_design -mode out_of_context`, tables as logic: `-max_bram 0`) for a Xilinx UltraScale+
device (XCZU7EV; the DTU's published numbers are for the VCU118's XCVU9P, which needs a licence we
do not have — same CLB/LUT6 architecture, so LUT/FF counts are comparable) and reported next to
the DTU's published FPGA utilization (M3v, ASPLOS'22, Table 1, VCU118: DTU 15.2k LUTs / 5.8k FFs
/ 0.5 BRAM, of which the endpoint register file 2.0k / 1.0k; Rocket 46.6k, BOOM 143.8k LUTs). No
ASIC numbers: there is no DTU RTL to compare against (`tcu-if` is the specification and bitfiles).
No power figure (FPGA power is not representative); it scales with the number of line-rate engines.
Everything is in `/scratch/harshanavkis/ironbus/tools/aiu-syn/` (outside the repos): `rtl/`,
`tb/`, `tiny_aes/`, `ot/` (fusesoc file lists), `vivado/*.tcl`, `reports/`, `collect_area.py`
→ `aiu-area.csv`.

Blocks:
- *Line-rate AES-256-GCM engine* (one per link direction): Hsing's fully pipelined AES-256 core
  (`tiny_aes`, Apache-2.0: 14 unrolled rounds, one 16 B block per clock, 28-clock latency) in
  counter mode, a bit-parallel GF(2^128) GHASH multiplier absorbing one block per clock
  (`rtl/ghash.v`), IV/J0/tag logic and the data delay line (`rtl/aes_gcm_engine.v`). Verified
  against the GCM specification's AES-256 test cases 14 and 15 (ciphertext and tag, encrypt and
  decrypt; `tb/tb_gcm.v`). This is the engine the performance model assumes (16 B/cycle = 32 GB/s
  at 2 GHz). OpenTitan's AES is *not* this engine: it is iterative (16 clocks per block = 2 GB/s)
  and processes one stream at a time. The model's per-packet latency L = 15 assumes round keys
  ready at the pipeline (fixed key per endpoint); Hsing's core computes the schedule on the fly
  and has 28 stages — irrelevant for area, disclosed for consistency.
- *Control channel*: OpenTitan `aes`, iterative, **unmasked** (SecMasking=0, LUT S-box), GCM
  enabled, no AES-192: the low-rate unit that decrypts and authenticates configuration requests.
- *Root of trust*: OpenTitan `otbn` (ECDSA; a general-purpose 256-bit coprocessor with its own
  memories, used only during attestation and channel setup — reported as the upper bound, a
  fixed-function P-256 verifier is several times smaller), `csrng` + `edn` + `entropy_src`
  (RNG), `hmac` (SHA-2 measurement and key derivation; the paper fixes no hash, SHA-2 is the
  smaller block and what SPDM uses). OpenTitan at commit `aecc39ba9a` (2026-09-16), UltraScale
  primitive mapping, file lists from fusesoc 2.4 (two `tlul` core files edited for its schema).
- *Keystore*: sized from the design, no synthesis: per endpoint a 256-bit AES key and a 96-bit IV
  counter (an endpoint is send or receive), plus one 256-bit control key K_c per AIU.
- *TDISP/IDE column*: a functional mapping from the specifications (PCIe IDE ECN, TDISP 1.0,
  SPDM 1.2 / CMA) — no TDISP device RTL exists to synthesize — priced with the same blocks where
  the function is identical. IDE key storage: 3 sub-streams (PR, NPR, CPL) × 2 directions × 2
  key slots (refresh) = 12 keys per stream × (256-bit key + IV state) ≈ 0.53 KiB per stream; a
  port with 16 selective streams holds ≈ 8.4 KiB.

**Table aiu-cost.** Areas: UltraScale+ LUTs / FFs (/ BRAM); [T1] = M3v Table 1.

| # | function | TDISP / IDE device port has | IronBus AIU has | area (LUTs / FFs / storage) | Δ IronBus − TDISP |
|---|---|---|---|---|---|
| 1 | Interface unit: DMA & message engines, MMIO decode | PCIe endpoint controller + DMA engines (device-specific IP) | DTU DMA/message units, command controller, FIFOs | ≈ 13.2k / 4.8k [T1: DTU minus register file] | 0 — every device has one |
| 2 | Line-rate encryption, Tx | IDE Tx: AES-256-GCM at link rate | engine (pipelined AES-256 + GHASH) | 40.9k / 14.1k | 0 |
| 3 | Line-rate decryption / verification, Rx | IDE Rx: AES-256-GCM at link rate | engine | 40.9k / 14.1k | 0 |
| 4 | Packet framing: IV insertion, MAC append/check | IDE TLP: IV + 96-bit MAC (+ PCRC) | 96-bit IV + 128-bit MAC | inside rows 2–3 | logic 0; +4 B per packet on the wire |
| 5 | Key storage | 12 keys per IDE stream ≈ 0.53 KiB per stream; 16 selective streams ≈ 8.4 KiB | 192 endpoints × (256-bit key + 96-bit IV counter) + one 256-bit control key = 8.3 KiB | 2 BRAM36 or ≈ 1.1k LUT-RAM cells | ≈ 0 KiB; per endpoint instead of per stream |
| 6 | Endpoint / capability store | per-TDI configuration (a few TDIs) + selective-IDE association registers (per stream) | endpoint table: 192 × 256 bit = 6.0 KiB | 2.0k / 1.0k [T1: register file] | **+ 2.0k / 1.0k / 6 KiB** — finer granularity |
| 7 | Per-transfer permission check | T-bit / stream-binding check per TLP; TDI lock state | endpoint permission check on every DMA / message command | part of the DTU control unit (≤ 10.3k [T1]) | **+ ≈ 1–3k LUTs** (bounded by row 1's controller) |
| 8 | Authenticated configuration | configuration only through the SPDM secure session while the TDI is LOCKED | MMIO configuration only through the control channel (AES-GCM-authenticated) | 10.6k / 2.9k (OpenTitan AES, iterative, unmasked) | 0 |
| 9 | Management-session key derivation | HKDF over the SPDM session | HMAC-based KDF | 11.3k / 4.9k (OpenTitan HMAC / SHA-2) | 0 |
| 10 | Device identity & signed challenges | SPDM CHALLENGE / certificate: ECDSA signer | ECDSA (OTBN; upper bound) | 86.1k / 21.6k / 16.5 BRAM | 0 |
| 11 | Measurement | SPDM MEASUREMENTS: SHA-2 | HMAC / SHA-2 (row 9's block) | — | 0 |
| 12 | Random numbers | DRBG for nonces and IVs | CSRNG + EDN + entropy source | 24.3k / 14.9k | 0 |
| | **Total AIU hardware** (rows 1–12) | | | **229k LUTs / 74k FFs / 16.5 BRAM / 14.3 KiB state** (of which the DTU 15.2k / 5.8k) | |
| | **Total Δ over TDISP** | | | | **+ ≈ 3–5k LUTs, 1k FFs, 6 KiB (rows 6–7; ≤ one DTU, 15.2k LUTs, as the upper bound); everything else parity** |

Per-block synthesis results behind the table (`aiu-area.csv`):

| block | LUTs | FFs | BRAM |
|---|---|---|---|
| AES-256-GCM engine, one direction, tables as logic | 40.9k | 14.1k | 0 |
| — same engine with T-tables in block RAM | 21.5k | 10.2k | 242 |
| — bare AES-256 pipeline (tiny_aes) | 5.4k + tables | 8.6k | (242) |
| — GHASH, one GF(2^128) multiplier | 15.7k | 0.4k | 0 |
| — GHASH with 4 parallel multipliers (H…H^4, multi-GHz timing) | 39.0k | 0.8k | 0 |
| OpenTitan AES-256-GCM, iterative, unmasked (control channel) | 10.6k | 2.9k | 0 |
| OpenTitan AES, masked (DOM) — not used, for reference | 21.4k | 7.5k | 0 |
| OpenTitan HMAC | 11.3k | 4.9k | 0 |
| OpenTitan CSRNG / EDN / entropy source | 8.3k / 2.9k / 13.1k | 5.4k / 2.7k / 6.8k | 0 |
| OpenTitan OTBN | 86.1k | 21.6k | 16.5 |

Storage (192 endpoints): DTU endpoint state 192 × 256 bit = 6.0 KiB; IronBus adds 192 × 352 bit
+ 256 bit = 8.3 KiB (+138 %), 14.3 KiB in total. 64 / 512 endpoints: +2.8 / +22.0 KiB, linear
(R1.1's key-storage remark); an IDE port's keystore grows the same way with its streams.

**Engine scaling (E3 with E1/E2).** One engine per direction (40.9k LUTs, ≈ 0.9 Rocket cores)
sustains 32 GB/s; 64 GB/s needs two per direction (+81.7k LUTs) — E1 gives the link rate each
configuration reaches, E2 the collective overhead (1 per direction +8–15 %, 2 per direction
+2–6 % at 32 GB/s). A single engine shared by both directions saves one engine (40.9k LUTs) and
costs +61–74 % on collectives (§4): the false economy. The engine count follows the link
bandwidth, not the number of endpoints or tiles.

**Explanation / reading for the paper.**
1. *Against a TDISP/IDE device port the AIU adds no crypto hardware*: rows 2–3 and 8–12 are the
   same functions and, here, the same RTL; the keystore is the same size at a finer granularity.
   The hardware delta is the per-endpoint capability store and the per-command permission check
   (rows 6–7), ≈ 3–5k LUTs and 6 KiB, bounded above by one DTU — and on M3 these exist in the
   native DTU already; IronBus makes them the policy store. The device-side DSM (TDISP responder
   firmware on an embedded controller) has no counterpart in the AIU: policy lives in the HAL,
   which the TCB accounting covers.
2. *Against native M3* the AIU adds the two engines, the control-channel AES, the RoT and the
   keystore — 214k LUTs, i.e., about 1.5 BOOM cores or 4.6 Rocket cores, dominated by the line-rate
   crypto (82k; 81 % of an engine is the GHASH multiplier and the unrolled rounds) and by OTBN
   (86k, upper bound). This is the price of link encryption and attestation on a device, not of
   IronBus's design: an IDE/TDISP-capable device pays it too.
3. *What scales with what*: engines with link bandwidth (E1/E2), keystore and endpoint table
   linearly with endpoints (192 → 14.3 KiB), the RoT not at all (one per device).

Caveats stated in the text: the TDISP column is a functional mapping, not a synthesized device;
the IDE keystore figure depends on the number of selective streams (formula given); numbers are
for a free-tier UltraScale+ part with the same CLB architecture as the DTU's VCU118 part; OTBN is
an upper bound for the ECDSA block; the pipelined engine's latency (28) differs from the model's
L = 15 (round keys on the fly vs. precomputed) without affecting area.

**Paper-text consequences.** §4: the AIU's data-path engine is "a pipelined AES-256-GCM engine
(16 B/cycle, ~15-cycle latency, IDE-class)"; OpenTitan supplies the root-of-trust blocks and the
control-channel AES (the submission attributed the data-path AES to OpenTitan — clarify, and
note it in the response letter). §7: Table aiu-cost with rows 1–12 and the Δ column, the engine-
scaling sentence tied to E1/E2, and the storage line.

## 11. Pending experiments

- Larger collectives (8 MiB) at N=16 as a second mitigation point (E2 at N=16 is done: 2 engines
  per direction give +5.9 % / +4.7 %).
- (E3 done, §10.) Optional: a fixed-function ECDSA-P256 core instead of OTBN for the RoT row.
