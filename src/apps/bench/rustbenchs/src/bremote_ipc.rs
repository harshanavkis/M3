/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use m3::cap::Selector;
use m3::com::{recv_msg, RecvGate, SGateArgs, SendGate};
use m3::rc::Rc;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::time::{CycleInstant, Duration, Profiler, TimeDuration};
use m3::vec::Vec;
use m3::{
    format, println, reply_vmsg, send_vmsg, wv_assert_eq, wv_assert_ok, wv_perf, wv_run_test,
};

const MSG_ORD: u32 = 11;

const WARMUP: u64 = 50;
const RUNS: u64 = 1000;

pub fn run(t: &mut dyn WvTester) {
    // Numbers at the end denote message size in bytes
    wv_run_test!(t, pingpong_remote8);
    wv_run_test!(t, pingpong_remote16);
    wv_run_test!(t, pingpong_remote32);
    wv_run_test!(t, pingpong_remote64);
    wv_run_test!(t, pingpong_remote128);
    wv_run_test!(t, pingpong_remote256);
    wv_run_test!(t, pingpong_remote512);
    wv_run_test!(t, pingpong_remote1024);
    // wv_run_test!(t, pingpong_remote2048);
}

fn pingpong_remote8(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 8);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 64),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(&sgate, reply_gate, 0u64));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote16(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 16);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 128),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(&sgate, reply_gate, 0u64, 0u64));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote32(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 32);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 256),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(&sgate, reply_gate, 0u64, 0u64, 0u64, 0u64));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote64(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 64);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 512),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(
                &sgate, reply_gate, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64
            ));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote128(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 128);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 1024),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(
                &sgate, reply_gate, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64
            ));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote256(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 256);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 2048),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(
                &sgate, reply_gate, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64
            ));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote512(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 512);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 4096),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(
                &sgate, reply_gate, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64
            ));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote1024(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 1024);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 8192),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(
                &sgate, reply_gate, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64
            ));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn pingpong_remote2048(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone"));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, MSG_ORD, MSG_ORD);
        wv_assert_ok!(rgate.activate());
        for i in 0..RUNS + WARMUP {
            let mut msg = wv_assert_ok!(recv_msg(&rgate));
            wv_assert_eq!(t, msg.size(), 2048);
            wv_assert_ok!(reply_vmsg!(msg, 0u64));
        }
        0
    }));

    let mut prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

    let reply_gate = RecvGate::def();
    wv_perf!(
        format!("{} pingpong with (1 * {}) msgs", "remote", 16384),
        prof.run::<CycleInstant, _>(|| {
            wv_assert_ok!(send_vmsg!(
                &sgate, reply_gate, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64,
                0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64
            ));

            let mut reply = wv_assert_ok!(recv_msg(reply_gate));
            wv_assert_eq!(t, reply.pop::<u64>(), Ok(0));
        })
    );

    wv_assert_eq!(t, act.wait(), Ok(0));
}
