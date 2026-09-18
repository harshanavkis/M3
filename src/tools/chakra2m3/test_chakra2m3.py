#!/usr/bin/env python3
"""Tests for chakra2m3 (run: python3 test_chakra2m3.py). They only use the lowering, text
parsing and verification code, i.e., no Chakra installation is needed."""

import os
import sys
import tempfile
import types

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import chakra2m3 as c  # noqa: E402


def args(**kw):
    a = types.SimpleNamespace(bytes_scale=1.0, comp_scale=1.0, min_bytes=64, num_ranks=4, tp=4,
                              layers=0, size=1 << 20, iterations=1, comp_cycles=0,
                              comp_unit="cycles", sim_freq_ghz=2.0, bw_ref_gbs=32.0,
                              bw_sim_gbs=32.0, peak_tflops=312.0, mem_bw_gbs=1935.0,
                              pattern=None, forward_only=False, drop_ops='')
    for k, v in kw.items():
        setattr(a, k, v)
    return a


def volume(ops, op):
    return sum(o.arg for o in ops if o.op == op)


def test_ring_volumes():
    n, size = 8, 1 << 20
    group = list(range(n))
    for kind, factor in ((c.ALL_REDUCE, 2 * (n - 1) / n), (c.ALL_GATHER, (n - 1) / n),
                         (c.REDUCE_SCATTER, (n - 1) / n), (c.ALL_TO_ALL, (n - 1) / n)):
        progs = {r: c.lower_collective(kind, r, group, size, c.TagAlloc(), 64, 1) for r in group}
        for r in group:
            assert volume(progs[r], c.OP_SEND) == int(factor * size), (kind, r)
            assert volume(progs[r], c.OP_RECV) == int(factor * size), (kind, r)
    print("ring volumes ok")


def test_pairing_and_no_deadlock():
    n = 8
    group = list(range(n))
    for kind in (c.ALL_REDUCE, c.ALL_GATHER, c.REDUCE_SCATTER, c.ALL_TO_ALL, c.BROADCAST, c.REDUCE):
        tags = c.TagAlloc()
        # tags must be identical on all ranks: allocate with one allocator but reset per rank
        progs = {}
        for r in group:
            t = c.TagAlloc()
            t.next.update(tags.next)
            progs[r] = c.lower_collective(kind, r, group, 4096, t, 64, 1)
        # every SEND has exactly one matching RECV on the peer
        sends = {(r, o.peer, o.tag, o.arg) for r in group for o in progs[r] if o.op == c.OP_SEND}
        recvs = {(o.peer, r, o.tag, o.arg) for r in group for o in progs[r] if o.op == c.OP_RECV}
        assert sends == recvs, kind
        c.verify(progs)
    print("pairing and deadlock-freedom ok")


def test_two_collectives_in_sequence():
    n = 4
    group = list(range(n))
    tags = [c.TagAlloc() for _ in group]
    progs = {r: [] for r in group}
    for _ in range(3):
        for r in group:
            progs[r] += c.lower_collective(c.ALL_REDUCE, r, group, 1 << 16, tags[r], 64, 1)
            progs[r] += c.lower_collective(c.ALL_TO_ALL, r, group, 1 << 16, tags[r], 64, 2)
    c.verify(progs)
    print("sequenced collectives ok")


def test_text_transformer_groups():
    txt = """HYBRID_TRANSFORMER\tmodel_parallel_NPU_group: 2
2
L1\t-1\t100\tALLREDUCE\t4096\t200\tNONE\t0\t300\tALLREDUCE\t8192\t10
L2\t-1\t100\tNONE\t0\t200\tALLREDUCE\t4096\t300\tALLREDUCE\t8192\t10
"""
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        f.write(txt)
        path = f.name
    a = args(num_ranks=4, tp=2)
    progs, stats = c.convert_text(path, a)
    os.unlink(path)
    # TP groups {0,1},{2,3}: fwd/ig all-reduces of 4096 over 2 ranks -> (2-1)/2*2*4096 = 4096
    # per collective; wg all-reduces over DP groups {0,2},{1,3} of 8192 -> 8192 per collective
    for r in range(4):
        peers = {o.peer for o in progs[r] if o.op == c.OP_SEND}
        tp_peer = r ^ 1
        dp_peer = (r + 2) % 4
        assert peers == {tp_peer, dp_peer}, (r, peers)
        assert stats[r]["collectives"] == 4  # 2 fwd/ig + 2 wg
        assert stats[r]["comp_cycles"] == 2 * (100 + 200 + 300)
    c.verify(progs)
    # with N == group size there is no DP communication
    a = args(num_ranks=2, tp=2)
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        f.write(txt)
        path = f.name
    progs, stats = c.convert_text(path, a)
    os.unlink(path)
    assert stats[0]["collectives"] == 2
    print("text transformer groups ok")


def test_text_data_parallel_and_dlrm():
    txt = "DATA\n1\nconv\t-1\t50\tNONE\t0\t60\tNONE\t0\t70\tALLREDUCE\t65536\t5\n"
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        f.write(txt)
        path = f.name
    progs, stats = c.convert_text(path, args(num_ranks=4))
    os.unlink(path)
    assert all(stats[r]["collectives"] == 1 for r in range(4))
    assert volume(progs[0], c.OP_SEND) == 2 * 3 // 4 * 65536 // 1 or volume(progs[0], c.OP_SEND) == int(2 * 3 / 4 * 65536)
    c.verify(progs)
    txt = "HYBRID_DLRM\t1\n2\nEmb\t-1\t10\tALLTOALL\t4096\t10\tALLTOALL\t4096\t10\tNONE\t0\t1\nMLP\t-1\t20\tNONE\t0\t20\tNONE\t0\t20\tALLREDUCE\t4096\t1\n"
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        f.write(txt)
        path = f.name
    progs, stats = c.convert_text(path, args(num_ranks=4))
    os.unlink(path)
    assert stats[0]["coll_all_to_all"] == 2 and stats[0]["coll_all_reduce"] == 1
    c.verify(progs)
    print("text data-parallel and DLRM ok")


def test_scaling_and_min_bytes():
    n = 4
    group = list(range(n))
    ops = c.lower_collective(c.ALL_REDUCE, 0, group, 100, c.TagAlloc(), 64, 1)
    assert all(o.arg == 64 for o in ops)  # 100/4 = 25 < min_bytes
    print("min bytes ok")


def test_chain_pattern():
    a = args(num_ranks=4, pattern="chain", size=4096, iterations=2, comp_cycles=10)
    progs, stats = c.convert_pattern(a)
    c.verify(progs)
    assert volume(progs[0], c.OP_SEND) == 2 * 4096 and volume(progs[3], c.OP_RECV) == 2 * 4096
    print("chain pattern ok")


def test_deadlock_detection():
    # two ranks that both RECV first must be reported
    progs = {0: [c.Op(c.OP_RECV, 1, 0, 64), c.Op(c.OP_SEND, 1, 0, 64)],
             1: [c.Op(c.OP_RECV, 0, 0, 64), c.Op(c.OP_SEND, 0, 0, 64)]}
    try:
        c.verify(progs)
    except ValueError as e:
        assert "deadlock" in str(e)
        print("deadlock detection ok")
        return
    raise AssertionError("deadlock not detected")


if __name__ == "__main__":
    test_ring_volumes()
    test_pairing_and_no_deadlock()
    test_two_collectives_in_sequence()
    test_text_transformer_groups()
    test_text_data_parallel_and_dlrm()
    test_scaling_and_min_bytes()
    test_chain_pattern()
    test_deadlock_detection()
    print("all tests passed")
