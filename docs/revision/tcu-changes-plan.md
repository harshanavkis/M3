# TCU/AIU model fixes — implementation & evaluation plan

Prerequisite for the trace-replay experiments (see `trace-replay-plan.md` §0b). Scope: (1) charge crypto on DRAM reads, (2) pipelined multi-packet DMA commands, (3) shared crypto-engine occupancy with `k` engines, (4) IV+MAC bytes on the wire. The per-packet crypto formula `L + (B−1) + L` (pp=1) stays as is — it is the store-and-forward AIU described in §4.2. Everything is behind parameters whose defaults reproduce today's behaviour, so the old numbers can be regenerated bit-identically as a regression check.

## Changes

**C1 — Attest memory tiles (kernel, 0.5 day).** `src/kernel/src/tiles/tilemng.rs::deprivilege_tiles`: re-enable the commented loop over `mem.mods()` calling `attest_tile_master(tile)` for memory tiles (attest only; do not `deprivilege_tile` them). The protocol is TCU-driven (ext command `ATTEST`, attestation buffer at `TCU::attest_addr()`, reply to `KPEX_EP`), and memory tiles run the same `Tcu` model, so it should just work; verify with `hello.xml` (expect "Attesting tile: 22 … complete") and with `M3_GEM5_DBG=Tcu` that read responses now log `Total encryption cost:157`. Fallback if the protocol needs a CU on the tile: a `SET_ATTESTED` ext command the kernel issues to memory tiles after its own attestation. Kernel boot time grows by one more 10 M-cycle attestation per memory tile (report in §7).

**C2 — Pipelined DMA commands (gem5 `mem_unit.{cc,hh}`, `cmds.cc`, `tcu.hh`; 1.5–2 days).** New `Tcu` param `max_inflight_packets` (default 1 = today). READ/WRITE currently issue one ≤`max_noc_packet_size` packet and re-issue from `finishCommand` when `data.size > 0` (`cmds.cc:316`). Change:
- `MemoryUnit` keeps per-command state: issue cursor (`nextLocalAddr`, `nextOffset`, `remainingToIssue`), `outstanding` count, `firstError`.
- `startReadWithEP` / `startWriteWithEP`: validate as today for the first packet, then `issuePackets()`: while `remainingToIssue > 0 && outstanding < max_inflight` — per-packet page-boundary + TLB check on its own local range, then issue (`READ_REQ`, or local read → `WRITE_REQ` via `WriteTransferEvent`). On a per-packet error: stop issuing, record error.
- `NocSenderState` gains `localAddr`, `size`; `readComplete` uses them instead of the data register (which is advanced at completion by `finishReadWrite`, as today).
- `readComplete` / `writeComplete`: `outstanding--`; `finishReadWrite`; if no error and `remainingToIssue > 0` → `issuePackets()`; if `outstanding == 0 && (remainingToIssue == 0 || error)` → `scheduleCmdFinish`. `finishCommand` no longer re-issues for READ/WRITE.
- `cmdIsRemote` stays true while `outstanding > 0`; abort (`AbortType::REMOTE`) stops issuing and finishes with `ABORT` once outstanding drains.
- Buffers: each in-flight write holds an xfer buffer during its local read and each arriving read response needs one → set `buf_count ≥ max_inflight` in the config (this is where `buf_count` finally matters). Out-of-order completion is fine (each packet carries its own address).
- Messages (SEND/REPLY) are single-packet and untouched.

**C3 — Crypto-engine occupancy (gem5 `tcu.{cc,hh}`; 0.5 day).** New param `crypto_engines` (default 1). `totalEncryptionCost(size)` → `cryptoCost(size, dir)`: keep the latency formula, additionally book `B` cycles (one 16 B block/cycle) on the earliest-free of `k` engine slots (`engineFreeAt[k]`): `wait = max(0, freeAt − now)`, `freeAt = max(now, freeAt) + B`, return `wait + L + (B−1) + L + L_int`. Receiver side: in `recvFromNoc` (data packets) and the read-response path, book `B` on the receiver's engine and add only its queueing `wait` (the decryption latency is already the second `L` of the sender-side formula). With `max_inflight = 1` packets never overlap, so results are identical to today — regression check.

