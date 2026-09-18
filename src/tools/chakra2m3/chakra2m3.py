#!/usr/bin/env python3
"""
chakra2m3: converts Chakra execution traces (ETs) or ASTRA-sim 1.0 text workloads into
per-rank op programs for the M3 trace-replay application (tracereplay).

Per rank, the program is a sequence of fixed-size records (little endian, 24 bytes):

    u8  op      0 = COMP, 1 = SEND, 2 = RECV, 3 = END
    u8  peer    destination (SEND) / source (RECV) rank
    u16 tag     matches a SEND with the corresponding RECV on the peer
    u32 pad
    u64 arg     cycles (COMP) or bytes (SEND/RECV)
    u64 id      id of the trace node the op stems from (debugging)

What is taken from the trace: compute durations, communication (peer + size), the dependency
order and the communication groups of collectives. Collectives are lowered to point-to-point
ring/pairwise steps (see lower_collective). Tensor contents, memory nodes and timestamps are
not used; interconnect latency/bandwidth and crypto costs are produced by the simulator.

Compute time -> cycles:
  - Chakra COMP nodes with duration_micros (real traces, text-converted traces): the text
    converter stores the workload's cycle count in duration_micros, so it is taken as cycles
    (--comp-unit cycles, default); real traces use --comp-unit us.
  - STAGE COMP nodes (num_ops, tensor_size): roofline t = num_ops / min(peak, mem_bw * OI) on
    the reference GPU (--peak-tflops, --mem-bw-gbs), then cycles = t * f_sim * bw_ref / bw_sim
    (ratio-preserving scaling of compute to the simulated link).
"""

import argparse
import json
import os
import struct
import sys
from collections import defaultdict, deque

OP_COMP, OP_SEND, OP_RECV, OP_END = 0, 1, 2, 3
OP_NAMES = {OP_COMP: "COMP", OP_SEND: "SEND", OP_RECV: "RECV", OP_END: "END"}
RECORD = struct.Struct("<BBHIQQ")
assert RECORD.size == 24

# Chakra CollectiveCommType
ALL_REDUCE, REDUCE, ALL_GATHER, GATHER, SCATTER, BROADCAST, ALL_TO_ALL, REDUCE_SCATTER, \
    REDUCE_SCATTER_BLOCK, BARRIER = range(10)
COLL_NAMES = {ALL_REDUCE: "all_reduce", ALL_GATHER: "all_gather", ALL_TO_ALL: "all_to_all",
              REDUCE_SCATTER: "reduce_scatter", REDUCE_SCATTER_BLOCK: "reduce_scatter",
              BARRIER: "barrier", BROADCAST: "broadcast", REDUCE: "reduce"}
TEXT_COLL = {"ALLREDUCE": ALL_REDUCE, "ALLGATHER": ALL_GATHER, "ALLTOALL": ALL_TO_ALL,
             "REDUCESCATTER": REDUCE_SCATTER, "NONE": None}


class Op:
    __slots__ = ("op", "peer", "tag", "arg", "id")

    def __init__(self, op, peer=0, tag=0, arg=0, id=0):
        self.op, self.peer, self.tag, self.arg, self.id = op, peer, tag, arg, id

    def pack(self):
        return RECORD.pack(self.op, self.peer, self.tag, 0, self.arg, self.id)

    def __repr__(self):
        if self.op == OP_COMP:
            return "COMP %d cycles (n%d)" % (self.arg, self.id)
        if self.op == OP_END:
            return "END"
        return "%s peer=%d tag=%d bytes=%d (n%d)" % (OP_NAMES[self.op], self.peer, self.tag, self.arg, self.id)


# --------------------------------------------------------------------------------------------
# collective lowering
# --------------------------------------------------------------------------------------------

class TagAlloc:
    """Tags identify a (collective, step) so that the replay can match a RECV with the right
    SEND from a peer even when several are in flight. Per communication group, all ranks see
    the collectives in the same order, so a per-group counter yields identical tags on all
    ranks of the group."""

    def __init__(self):
        self.next = defaultdict(int)

    def alloc(self, group, steps):
        key = tuple(group)
        base = self.next[key]
        self.next[key] = (base + steps) & 0xFFFF
        return base


