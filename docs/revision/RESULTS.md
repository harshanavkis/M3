# IronBus evaluation — platform, methodology and results (TACO revision)

Working notes for §7 of the revised paper. Everything here was measured on 2026-09-17 with the
code at M3 commit `96a691dc0` (gem5 submodule `b0d732bc2`) via `python3 reproduce.py`; raw
numbers are in `snapshot.json` (`benchmarks/snapshot-stopwait.json` holds the numbers of the
original submission for comparison), logs in `benchmarks/exp-results/`, figures alongside.
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
  after tag verification (pp=1, "parallel pipelined"). In-flight packets share **one AES-GCM
  engine** per AIU (one block per cycle = 32 GB/s at 2 GHz; sender and receiver each book the
  engine for `B` cycles per packet). Every encrypted packet carries **32 B of IV+MAC** on the
  wire. Memory tiles have AIUs too (attested at boot, after the user tiles), so DRAM read
  responses are encrypted at the memory side and decrypted at the requester.
- **Attestation cost**: 10.2 M cycles (5.1 ms) per tile, one-time at boot; 23 tiles → ~117 ms.
- **Baselines**: *M3* = the same platform with encryption disabled (native, unprotected NoC);
  *host-centric* = every device-to-device transfer bounced through a host tile with per-link
  encryption (2N−2 hops instead of N−1; `bremote_ipc` chain tests).
- Environment (`benchmarks/constants.py`): `M3_GEM5_CPUFREQ=2GHz M3_GEM5_MEMFREQ=2GHz
  M3_PAR_PIPE=1 M3_ENCR_LATENCY=15 (IronBus) / 0 (M3) M3_INT_TRA_LATENCY=0|500
  M3_GEM5_INFLIGHT=16 M3_GEM5_BUFCOUNT=16 M3_GEM5_CRYPTO_ENGINES=1 M3_GEM5_CRYPTO_WIRE=32B`.

### What changed relative to the original submission (for the response letter)

1. Memory-tile AIUs are attested and charged (they were in the runs behind the submitted
   figures, but the tree had lost it; the revision's tree reproduces the submitted numbers to
   ≤ 0.01 % for single-packet DMA and ≤ 1.6 % for the applications when the new features are
   switched off).
2. DMA commands pipeline up to 16 packets (previously stop-and-wait per 2 KiB packet, which
   bounded any DMA to packet/RTT, e.g. 2.6 GB/s off-chip — no real DMA engine works that way).
3. In-flight packets share the AIU's AES-GCM engine(s); IV+MAC bytes are counted on the wire.
4. Off-chip latency is 250 ns, not 500 ns (text error).

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

First E2a numbers (N=4): native 128.5k cycles, 1 shared 223.8k (+74 %), 1/dir 138.3k (+7.6 %),
2 shared (old "eng2") 133.3k (+3.7 %); 2/dir and N=8 pending.

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

## 8. Pending experiments

- Collective communication patterns vs. number of tiles (own figure) and application traces —
  LLM inference/training, DLRM, ResNet-50 — on N = 4/8 tiles (own figure): see
  `trace-replay-plan.md`.
- OpenTitan synthesis (AES-GCM, OTBN, CSRNG, keystore SRAM) for the AIU area/power table.
