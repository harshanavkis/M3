/*
 * tracereplay: replays per-rank op programs produced by chakra2m3 on a set of tiles.
 *
 *   tracereplay coord <name> <ranks> <mode> [inst] [groups]   (started from the boot script;
 *   inst numbers concurrent coordinators; groups > 1 runs that many independent groups of
 *   <ranks> ranks under one coordinator — in host mode they share the single relay)
 *
 * The coordinator starts one activity per rank on scratchpad tiles (and, in host-centric mode,
 * a relay activity), loads the ranks' programs /traces/<name>.<rank>.m3t into their buffers
 * (the ranks themselves use no file system), wires up the channels and starts the replay.
 * Every rank executes its program: COMP busy-waits, SEND writes the data into the peer's
 * inbound slot with a memory channel and notifies the peer with a message, RECV waits for the
 * matching notification and acknowledges it (which returns the sender's credit and frees the
 * slot). Modes: "native"/"ironbus" (direct rank-to-rank channels; encryption is a platform
 * setting) and "host" (every transfer is written to the relay, which forwards it to the
 * destination, i.e., the 2N-2 transfers of a host-mediated design with per-link protection).
 *
 * Flow control is one transfer in flight per (source, destination) pair, which is what the
 * converter's verifier assumes: the sender's notification is credited back when the receiver
 * has consumed it. In host mode the relay keeps that invariant per pair on the second hop and
 * buffers transfers that are not yet deliverable, like a host with ample memory would.
 */

#![no_std]

use core::cmp;
use m3::cell::StaticRefCell;
use m3::col::Vec;
use m3::com::{MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::io::Read;
use m3::kif;
use m3::kif::{CapRngDesc, CapType};
use m3::mem::{AlignedBuf, MsgBuf};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, RunningProgramActivity, Tile};
use m3::time::{CycleInstant, Duration};
use m3::vfs::{OpenFlags, VFS};
use m3::{env, format, println, wv_assert_ok};

const MAX_RANKS: usize = 16;
const SLOT: usize = 1024 * 1024; // inbound slot per peer (chakra2m3 --max-transfer)
const RG_ORDER: u32 = 13; // 8 KiB receive buffer ...
const RG_MSG_ORDER: u32 = 8; // ... of 256 B messages (32 slots, the kernel maximum)

const OP_COMP: u8 = 0;
const OP_SEND: u8 = 1;
const OP_RECV: u8 = 2;
const OP_END: u8 = 3;

const MODE_DIRECT: u64 = 0;
const MODE_HOST: u64 = 1;

const KIND_DATA: u64 = 1;
const KIND_GO: u64 = 2;
const KIND_ADDR: u64 = 3;
const KIND_STATS: u64 = 4;
const KIND_STOP: u64 = 5;
const KIND_READY: u64 = 6;
const KIND_START: u64 = 7;

const LABEL_COORD: u32 = 0xFFFF;
const LABEL_RELAY: u32 = 0xFFFE;

// Channels are delegated to the members after they started, i.e., into a selector space the
// member allocates from at the same time (reply gate, EPs on first use of a gate, files).
// Hence, they go into a reserved high range that the member's own allocator never reaches.
const CHAN_SEL: kif::CapSel = 2000;

// inbound slots (one per peer; the relay uses one per source) and the local send buffer
static INBOUND: StaticRefCell<AlignedBuf<{ MAX_RANKS * SLOT }>> =
    StaticRefCell::new(AlignedBuf::new_zeroed());
static SENDBUF: StaticRefCell<AlignedBuf<SLOT>> = StaticRefCell::new(AlignedBuf::new_zeroed());