**C4 — IV+MAC on the wire (0.1 day).** Data packets carry 32 B more: add `divCeil(32, xbar_width) = 2` cycles to the packet's send delay when attested (equivalently enlarge `payloadDelay`). Text in §4.2 already claims this.

**C5 — Config plumbing (0.2 day).** `tcu_fs.py`: `M3_GEM5_INFLIGHT` → `max_inflight_packets`, `M3_GEM5_CRYPTO_ENGINES` → `crypto_engines`, `M3_GEM5_BUFCOUNT`/`M3_GEM5_PKTSIZE` (already added). Defaults = old behaviour.

**C6 — Benchmark (0.3 day).** `bencrmgate-large`: same as `bencrmgate` but with 64 KiB, 256 KiB and 1 MiB commands (static buffer), run once on a cached tile (`core`) and once on an SPM tile (`core|imem`) — the cached tile's local fill path is ~4 B/cycle (≈7 GB/s) and would otherwise mask the link.

## Evaluation

**E1 — Regression (0.3 day).** `max_inflight=1`, `crypto_engines=1`, memory attestation off → `bencrmgate`, `bremote_ipc`, `facever` must reproduce `benchmarks/exp-results/*-0.log` and `*-500.log` cycle-for-cycle. Then attestation on → only read-side numbers move; record the deltas for the response letter.