def lower_collective(kind, rank, group, size, tags, min_bytes, node_id):
    """Returns the list of (op) for `rank` for a collective of `kind` over `group` (sorted
    ranks) with total size `size` bytes, using ring (all-reduce = reduce-scatter + all-gather,
    all-gather, reduce-scatter) or pairwise (all-to-all) schedules. Conventions: all-reduce:
    every rank holds `size`; all-gather: `size` is the gathered result, each rank contributes
    size/N; reduce-scatter: every rank holds `size` as input; all-to-all: every rank sends
    size/N to every other rank. Per-rank bytes moved: 2(N-1)/N*size (all-reduce), (N-1)/N*size
    (others)."""
    n = len(group)
    if n < 2 or kind == BARRIER and n < 2:
        return []
    idx = group.index(rank)
    chunk = max(min_bytes, size // n)
    ops = []
    if kind in (ALL_REDUCE, ALL_GATHER, REDUCE_SCATTER, REDUCE_SCATTER_BLOCK, BARRIER):
        steps = 2 * (n - 1) if kind == ALL_REDUCE else (n - 1)
        if kind == BARRIER:
            chunk = min_bytes
        base = tags.alloc(group, steps)
        nxt, prv = group[(idx + 1) % n], group[(idx - 1) % n]
        for s in range(steps):
            tag = (base + s) & 0xFFFF
            ops.append(Op(OP_SEND, nxt, tag, chunk, node_id))
            ops.append(Op(OP_RECV, prv, tag, chunk, node_id))
    elif kind == ALL_TO_ALL:
        steps = n - 1
        base = tags.alloc(group, steps)
        for s in range(1, n):
            tag = (base + s - 1) & 0xFFFF
            ops.append(Op(OP_SEND, group[(idx + s) % n], tag, chunk, node_id))
            ops.append(Op(OP_RECV, group[(idx - s) % n], tag, chunk, node_id))
    elif kind in (BROADCAST, REDUCE):
        # root = first rank of the group; linear (chain) schedule over the ring
        steps = n - 1
        base = tags.alloc(group, steps)
        for s in range(steps):
            tag = (base + s) & 0xFFFF
            src, dst = group[s], group[s + 1]
            if kind == REDUCE:
                src, dst = group[n - 1 - s], group[n - 2 - s]
            if rank == src:
                ops.append(Op(OP_SEND, dst, tag, max(min_bytes, size), node_id))
            elif rank == dst:
                ops.append(Op(OP_RECV, src, tag, max(min_bytes, size), node_id))
    else:
        raise ValueError("unsupported collective %d" % kind)
    return ops


# --------------------------------------------------------------------------------------------
# input: Chakra ETs
# --------------------------------------------------------------------------------------------

def read_et(path):
    from chakra.schema.protobuf.et_def_pb2 import GlobalMetadata, Node
    from chakra.src.third_party.utils.protolib import decodeMessage
    nodes = []
    with open(path, "rb") as f:
        gm = GlobalMetadata()
        decodeMessage(f, gm)
        while True:
            n = Node()
            if not decodeMessage(f, n):
                break
            nodes.append(n)
    return nodes


def attr(node, name, default=None):
    for a in node.attr:
        if a.name == name:
            for field in ("int64_val", "uint64_val", "int32_val", "uint32_val", "string_val",
                          "bool_val", "double_val", "float_val"):
                if a.HasField(field):
                    return getattr(a, field)
            return default
    return default


def node_priority(node):
    """Order among ready nodes: a sequential rank must issue SENDs as early and blocking RECVs
    as late as possible (the simulator the traces were made for issues ready nodes concurrently)."""
    from chakra.schema.protobuf.et_def_pb2 import NodeType
    if node.type == NodeType.COMM_SEND_NODE:
        return 0
    if node.type == NodeType.COMM_RECV_NODE:
        return 2
    return 1


def topo_order(nodes):
    """Deterministic topological order (Kahn; ties by (priority, node id)) honouring data and
    control dependencies; dependencies on unknown nodes are ignored."""
    by_id = {n.id: n for n in nodes}
    indeg = {n.id: 0 for n in nodes}
    succ = defaultdict(list)
    for n in nodes:
        for d in set(list(n.data_deps) + list(n.ctrl_deps)):
            if d in by_id and d != n.id:
                indeg[n.id] += 1
                succ[d].append(n.id)
    import heapq
    ready = [(node_priority(by_id[i]), i) for i, d in indeg.items() if d == 0]
    heapq.heapify(ready)
    order = []
    while ready:
        _, i = heapq.heappop(ready)
        order.append(by_id[i])
        for s in succ[i]:
            indeg[s] -= 1
            if indeg[s] == 0:
                heapq.heappush(ready, (node_priority(by_id[s]), s))
    if len(order) != len(nodes):
        raise ValueError("dependency cycle in trace (%d of %d nodes ordered)" % (len(order), len(nodes)))
    return order


def convert_et(prefix, args):
    from chakra.schema.protobuf.et_def_pb2 import NodeType
    # rank files: <prefix>.<rank>.et
    ranks = []
    r = 0
    while os.path.exists("%s.%d.et" % (prefix, r)):
        ranks.append(r)
        r += 1
    if not ranks:
        sys.exit("no rank files %s.<rank>.et" % prefix)
    n = len(ranks)
    # communication groups (STAGE): <prefix>.json maps pg_name -> [ranks]
    groups = {}
    gfile = prefix + ".json"
    if os.path.exists(gfile):
        groups = {str(k): sorted(int(x) for x in v) for k, v in json.load(open(gfile)).items()}
    world = list(range(n))

    programs = {}
    stats = {}
    for rank in ranks:
        # one allocator per rank: all ranks of a group see the group's collectives in the same
        # order, so independent allocators produce identical tags
        tags = TagAlloc()
        nodes = read_et("%s.%d.et" % (prefix, rank))
        if args.forward_only:
            nodes = [n for n in nodes if not is_backward(n)]
        if args.drop_ops:
            drop = set(args.drop_ops.split(","))
            nodes = [n for n in nodes if n.name.split("@")[0].split(".")[-1] not in drop]
        ops = []
        st = defaultdict(int)
        for node in topo_order(nodes):
            if node.type == NodeType.COMP_NODE:
                cycles = comp_cycles(node, args)
                if cycles > 0:
                    ops.append(Op(OP_COMP, 0, 0, cycles, node.id))
                    st["comp_cycles"] += cycles
            elif node.type == NodeType.COMM_COLL_NODE:
                kind = int(attr(node, "comm_type", 0) or 0)
                size = int(attr(node, "comm_size", 0) or 0)
                pg = attr(node, "pg_name", None)
                group = groups.get(str(pg), world) if pg is not None else world
                if rank not in group:
                    continue
                size = scale_bytes(size, args)
                lowered = lower_collective(kind, rank, group, size, tags, args.min_bytes, node.id)
                ops.extend(lowered)
                st["collectives"] += 1
                st["coll_" + COLL_NAMES.get(kind, str(kind))] += 1
            elif node.type == NodeType.COMM_SEND_NODE:
                dst = int(attr(node, "comm_dst", 0) or 0)
                size = scale_bytes(int(attr(node, "comm_size", 0) or 0), args)
                tag = int(attr(node, "comm_tag", 0) or 0) & 0xFFFF
                ops.append(Op(OP_SEND, dst, tag, max(args.min_bytes, size), node.id))
            elif node.type == NodeType.COMM_RECV_NODE:
                src = int(attr(node, "comm_src", 0) or 0)
                size = scale_bytes(int(attr(node, "comm_size", 0) or 0), args)
                tag = int(attr(node, "comm_tag", 0) or 0) & 0xFFFF
                ops.append(Op(OP_RECV, src, tag, max(args.min_bytes, size), node.id))
            # MEM_LOAD/MEM_STORE and metadata nodes: local, not on the bus
        programs[rank] = ops
        stats[rank] = st
    return programs, stats


def is_backward(node):
    """STAGE names backward-pass ops d<something> (dx, dy, dw, dqkv, ...) or *_grad; forward ops
    are x*, y, q, k, v, o, w, ... (the comm nodes carry the same op names)."""
    seg = node.name.split("@")[0].split(".")[-1]
    return seg.startswith("d") or "grad" in seg


def comp_cycles(node, args):
    num_ops = attr(node, "num_ops", None)
    if num_ops is not None:
        # STAGE: roofline on the reference GPU
        tensor = float(attr(node, "tensor_size", 0) or 0)
        num_ops = float(num_ops)
        if num_ops <= 0:
            return 0
        peak = args.peak_tflops * 1e12
        if tensor > 0:
            oi = num_ops / tensor
            perf = min(peak, args.mem_bw_gbs * 1e9 * oi)
        else:
            perf = peak
        seconds = num_ops / perf
        cycles = seconds * args.sim_freq_ghz * 1e9 * (args.bw_ref_gbs / args.bw_sim_gbs)
    else:
        d = float(node.duration_micros)
        if args.comp_unit == "cycles":
            cycles = d
        else:
            cycles = d * 1e-6 * args.sim_freq_ghz * 1e9 * (args.bw_ref_gbs / args.bw_sim_gbs)
    return int(cycles * args.comp_scale)


def scale_bytes(size, args):
    return int(size * args.bytes_scale)


# --------------------------------------------------------------------------------------------
# input: ASTRA-sim 1.0 text workloads
# --------------------------------------------------------------------------------------------

class Layer:
    def __init__(self, cols):
        self.name = cols[0]
        self.fwd_comp, self.fwd_type, self.fwd_size = int(cols[2]), TEXT_COLL[cols[3]], int(cols[4])
        self.ig_comp, self.ig_type, self.ig_size = int(cols[5]), TEXT_COLL[cols[6]], int(cols[7])
        self.wg_comp, self.wg_type, self.wg_size = int(cols[8]), TEXT_COLL[cols[9]], int(cols[10])


def convert_text(path, args):
    """ASTRA-sim 1.0 text format. Execution per rank: forward pass over all layers, then the
    backward pass in reverse (input-gradient, weight-gradient), with the communications of the
    parallelisation scheme:
      DATA / MICRO:         wg comms over all ranks
      MODEL:                fwd/ig comms over all ranks
      HYBRID_TRANSFORMER g: fwd/ig comms within tensor-parallel groups of g consecutive ranks,
                            wg comms across the data-parallel groups (ranks with equal index
                            within their TP group); with N == g there is no DP communication
      HYBRID_DLRM:          all comms over all ranks (embedding all-to-all, MLP all-reduce)
      HYBRID_DATA_MODEL / HYBRID_MODEL_DATA: as HYBRID_TRANSFORMER with g from --tp
    """
    with open(path) as f:
        header = f.readline().split()
        num_layers = int(f.readline().strip())
        layers = []
        for line in f:
            cols = line.split()
            if len(cols) >= 11:
                layers.append(Layer(cols))
    layers = layers[:num_layers]
    if args.layers:
        layers = layers[:args.layers]
    ptype = header[0]
    n = args.num_ranks
    world = list(range(n))
    if ptype == "HYBRID_TRANSFORMER":
        g = int(header[-1]) if len(header) > 1 and header[-1].isdigit() else args.tp
    else:
        g = args.tp
    if ptype in ("HYBRID_TRANSFORMER", "HYBRID_DATA_MODEL", "HYBRID_MODEL_DATA"):
        g = max(1, min(g, n))
        tp_groups = [list(range(i, min(i + g, n))) for i in range(0, n, g)]
        dp_groups = [[r for r in world if r % g == k] for k in range(g)]
    else:
        tp_groups, dp_groups = [world], [world]

    def tp_group(rank):
        for grp in tp_groups:
            if rank in grp:
                return grp
        return world

    def dp_group(rank):
        for grp in dp_groups:
            if rank in grp:
                return grp
        return world

    fwd_ig_all = ptype in ("MODEL", "HYBRID_DLRM", "HYBRID_DLRM_ENHANCED")
    wg_all = ptype in ("DATA", "MICRO", "HYBRID_DLRM", "HYBRID_DLRM_ENHANCED")

    programs, stats = {}, {}
    for rank in world:
        tags = TagAlloc()  # per rank, see convert_et
        ops, st = [], defaultdict(int)
        nid = 0

        def comp(cycles):
            nonlocal nid
            nid += 1
            c = int(cycles * args.comp_scale)
            if c > 0:
                ops.append(Op(OP_COMP, 0, 0, c, nid))
                st["comp_cycles"] += c

        def comm(kind, size, group):
            nonlocal nid
            nid += 1
            if kind is None or size <= 0 or len(group) < 2:
                return
            ops.extend(lower_collective(kind, rank, group, scale_bytes(size, args), tags,
                                        args.min_bytes, nid))
            st["collectives"] += 1
            st["coll_" + COLL_NAMES.get(kind, str(kind))] += 1

        for layer in layers:
            comp(layer.fwd_comp)
            if ptype not in ("DATA", "MICRO"):
                comm(layer.fwd_type, layer.fwd_size, world if fwd_ig_all else tp_group(rank))
        if ptype != "MICRO":
            for layer in reversed(layers):
                comp(layer.ig_comp)
                if ptype != "DATA":
                    comm(layer.ig_type, layer.ig_size, world if fwd_ig_all else tp_group(rank))
                comp(layer.wg_comp)
                comm(layer.wg_type, layer.wg_size, world if wg_all else dp_group(rank))
        else:
            for layer in layers:
                comm(layer.wg_type, layer.wg_size, world)
        programs[rank] = ops
        stats[rank] = st
    return programs, stats


# --------------------------------------------------------------------------------------------
# synthetic patterns
# --------------------------------------------------------------------------------------------

def convert_pattern(args):
    n = args.num_ranks
    world = list(range(n))
    programs, stats = {}, {}
    size = scale_bytes(args.size, args)
    for rank in world:
        tags = TagAlloc()  # per rank, see convert_et
        ops, st = [], defaultdict(int)
        for it in range(args.iterations):
            if args.pattern == "chain":
                # pipeline: rank i receives from i-1, computes, sends to i+1
                if rank > 0:
                    ops.append(Op(OP_RECV, rank - 1, (it * n + rank) & 0xFFFF, size, it))
                if args.comp_cycles:
                    ops.append(Op(OP_COMP, 0, 0, args.comp_cycles, it))
                    st["comp_cycles"] += args.comp_cycles
                if rank < n - 1:
                    ops.append(Op(OP_SEND, rank + 1, (it * n + rank + 1) & 0xFFFF, size, it))
            else:
                kind = {"all_reduce": ALL_REDUCE, "all_gather": ALL_GATHER,
                        "reduce_scatter": REDUCE_SCATTER, "all_to_all": ALL_TO_ALL,
                        "barrier": BARRIER}[args.pattern]
                ops.extend(lower_collective(kind, rank, world, size, tags, args.min_bytes, it))
                st["collectives"] += 1
        programs[rank] = ops
        stats[rank] = st
    return programs, stats


# --------------------------------------------------------------------------------------------
# verification: simulate the replay protocol (per (src,dst) pair one message in flight, a SEND
# completes when the pair's slot is free, a RECV consumes the matching message)
# --------------------------------------------------------------------------------------------

def verify(programs):
    pc = {r: 0 for r in programs}
    inflight = {}  # (src, dst) -> (tag, bytes)
    progress = True
    while progress:
        progress = False
        for r, ops in programs.items():
            while pc[r] < len(ops):
                op = ops[pc[r]]
                if op.op == OP_COMP:
                    pc[r] += 1
                elif op.op == OP_SEND:
                    if (r, op.peer) in inflight:
                        break
                    inflight[(r, op.peer)] = (op.tag, op.arg)
                    pc[r] += 1
                elif op.op == OP_RECV:
                    m = inflight.get((op.peer, r))
                    if m is None:
                        break
                    if m[0] != op.tag or m[1] != op.arg:
                        raise ValueError("rank %d: RECV from %d expects tag %d/%d bytes, got %d/%d"
                                         % (r, op.peer, op.tag, op.arg, m[0], m[1]))
                    del inflight[(op.peer, r)]
                    pc[r] += 1
                else:
                    pc[r] += 1
                progress = True
    stuck = {r: pc[r] for r in programs if pc[r] < len(programs[r])}
    if stuck:
        details = ", ".join("rank %d at op %d (%s)" % (r, i, programs[r][i]) for r, i in stuck.items())
        raise ValueError("deadlock: " + details)
    if inflight:
        raise ValueError("unmatched sends: %s" % inflight)


# --------------------------------------------------------------------------------------------
# output
# --------------------------------------------------------------------------------------------

def split_transfers(programs, max_transfer):
    """Splits SEND/RECV ops larger than max_transfer into chunks with consecutive tags, so that
    the replay can use a fixed inbound slot size per peer. Applied identically on both sides
    of a transfer, so SEND/RECV pairing is preserved."""
    out = {}
    for rank, ops in programs.items():
        new = []
        for op in ops:
            if op.op in (OP_SEND, OP_RECV) and op.arg > max_transfer:
                chunks = (op.arg + max_transfer - 1) // max_transfer
                left = op.arg
                for i in range(chunks):
                    b = min(max_transfer, left)
                    left -= b
                    new.append(Op(op.op, op.peer, (op.tag * 64 + i) & 0xFFFF, b, op.id))
            else:
                if op.op in (OP_SEND, OP_RECV):
                    op = Op(op.op, op.peer, (op.tag * 64) & 0xFFFF, op.arg, op.id)
                new.append(op)
        out[rank] = new
    return out


def write_programs(name, outdir, programs, stats, args, source):
    os.makedirs(outdir, exist_ok=True)
    manifest = {"name": name, "source": source, "ranks": len(programs), "ranks_info": {},
                "bytes_scale": args.bytes_scale, "comp_scale": args.comp_scale,
                "min_bytes": args.min_bytes, "record_size": RECORD.size}
    for rank, ops in programs.items():
        peers = sorted({op.peer for op in ops if op.op in (OP_SEND, OP_RECV)})
        sent = sum(op.arg for op in ops if op.op == OP_SEND)
        recvd = sum(op.arg for op in ops if op.op == OP_RECV)
        maxb = max([op.arg for op in ops if op.op in (OP_SEND, OP_RECV)] + [0])
        with open(os.path.join(outdir, "%s.%d.m3t" % (name, rank)), "wb") as f:
            for op in ops:
                f.write(op.pack())
            f.write(Op(OP_END).pack())
        manifest["ranks_info"][str(rank)] = {
            "ops": len(ops) + 1, "peers": peers, "bytes_sent": sent, "bytes_recv": recvd,
            "max_transfer": maxb, "comp_cycles": stats[rank].get("comp_cycles", 0),
            "collectives": stats[rank].get("collectives", 0),
            "sends": sum(1 for op in ops if op.op == OP_SEND),
        }
    with open(os.path.join(outdir, name + ".json"), "w") as f:
        json.dump(manifest, f, indent=1)
    return manifest


def summarize(manifest):
    ri = manifest["ranks_info"]
    tot_sent = sum(v["bytes_sent"] for v in ri.values())
    max_sent = max(v["bytes_sent"] for v in ri.values())
    comp = max(v["comp_cycles"] for v in ri.values())
    sends = max(v["sends"] for v in ri.values())
    print("%s: %d ranks, %d ops/rank max, %.2f MiB sent per rank (max), %.2f MiB total, "
          "%d sends/rank (max), %.1f M compute cycles/rank (max), max transfer %d B"
          % (manifest["name"], manifest["ranks"], max(v["ops"] for v in ri.values()),
             max_sent / 2**20, tot_sent / 2**20, sends, comp / 1e6,
             max(v["max_transfer"] for v in ri.values())))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    src = p.add_mutually_exclusive_group(required=True)
    src.add_argument("--et", help="Chakra ET prefix (<prefix>.<rank>.et, optional <prefix>.json comm groups)")
    src.add_argument("--text", help="ASTRA-sim 1.0 text workload (.txt)")
    src.add_argument("--pattern", choices=["chain", "all_reduce", "all_gather", "reduce_scatter", "all_to_all", "barrier"])
    p.add_argument("--name", required=True, help="output name (<name>.<rank>.m3t, <name>.json)")
    p.add_argument("--out", default=".", help="output directory")
    p.add_argument("--num-ranks", type=int, default=4, help="ranks for --text/--pattern")
    p.add_argument("--tp", type=int, default=4, help="tensor-parallel group size for hybrid text workloads")
    p.add_argument("--layers", type=int, default=0, help="replay only the first k layers (text)")
    p.add_argument("--size", type=int, default=1 << 20, help="bytes per collective/transfer (--pattern)")
    p.add_argument("--iterations", type=int, default=1, help="repetitions (--pattern)")
    p.add_argument("--comp-cycles", type=int, default=0, help="compute per stage (--pattern chain)")
    p.add_argument("--bytes-scale", type=float, default=1.0, help="scale factor for all transfer sizes")
    p.add_argument("--comp-scale", type=float, default=None, help="scale factor for compute (default: = bytes scale)")
    p.add_argument("--min-bytes", type=int, default=64, help="minimum bytes per transfer")
    p.add_argument("--max-transfer", type=int, default=1 << 20, help="split larger transfers into chunks (replay slot size)")
    p.add_argument("--comp-unit", choices=["cycles", "us"], default="cycles", help="unit of duration_micros in ETs")
    p.add_argument("--sim-freq-ghz", type=float, default=2.0)
    p.add_argument("--bw-ref-gbs", type=float, default=32.0, help="reference link (PCIe Gen4 x16)")
    p.add_argument("--bw-sim-gbs", type=float, default=31.3, help="measured simulated link")
    p.add_argument("--peak-tflops", type=float, default=312.0, help="reference GPU peak (A100 PCIe, dense FP16)")
    p.add_argument("--mem-bw-gbs", type=float, default=1935.0, help="reference GPU memory bandwidth")
    p.add_argument("--forward-only", action="store_true", help="drop backward-pass nodes (STAGE inference)")
    p.add_argument("--drop-ops", default="", help="comma-separated op names to drop (e.g. 'w' = STAGE weight gathers)")
    p.add_argument("--dump", action="store_true", help="print the programs")
    p.add_argument("--no-verify", action="store_true")
    args = p.parse_args()
    if args.comp_scale is None:
        args.comp_scale = args.bytes_scale

    if args.et:
        programs, stats = convert_et(args.et, args)
        source = args.et
    elif args.text:
        programs, stats = convert_text(args.text, args)
        source = args.text
    else:
        programs, stats = convert_pattern(args)
        source = "pattern:" + args.pattern

    programs = split_transfers(programs, args.max_transfer)
    if not args.no_verify:
        verify(programs)
    manifest = write_programs(args.name, args.out, programs, stats, args, source)
    summarize(manifest)
    if args.dump:
        for rank, ops in programs.items():
            print("--- rank %d (%d ops)" % (rank, len(ops)))
            for op in ops[:200]:
                print("  ", op)
            if len(ops) > 200:
                print("   ... (%d more)" % (len(ops) - 200))


if __name__ == "__main__":
    main()