#[repr(C)]
#[derive(Clone, Copy)]
struct Notify {
    kind: u64,
    src: u64,
    dst: u64,
    tag: u64,
    bytes: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Go {
    kind: u64,
    ranks: u64,
    mode: u64,
    // per peer p: mem gate selector and send gate selector (0 = none); ranks index by their
    // group-local peer, the relay by the global rank
    mgates: [u32; MAX_RANKS],
    sgates: [u32; MAX_RANKS],
    // length of the program the coordinator wrote to the start of our inbound buffer
    prog_len: u64,
    // global rank of this group's rank 0 (ranks are numbered globally across groups)
    base: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Stats {
    kind: u64,
    rank: u64,
    total: u64,
    comp: u64,
    send: u64,
    recv: u64,
    ops: u64,
    bytes: u64,
}

fn words_to<T: Copy>(words: &[u64]) -> T {
    assert!(words.len() * 8 >= core::mem::size_of::<T>());
    // safety: T is a repr(C) POD of u64/u32 fields and the message is 8-byte aligned
    unsafe { core::ptr::read_unaligned(words.as_ptr() as *const T) }
}

// Credits of a send gate are returned to the sender by the receiver's reply. Hence, every
// received message is answered with a reply (`done`) that echoes the message (so that the relay
// can tell which transfer was consumed), replies are collected on a per-activity reply gate,
// and senders drain that gate (`drain`) before sending.

const REPLY_WORDS: usize = 5; // the words of Notify

fn done(rgate: &RecvGate, msg: &'static m3::tcu::Message) {
    let w = msg.as_words();
    let mut echo = [0u64; REPLY_WORDS];
    echo.copy_from_slice(&w[..REPLY_WORDS]);
    let mut reply = MsgBuf::new();
    reply.set(echo);
    wv_assert_ok!(rgate.reply(&reply, msg));
}

fn drain(reply_gate: &RecvGate) {
    while let Some(r) = reply_gate.fetch() {
        wv_assert_ok!(reply_gate.ack_msg(r));
    }
}

fn send_struct<T: Copy>(sgate: &SendGate, reply_gate: &RecvGate, v: T) -> Result<(), Error> {
    drain(reply_gate);
    let mut msg = MsgBuf::new();
    msg.set(v);
    sgate.send(&msg, reply_gate)
}

fn send_struct_retry<T: Copy>(sgate: &SendGate, reply_gate: &RecvGate, v: T) {
    loop {
        match send_struct(sgate, reply_gate, v) {
            Ok(_) => return,
            Err(e) if e.code() == Code::NoCredits => continue,
            Err(e) => panic!("send failed: {:?}", e),
        }
    }
}

// a gate for 2^(order-6) replies of 64 B; replies need no reply endpoints
fn new_reply_gate(order: u32) -> RecvGate {
    let mut rg = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(order).msg_order(6).replies(false)
    ));
    wv_assert_ok!(rg.activate());
    rg
}

// waits for a message of `kind` on `rgate` (other messages are acknowledged and skipped)
fn wait_for(rgate: &RecvGate, kind: u64) -> [u64; 32] {
    loop {
        let msg = wv_assert_ok!(rgate.receive(None));
        let words = msg.as_words();
        let mut copy = [0u64; 32];
        let n = cmp::min(words.len(), copy.len());
        copy[..n].copy_from_slice(&words[..n]);
        let found = words[0] == kind;
        // transfers cannot arrive before the start (READY/START barrier); do not drop them
        assert!(words[0] != KIND_DATA, "unexpected transfer while waiting for kind {}", kind);
        done(rgate, msg);
        if found {
            return copy;
        }
    }
}

fn busy_wait(cycles: u64) {
    let end = CycleInstant::now().as_cycles() + cycles;
    while CycleInstant::now().as_cycles() < end {}
}

// ------------------------------------------------------------------------------------------
// rank
// ------------------------------------------------------------------------------------------

struct Op {
    op: u8,
    peer: u8,
    tag: u16,
    arg: u64,
}

// reads /traces/<name>.<rank>.m3t (coordinator side)
fn read_program(name: &str, rank: usize) -> Vec<u8> {
    let path = format!("/traces/{}.{}.m3t", name, rank);
    let mut file = wv_assert_ok!(VFS::open(&path, OpenFlags::R));
    let mut data: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = wv_assert_ok!(file.read(&mut buf));
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    data
}