**E2 — Bandwidth calibration (0.3 day, runs in parallel).** `bencrmgate-large` on SPM and cached tiles: `inflight ∈ {1, 2, 4, 8, 16}` × `L ∈ {0, 500}` × native/encrypted, 2 KiB packets. Pick the smallest `inflight` that reaches ≥ 25 GB/s native at L=500 (Gen4-class; expected ≈ 8). That config becomes *the* platform: on-chip `L=0`, off-chip `L=500` (250 ns — the paper's plotted value), 2 GHz, pp=1. Table goes into §7 Methodology.

**E3 — Crypto-engine sweep (0.2 day).** At the chosen `inflight`: encrypted throughput vs `crypto_engines ∈ {1, 2, 4}` at L=0 and L=500. Expected: one 32 GB/s pipe is enough for the Gen4-class link (crypto adds latency only); a Gen5-class link would need two. This is the performance half of R1.2's "cost of more engines" (area half from OpenTitan later).

**E4 — Re-run the existing result set (0.5 day, minutes per run).** Everything behind the current figures (`benchmarks/`: read/write, IPC, syscall, fs, apps, host-centric chain) under the new platform; regenerate the plots. Expect: on-chip write/IPC unchanged, reads modestly slower (attestation), large transfers faster in absolute terms for both baseline and IronBus.

**E5 —** trace replay Parts A/B on the calibrated platform (`trace-replay-plan.md`).

## Timeline

≈ 5 working days: C1 (0.5) → C2 (2) → C3+C4+C5 (0.8) → C6 (0.3) → E1–E3 (0.8) → E4 (0.5). C1 and C6 can be done first while C2 is in progress; E1 runs after each change.

## Risks

- Memory-tile attestation may need a tile-side hook (fallback in C1).
- Abort/error semantics with several packets in flight — covered by E1 plus the existing `unittests`/`rustunittests` boot scripts, which exercise aborts and page faults.
- `xfer_unit` buffer pressure: transfers queue on free buffers; keep `buf_count ≥ inflight` and check no deadlock between outstanding reads and writes on the same TCU (the memory tile serves both).

## How to present this in the revision

State it in the Methodology and in the response letter as a model correction plus an extension: "(i) memory-tile AIUs now encrypt read responses (previously unaccounted), (ii) DMA commands keep up to N packets in flight, (iii) in-flight packets share the AIU's AES-GCM engine(s), (iv) IV/MAC bytes are counted on the wire; the off-chip latency is 250 ns (the earlier text said 500 ns in error). All results were regenerated; on-chip write and IPC results are unchanged, read-side overheads increased from X to Y." Two of the four changes make IronBus look worse and none change the design in §4.2, which is what makes the update credible to the same reviewers.


---

# Status (2026-09-17, implemented and measured)

## Changes made (all uncommitted; defaults reproduce the old behaviour)

- **Kernel** `src/kernel/src/tiles/tilemng.rs`: memory tiles are attested after the user tiles (attest only, no deprivilege). Verified: "Attesting tile: 22 … complete" in `hello.xml`.
- **gem5 TCU** (`platform/gem5/src/mem/tcu/`): `max_inflight_packets` (default 1), `crypto_engines` (default 0 = not modelled), `crypto_wire_overhead` (default 0 B). `mem_unit.cc` issues up to N packets per READ/WRITE command (`initCommand`/`issuePackets`/`packetDone`, per-packet local address in `NocSenderState`, packets cut at page boundaries), `cmds.cc` defers `finishCommand` while packets are in flight, `tcu.cc` books engine occupancy (sender and receiver) and the IV/MAC crossbar cycles on top of the unchanged `L + (B−1) + L` formula.
- **Config** `platform/gem5/configs/example/tcu_fs.py`: env knobs `M3_GEM5_INFLIGHT`, `M3_GEM5_CRYPTO_ENGINES`, `M3_GEM5_CRYPTO_WIRE`, `M3_GEM5_BUFCOUNT`, `M3_GEM5_PKTSIZE`, `M3_GEM5_XBAR_WIDTH`, `M3_GEM5_MEMCRYPTO=0` (old un-attested memory-tile behaviour).
- **Library** `src/libs/rust/base/src/arch/kachel/tcu.rs`: on tiles without virtual memory a transfer is one TCU command (was: one command per 4 KiB page, which caps pipelining at 2 packets); tiles with VM keep page-granular commands (translation faults are resolved per page).
- **Benchmarks**: `bmgatelarge` (DRAM, 4K–1M commands; boot files for SPM and cached tiles), `bp2pspm` (SPM→SPM device-to-device); `bremote_ipc` pingpong tests re-enabled.

## E1 — regression against `snapshot.json`

- **Finding:** the snapshot was produced *with* memory tiles attested. With the tree as found (memory tiles never attested), secure reads were 20 % faster than the snapshot (only the 1-block request header was charged). With C1 the secure `read-write` numbers match the snapshot to 0.00–0.01 % (reads) and ≤ 1.2 % (writes); native numbers match to ≤ 0.01 % (2/4 KiB) and ≤ 1.2 % (512 B/1 KiB — same wobble in the pristine tree with the longer boot).
- Apps (`config/accels.py`): imgproc, facever, img-class-systolic/-distinf within −0.1…−1.6 % of the snapshot, secure and native alike.
- IPC pingpong: the pristine tree (all changes stashed, rebuilt) gives *cycle-identical* numbers to the modified tree; both sit +1–4 % (4–11 cycles) above the snapshot, i.e., that offset predates this work. The secure−native gap is unchanged (61 cycles at 64 B).
- Conclusion: with defaults the model is neutral; C1 restores the paper's configuration.

## E2 — bandwidth calibration (2 KiB packets, 2 GHz, GB/s)

DRAM (memory tile, single DDR4-2400 channel) is the ceiling for host↔DRAM traffic: SPM tile, native, 1 MiB commands: on-chip 7.0 → 10.7 (≥4 in flight); off-chip 2.6 → 10.6 (8 in flight); writes 14.8 (DRAM-bound). Cached tiles stay ≤ 7.6 GB/s reads (4 KiB commands, cache-fill path).

**Device-to-device (SPM→SPM, `bp2pspm`), the relevant link for the trace replay:**

| in flight | native on-chip | native off-chip (250 ns) | IronBus off-chip, 1 engine | overhead |
|---|---|---|---|---|
| 1 | 31.7 | 3.7 | 3.0 | −18 % |
| 4 | 31.7 | 14.6 | 12.0 | −18 % |
| 8 | 31.7 | 29.0 | 23.7 | −18 % |
| 16 | 31.7 | **31.3** (line rate) | **31.2** | **−0.3 %** |

(1 MiB commands; crossbar line rate = 16 B/cycle × 2 GHz = 32 GB/s.) With 64 KiB commands IronBus is −4 %, with 4 KiB commands (2 packets) −14 %. Engines 1/2/4: identical for large transfers (link ≤ 32 GB/s < one 32 GB/s pipe), 2 engines gain ~3 % on 4 KiB commands.

**Platform for the revision:** `M3_GEM5_INFLIGHT=16 M3_GEM5_BUFCOUNT=16 M3_GEM5_CRYPTO_ENGINES=1 M3_GEM5_CRYPTO_WIRE=32B`, memory tiles attested, `L=0` (on-chip) / `L=500` cycles = 250 ns (off-chip), 2 GHz, pp=1 → a 32 GB/s link (between PCIe Gen4 and Gen5 x16 effective) with 250 ns one-way latency.

**Model fix found by E4:** the first receiver-side engine booking counted the B decryption cycles *after* packet arrival, although the sender-side formula already charges that decryption before delivery; this delayed the next command by up to 55 cycles per 2 KiB packet even with one packet in flight. Fixed: the receiver books the B-cycle window *ending* at arrival (`Tcu::receiveDecryptionWait`). Check: single packet + 1 engine + 32 B wire now gives secure 2 KiB reads identical to the snapshot (797229 vs 797231) and writes +2 % (the wire bytes).

## E3 — crypto engines on a 64 GB/s link (`M3_GEM5_XBAR_WIDTH=32`, SPM→SPM, 1 MiB commands, 32 in flight, GB/s)

| | on-chip | off-chip 250 ns |
|---|---|---|
| native | 62.8 | 61.0 |
| IronBus, 1 engine | 31.6 (−50 %, engine-bound at 16 B/cycle) | 31.2 (−49 %) |
| IronBus, 2 engines | 62.2 (−1 %) | 60.4 (−1 %) |
| IronBus, 4 engines | 62.5 | 60.7 |

On the 32 GB/s link one engine is enough (−0.3 %); on a 64 GB/s (Gen5-class) link one engine halves throughput and two restore it — the performance half of R1.2.

## E4 — existing benchmark set on the new platform (INFLIGHT=16, 1 engine, 32 B wire, memory tiles attested) vs. `snapshot.json`

- Single-packet transfers (≤ 2 KiB commands) and IPC pingpong: unchanged (≤ 0.6 %; 512 B ≤ 2.8 % from wire bytes/boot alignment).
- 4 KiB commands (2 packets pipelined): native reads −8 % on-chip / −34 % off-chip, native writes 0 % / −44 %; secure reads −19 % / −36 %, secure writes −13 % / −41 %.
- `fs` (m3fs 2 MiB read/write): −7…−13 % on-chip, −33…−36 % off-chip, native and secure alike; `imgproc` −9…−11 %; `facever`, `img-class-*` ≤ 1.6 % (compute-bound).
- IronBus overhead vs. native on the new platform (4 KiB commands): reads +18 % on-chip / +8 % off-chip (was +35 % / +12 %), writes +10 % / +22 % (was +26 % / +16 %); m3fs read +16 % / +11 %, write +11 % / +17 %.
- Cached (VM) tiles still issue page-granular (4 KiB) commands, so they pipeline only 2 packets; SPM tiles hand whole transfers to the TCU and reach line rate. State this in Methodology.

Raw logs and tables: scratchpad `regress/out/{base,c1,c1app,c1ipc,pristine,e2,p2p,w32,e4,iso}`, scripts `regress/runbench.sh`, `compare.py`, `analyze_large.py`.