// decodes a program (24 B records, see chakra2m3)
fn parse_program(data: &[u8]) -> Vec<Op> {
    let mut ops = Vec::new();
    let mut i = 0;
    while i + 24 <= data.len() {
        let r = &data[i..i + 24];
        let tag = u16::from_le_bytes([r[2], r[3]]);
        let arg = u64::from_le_bytes([r[8], r[9], r[10], r[11], r[12], r[13], r[14], r[15]]);
        ops.push(Op {
            op: r[0],
            peer: r[1],
            tag,
            arg,
        });
        i += 24;
    }
    ops
}

fn rank_main() -> i32 {
    let mut src = Activity::own().data_source();
    let rank: u64 = src.pop().unwrap();
    let name: m3::col::String = src.pop().unwrap();
    let rgate_sel: kif::CapSel = src.pop().unwrap();
    let coord_sel: kif::CapSel = src.pop().unwrap();

    let mut rgate = RecvGate::new_bind(rgate_sel, RG_ORDER, RG_MSG_ORDER);
    wv_assert_ok!(rgate.activate());
    let to_coord = SendGate::new_bind(coord_sel);
    let reply_gate = new_reply_gate(11);
    let _ = name;

    // announce our buffers
    let inbound = INBOUND.borrow().as_ptr() as u64;
    let sendbuf = SENDBUF.borrow().as_ptr() as u64;
    send_struct_retry(&to_coord, &reply_gate, Notify {
        kind: KIND_ADDR,
        src: rank,
        dst: inbound,
        tag: sendbuf,
        bytes: 0,
    });

    // wait for the go message with our channel selectors; by then the coordinator has written
    // our program to the start of the inbound buffer (unused until START)
    let go: Go = words_to(&wait_for(&rgate, KIND_GO));
    let ranks = go.ranks as usize;
    let host = go.mode == MODE_HOST;
    let base = go.base;
    let ops = parse_program(&INBOUND.borrow()[..go.prog_len as usize]);

    let mut mgates: Vec<Option<MemGate>> = Vec::new();
    let mut sgates: Vec<Option<SendGate>> = Vec::new();
    for p in 0..ranks {
        mgates.push(if go.mgates[p] != 0 { Some(MemGate::new_bind(go.mgates[p] as kif::CapSel)) } else { None });
        sgates.push(if go.sgates[p] != 0 { Some(SendGate::new_bind(go.sgates[p] as kif::CapSel)) } else { None });
    }
    // channel setup (endpoint activation) is not part of the replay: activate, report ready
    // and wait for the start
    for mg in mgates.iter().flatten() {
        wv_assert_ok!(mg.activate());
    }
    for sg in sgates.iter().flatten() {
        wv_assert_ok!(sg.activate());
    }
    send_struct_retry(&to_coord, &reply_gate, Notify {
        kind: KIND_READY,
        src: rank,
        dst: 0,
        tag: 0,
        bytes: 0,
    });
    wait_for(&rgate, KIND_START);
    // in host mode, index 0 holds the channel to the relay
    let chan = |p: usize| if host { 0 } else { p };

    let sbuf = SENDBUF.borrow();
    // notifications that arrived before we were ready for them (one per source at most)
    let mut parked: Vec<&'static m3::tcu::Message> = Vec::new();

    let mut t_comp = 0u64;
    let mut t_send = 0u64;
    let mut t_recv = 0u64;
    let mut bytes = 0u64;
    let start = CycleInstant::now();

    for op in &ops {
        match op.op {
            OP_COMP => {
                let t = CycleInstant::now();
                busy_wait(op.arg);
                t_comp += CycleInstant::now().duration_since(t).as_raw();
            },
            OP_SEND => {
                let t = CycleInstant::now();
                let dst = op.peer as usize;
                let c = chan(dst);
                let mg = mgates[c].as_ref().expect("no memory channel to peer");
                let sg = sgates[c].as_ref().expect("no send channel to peer");
                let len = cmp::min(op.arg as usize, SLOT);
                // data into the peer's (or the relay's) slot, then the notification; the
                // notification only succeeds once the previous one to this channel was consumed
                wv_assert_ok!(mg.write(&sbuf[..len], 0));
                send_struct_retry(sg, &reply_gate, Notify {
                    kind: KIND_DATA,
                    src: rank,
                    dst: base + dst as u64,
                    tag: op.tag as u64,
                    bytes: op.arg,
                });
                bytes += op.arg;
                t_send += CycleInstant::now().duration_since(t).as_raw();
            },
            OP_RECV => {
                let t = CycleInstant::now();
                let want_src = base + op.peer as u64;
                let want_tag = op.tag as u64;
                // parked first
                let mut found = None;
                for (i, m) in parked.iter().enumerate() {
                    let w = m.as_words();
                    if w[1] == want_src && w[3] == want_tag {
                        found = Some(i);
                        break;
                    }
                }
                if let Some(i) = found {
                    let m = parked.remove(i);
                    done(&rgate, m);
                }
                else {
                    loop {
                        drain(&reply_gate);
                        let msg = wv_assert_ok!(rgate.receive(None));
                        let w = msg.as_words();
                        if w[0] == KIND_DATA && w[1] == want_src && w[3] == want_tag {
                            assert!(w[4] == op.arg, "size mismatch");
                            done(&rgate, msg);
                            break;
                        }
                        parked.push(msg);
                    }
                }
                t_recv += CycleInstant::now().duration_since(t).as_raw();
            },
            OP_END => break,
            _ => panic!("bad op"),
        }
    }
    let total = CycleInstant::now().duration_since(start).as_raw();

    send_struct_retry(&to_coord, &reply_gate, Stats {
        kind: KIND_STATS,
        rank,
        total,
        comp: t_comp,
        send: t_send,
        recv: t_recv,
        ops: ops.len() as u64,
        bytes,
    });
    // leave only when all members are done, so that every credit has returned to its gate
    wait_for(&rgate, KIND_STOP);
    0
}

// ------------------------------------------------------------------------------------------
// relay (host-centric mode): forwards every transfer from its per-source slot to the
// destination's inbound slot and notifies the destination. The source is credited as soon as
// the data has been copied; the notification to the destination waits until the destination
// has consumed the previous transfer of the same source (one in flight per pair, as on a
// direct channel), so a destination that is not ready does not stall the other destinations.
// ------------------------------------------------------------------------------------------

// acknowledges the replies to our notifications: the echoed Notify tells which pair is free
fn relay_drain(reply_gates: &[RecvGate], outstanding: &mut [[bool; MAX_RANKS]; MAX_RANKS]) {
    for rg in reply_gates {
        while let Some(r) = rg.fetch() {
            let w = r.as_words();
            if w[0] == KIND_DATA {
                outstanding[w[1] as usize][w[2] as usize] = false;
            }
            wv_assert_ok!(rg.ack_msg(r));
        }
    }
}

fn relay_main() -> i32 {
    let mut src = Activity::own().data_source();
    let _rank: u64 = src.pop().unwrap();
    let _name: m3::col::String = src.pop().unwrap();
    let rgate_sel: kif::CapSel = src.pop().unwrap();
    let coord_sel: kif::CapSel = src.pop().unwrap();

    let mut rgate = RecvGate::new_bind(rgate_sel, RG_ORDER, RG_MSG_ORDER);
    wv_assert_ok!(rgate.activate());
    let to_coord = SendGate::new_bind(coord_sel);
    let reply_gate = new_reply_gate(11);

    let inbound = INBOUND.borrow().as_ptr() as u64;
    send_struct_retry(&to_coord, &reply_gate, Notify {
        kind: KIND_ADDR,
        src: LABEL_RELAY as u64,
        dst: inbound,
        tag: 0,
        bytes: 0,
    });

    let go: Go = words_to(&wait_for(&rgate, KIND_GO));
    let ranks = go.ranks as usize;
    let mut mgates: Vec<MemGate> = Vec::new();
    let mut sgates: Vec<SendGate> = Vec::new();
    // one reply gate per destination: up to ranks-1 notifications (one per source) can be
    // outstanding per destination and a receive buffer has at most 32 slots
    let mut reply_gates: Vec<RecvGate> = Vec::new();
    for p in 0..ranks {
        mgates.push(MemGate::new_bind(go.mgates[p] as kif::CapSel));
        sgates.push(SendGate::new_bind(go.sgates[p] as kif::CapSel));
        reply_gates.push(new_reply_gate(11));
        wv_assert_ok!(mgates[p].activate());
        wv_assert_ok!(sgates[p].activate());
    }
    send_struct_retry(&to_coord, &reply_gate, Notify {
        kind: KIND_READY,
        src: LABEL_RELAY as u64,
        dst: 0,
        tag: 0,
        bytes: 0,
    });

    let slots = INBOUND.borrow();
    // outstanding[s][d]: the destination has not consumed the last notification of the pair
    let mut outstanding = [[false; MAX_RANKS]; MAX_RANKS];
    // forwarded transfers whose notification waits for the pair to become free (in order)
    let mut pending: Vec<Notify> = Vec::new();
    let mut forwarded = 0u64;
    loop {
        relay_drain(&reply_gates, &mut outstanding);
        // deliver pending notifications in order, at most one per pair
        let mut i = 0;
        while i < pending.len() {
            let n = pending[i];
            let (s, d) = (n.src as usize, n.dst as usize);
            if outstanding[s][d] {
                i += 1;
                continue;
            }
            let mut msg = MsgBuf::new();
            msg.set(n);
            match sgates[d].send(&msg, &reply_gates[d]) {
                Ok(_) => {
                    outstanding[s][d] = true;
                    pending.remove(i);
                },
                // cannot happen with one credit per source, but keep the pair order if it does
                Err(e) if e.code() == Code::NoCredits => break,
                Err(e) => panic!("relay send failed: {:?}", e),
            }
        }
        let msg = if pending.is_empty() {
            wv_assert_ok!(rgate.receive(None))
        }
        else {
            match rgate.fetch() {
                Some(m) => m,
                None => continue,
            }
        };
        let w = msg.as_words();
        if w[0] == KIND_STOP {
            done(&rgate, msg);
            break;
        }
        if w[0] != KIND_DATA {
            done(&rgate, msg);
            continue;
        }
        let n = Notify {
            kind: KIND_DATA,
            src: w[1],
            dst: w[2],
            tag: w[3],
            bytes: w[4],
        };
        let (s, d) = (n.src as usize, n.dst as usize);
        let len = cmp::min(n.bytes as usize, SLOT);
        // second hop: from our slot for the source into the destination's slot for the source;
        // then the source's slot is free again
        wv_assert_ok!(mgates[d].write(&slots[s * SLOT..s * SLOT + len], (s * SLOT) as u64));
        done(&rgate, msg);
        forwarded += 1;
        // the notification: now, unless the pair is busy or has earlier pending transfers
        pending.push(n);
    }
    assert!(pending.is_empty(), "relay: {} transfers not delivered", pending.len());
    send_struct_retry(&to_coord, &reply_gate, Stats {
        kind: KIND_STATS,
        rank: LABEL_RELAY as u64,
        total: 0,
        comp: 0,
        send: 0,
        recv: 0,
        ops: forwarded,
        bytes: 0,
    });
    0
}

// ------------------------------------------------------------------------------------------
// coordinator
// ------------------------------------------------------------------------------------------

// delegates the gate `sel` to the running member `act` at its next channel selector
fn delegate_chan(act: &RunningProgramActivity, next: &mut kif::CapSel, sel: kif::CapSel) -> u32 {
    let dst = *next;
    *next += 1;
    wv_assert_ok!(act.activity().delegate_to(CapRngDesc::new(CapType::OBJECT, sel, 1), dst));
    dst as u32
}

fn coord_main(name: &str, ranks: usize, mode: &str, inst: usize, groups: usize) -> i32 {
    assert!(ranks >= 2 && groups >= 1 && ranks * groups <= MAX_RANKS);
    let t_begin = CycleInstant::now();
    let host = mode == "host";
    let total = ranks * groups; // ranks of all groups, numbered globally
    let members = if host { total + 1 } else { total }; // + relay
    let relay = total; // index of the relay
    let group_of = |m: usize| m / ranks;

    // the programs, and the (source, destination) pairs they use: channels are created for
    // those only (a HAL creates the channels the DFG needs, not all pairs)
    let mut progs: Vec<Vec<u8>> = Vec::new();
    let mut used = [[false; MAX_RANKS]; MAX_RANKS];
    for m in 0..total {
        let prog = read_program(name, m % ranks);
        for op in parse_program(&prog) {
            if op.op == OP_SEND {
                used[m][group_of(m) * ranks + op.peer as usize] = true;
            }
        }
        progs.push(prog);
    }

    let mut own_rgate = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(RG_ORDER).msg_order(RG_MSG_ORDER)
    ));
    wv_assert_ok!(own_rgate.activate());
    let reply_gate = new_reply_gate(11);

    // receive gates of all members, send gates member -> coordinator
    let mut rgates: Vec<RecvGate> = Vec::new();
    let mut to_coord: Vec<SendGate> = Vec::new();
    for m in 0..members {
        rgates.push(wv_assert_ok!(RecvGate::new_with(
            RGateArgs::default().order(RG_ORDER).msg_order(RG_MSG_ORDER)
        )));
        to_coord.push(wv_assert_ok!(SendGate::new_with(
            SGateArgs::new(&own_rgate).credits(1).label(m as u32)
        )));
    }
    // coordinator -> member (start message)
    let mut go_gates: Vec<SendGate> = Vec::new();
    for m in 0..members {
        go_gates.push(wv_assert_ok!(SendGate::new_with(
            SGateArgs::new(&rgates[m]).credits(1).label(LABEL_COORD)
        )));
    }
    // data notification channels: direct: rank -> rank; host: rank -> relay, relay -> rank
    let mut notify: Vec<Vec<Option<SendGate>>> = Vec::new();
    for s in 0..members {
        let mut row = Vec::new();
        for d in 0..members {
            let create = if host {
                (s < total && d == relay) || (s == relay && d < total)
            }
            else {
                s != d && group_of(s) == group_of(d) && used[s][d]
            };
            row.push(if create {
                // the relay's channel to a rank carries the transfers of all sources
                let (label, credits) = if s == relay { (LABEL_RELAY, total as u32) } else { (s as u32, 1) };
                Some(wv_assert_ok!(SendGate::new_with(
                    SGateArgs::new(&rgates[d]).credits(credits).label(label)
                )))
            }
            else {
                None
            });
        }
        notify.push(row);
    }

    // start the members
    let mut acts: Vec<RunningProgramActivity> = Vec::new();
    for m in 0..members {
        let tile = wv_assert_ok!(Tile::get("imem+riscv"));
        let aname = if m == relay && host { format!("relay") } else { format!("rank{}", m) };
        let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new(&aname)));
        wv_assert_ok!(act.delegate_obj(rgates[m].sel()));
        wv_assert_ok!(act.delegate_obj(to_coord[m].sel()));
        let mut sink = act.data_sink();
        sink.push(m as u64);
        sink.push(name);
        sink.push(rgates[m].sel());
        sink.push(to_coord[m].sel());
        let run = if m == relay && host {
            wv_assert_ok!(act.run(relay_main))
        }
        else {
            wv_assert_ok!(act.run(rank_main))
        };
        acts.push(run);
    }
    // setup time, part 1: tiles, activities, programs loaded
    let t_started = CycleInstant::now();

    // collect the buffer addresses
    let mut inbound = [0u64; MAX_RANKS + 1];
    let mut got = 0;
    while got < members {
        let msg = wv_assert_ok!(own_rgate.receive(None));
        let w = msg.as_words();
        assert!(w[0] == KIND_ADDR);
        let m = if w[1] == LABEL_RELAY as u64 { relay } else { w[1] as usize };
        inbound[m] = w[2];
        done(&own_rgate, msg);
        got += 1;
    }
    // the ranks' programs go to the start of their inbound buffers
    let mut prog_lens = [0u64; MAX_RANKS + 1];
    for m in 0..total {
        let prog = &progs[m];
        assert!(prog.len() <= SLOT, "program of rank {} too large", m);
        let mg = wv_assert_ok!(acts[m].activity().get_mem(inbound[m], SLOT as u64, kif::Perm::W));
        wv_assert_ok!(mg.write(prog, 0));
        prog_lens[m] = prog.len() as u64;
    }
    // setup time, part 2 starts here: all members are running and have their programs
    let t_loaded = CycleInstant::now();

    // memory channels and start messages
    let mut gos: Vec<Go> = Vec::new();
    for m in 0..members {
        gos.push(Go {
            kind: KIND_GO,
            // ranks see their group (local peers), the relay all ranks (global)
            ranks: if m == relay && host { total as u64 } else { ranks as u64 },
            mode: if host { MODE_HOST } else { MODE_DIRECT },
            mgates: [0; MAX_RANKS],
            sgates: [0; MAX_RANKS],
            prog_len: prog_lens[m],
            base: if m == relay && host { 0 } else { (group_of(m) * ranks) as u64 },
        });
    }
    // the selectors in the Go messages are the members' (see CHAN_SEL)
    let mut next_sel = [CHAN_SEL; MAX_RANKS + 1];
    let mut mem_channels: Vec<MemGate> = Vec::new();
    if !host {
        for s in 0..total {
            for d in 0..total {
                if s == d || group_of(s) != group_of(d) || !used[s][d] {
                    continue;
                }
                // slot for source s in d's inbound area, writable by s; the Go message indexes
                // the channels by the group-local peer
                let mg = wv_assert_ok!(acts[d].activity().get_mem(
                    inbound[d] + (s * SLOT) as u64,
                    SLOT as u64,
                    kif::Perm::W
                ));
                let dl = d % ranks;
                gos[s].mgates[dl] = delegate_chan(&acts[s], &mut next_sel[s], mg.sel());
                gos[s].sgates[dl] =
                    delegate_chan(&acts[s], &mut next_sel[s], notify[s][d].as_ref().unwrap().sel());
                mem_channels.push(mg);
            }
        }
    }
    else {
        for s in 0..total {
            // s -> relay slot s
            let mg = wv_assert_ok!(acts[relay].activity().get_mem(
                inbound[relay] + (s * SLOT) as u64,
                SLOT as u64,
                kif::Perm::W
            ));
            gos[s].mgates[0] = delegate_chan(&acts[s], &mut next_sel[s], mg.sel());
            gos[s].sgates[0] =
                delegate_chan(&acts[s], &mut next_sel[s], notify[s][relay].as_ref().unwrap().sel());
            mem_channels.push(mg);
            // relay -> whole inbound area of s (slots of all global sources)
            let mg = wv_assert_ok!(acts[s].activity().get_mem(
                inbound[s],
                (total * SLOT) as u64,
                kif::Perm::W
            ));
            gos[relay].mgates[s] = delegate_chan(&acts[relay], &mut next_sel[relay], mg.sel());
            gos[relay].sgates[s] = delegate_chan(
                &acts[relay],
                &mut next_sel[relay],
                notify[relay][s].as_ref().unwrap().sel()
            );
            mem_channels.push(mg);
        }
    }

    for m in 0..members {
        send_struct_retry(&go_gates[m], &reply_gate, gos[m]);
    }
    // barrier: all members have set up their channels
    let mut got = 0;
    while got < members {
        let msg = wv_assert_ok!(own_rgate.receive(None));
        let w = msg.as_words();
        assert!(w[0] == KIND_READY);
        done(&own_rgate, msg);
        got += 1;
    }
    let t0 = CycleInstant::now();
    // setup time: activities = tiles allocated, activities created, programs loaded (until all
    // members reported in); channels = memory/send gates created, delegated and activated
    let t_activities = t_loaded.duration_since(t_begin).as_raw();
    let t_channels = t0.duration_since(t_loaded).as_raw();
    let _ = t_started;
    for m in 0..total {
        send_struct_retry(&go_gates[m], &reply_gate, Notify {
            kind: KIND_START,
            src: 0,
            dst: 0,
            tag: 0,
            bytes: 0,
        });
    }

    // results
    let mut stats: Vec<Stats> = Vec::new();
    let mut got = 0;
    while got < total {
        let msg = wv_assert_ok!(own_rgate.receive(None));
        let w = msg.as_words();
        if w[0] == KIND_STATS {
            stats.push(words_to(w));
            got += 1;
        }
        done(&own_rgate, msg);
    }
    let wall = CycleInstant::now().duration_since(t0).as_raw();
    if host {
        send_struct_retry(&go_gates[relay], &reply_gate, Notify {
            kind: KIND_STOP,
            src: 0,
            dst: 0,
            tag: 0,
            bytes: 0,
        });
        let msg = wv_assert_ok!(own_rgate.receive(None));
        let w = msg.as_words();
        if w[0] == KIND_STATS {
            println!("relay: forwarded {} transfers", w[6]);
        }
        done(&own_rgate, msg);
    }

    // concurrent instances share the console: print one after the other
    busy_wait(inst as u64 * 20_000_000);
    println!(
        "setup {} ranks={} mode={} inst={}: activities {} channels {} cycles",
        name, ranks, mode, inst, t_activities, t_channels
    );
    // one result line per group (inst numbers groups when there are several)
    stats.sort_by_key(|s| s.rank);
    for g in 0..groups {
        let mut max_total = 0;
        for s in stats.iter().filter(|s| group_of(s.rank as usize) == g) {
            println!(
                "rank {}: total {} comp {} send {} recv {} ops {} bytes {}",
                s.rank, s.total, s.comp, s.send, s.recv, s.ops, s.bytes
            );
            max_total = cmp::max(max_total, s.total);
        }
        println!(
            "replay {} ranks={} mode={} inst={}: total: {} cycles (wall {} cycles)",
            name, ranks, mode, if groups > 1 { g } else { inst }, max_total, wall
        );
    }

    for m in 0..total {
        send_struct_retry(&go_gates[m], &reply_gate, Notify {
            kind: KIND_STOP,
            src: 0,
            dst: 0,
            tag: 0,
            bytes: 0,
        });
    }
    for a in acts {
        wv_assert_ok!(a.wait());
    }
    0
}

#[no_mangle]
pub fn main() -> i32 {
    let args: Vec<&str> = env::args().collect();
    if args.len() < 5 || args[1] != "coord" {
        println!("Usage: {} coord <name> <ranks> <native|ironbus|host> [inst]", args[0]);
        return 1;
    }
    let ranks: usize = args[3].parse().expect("ranks");
    let inst: usize = if args.len() > 5 { args[5].parse().expect("inst") } else { 0 };
    let groups: usize = if args.len() > 6 { args[6].parse().expect("groups") } else { 1 };
    coord_main(args[2], ranks, args[4], inst, groups)
}
