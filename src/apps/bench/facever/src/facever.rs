/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#![no_std]

use core::cmp;
use m3::cell::StaticRefCell;
use m3::col::{String, ToString, Vec};
use m3::com::{recv_msg, GateIStream, MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::dataflow::{AppContext, CtxState, Flags, Session, SessionArgs};
use m3::errors::{Code, Error};
use m3::io::{self, Read, Write};
use m3::kif;
use m3::kif::Perm;
use m3::math::next_log2;
use m3::mem::AlignedBuf;
use m3::mem::{size_of, MsgBuf};
use m3::session::Pipes;
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::time::{CycleDuration, CycleInstant, Duration, Instant};
use m3::vfs::{IndirectPipe, OpenFlags, VFS};
use m3::wv_assert_ok;
use m3::{env, reply_vmsg};
use m3::{log, println, vec};
use m3::{send_vmsg, tcu};

const LOG_MSGS: bool = true;
const LOG_MEM: bool = true;
const LOG_COMP: bool = true;
const LOG_FILE: bool = true;

fn tpu_ml_dataflow_api() {
    let ctx_fn = || {
        let mut ctx_state = CtxState::new();

        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        ctx_state.recv_from(rgate_sel);

        // Read in image data from client: 256 images * (28*28*1) bytes per image
        let mut data_buf: [u8; 2048] = [0; 2048];
        let mut total = 0;
        while total < 0x31000 {
            ctx_state.read_from(shmem_sel, &mut data_buf, total);
            total += data_buf.len() as u64;
        }

        // Perform computation
        log!(LOG_COMP, "Starting TPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(13437141).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 1 byte per result, 0 or 1 depending on whether the face matches
        // the LBP histogram read from the LBP file
        let mut file = wv_assert_ok!(VFS::open(
            "/results-tpu-single",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 0x4000 {
            total += wv_assert_ok!(file.write(&data_buf));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Notify main application session about completion of task
        ctx_state.reply_to(rgate_sel, "World".as_bytes());

        0
    };

    let mut app_session = Session::new();

    let mut ctx1 = AppContext::new("core".to_string(), TILE_HASH, false, ctx_fn);

    let ctx1_sel = match app_session.insert(ctx1) {
        Ok(sel) => sel,
        Err(e) => match e.code() {
            Code::IdentityMatchFail => {
                panic!("Failed to find tile with specified identity")
            },
            Code::ExclusiveAccessFail => {
                panic!("Failed to find exclusive access to tile with specified identity")
            },
            _ => panic!(),
        },
    };

    // shared memory channel
    app_session.connect_to(ctx1_sel, Flags::R | Flags::W, SessionArgs::new(0x31000));

    // Message passing channel
    app_session.connect_to(ctx1_sel, Flags::S | Flags::G, SessionArgs::default());

    app_session.run();

    let start_compute = CycleInstant::now();
    app_session.send_to(ctx1_sel, "Hello".as_bytes()).unwrap();
    app_session.recv_from(ctx1_sel);

    app_session.wait();

    println!(
        "total: {:?}",
        CycleInstant::now().duration_since(start_compute)
    );
}

fn tpu_ml() {
    let tile = wv_assert_ok!(Tile::get("core"));

    let sh_mem = wv_assert_ok!(MemGate::new(0x31000, kif::Perm::RW));

    let rgate = wv_assert_ok!(RecvGate::new(8, 8));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("reader")));

    wv_assert_ok!(act.delegate_obj(sh_mem.sel()));
    wv_assert_ok!(act.delegate_obj(rgate.sel()));
    let mut dst = act.data_sink();
    dst.push(sh_mem.sel());
    dst.push(rgate.sel());

    act.add_mount("/", "/");

    let act = wv_assert_ok!(act.run(|| {
        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        let mut shmem_gate = MemGate::new_bind(shmem_sel);
        let mut rgate = RecvGate::new_bind(rgate_sel, 8, 8);

        let mut data_buf: [u8; 2048] = [0; 2048];

        wv_assert_ok!(shmem_gate.activate());
        wv_assert_ok!(rgate.activate());

        let mut msg = wv_assert_ok!(recv_msg(&rgate));

        // Read in image data from client: 256 images * (28*28*1) bytes per image
        let mut total = 0;
        log!(LOG_MEM, "Starting read mgate");
        while total < 0x31000 {
            shmem_gate.read(&mut data_buf, total);
            total += data_buf.len() as u64;
        }
        log!(LOG_MEM, "Finished read mgate");

        // Perform computation
        log!(LOG_COMP, "Starting TPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(13437141).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 64 byte per result for each digit
        let mut file = wv_assert_ok!(VFS::open(
            "/results-tpu-single",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 0x4000 {
            total += wv_assert_ok!(file.write(&data_buf));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Reply to notify that the computation was completed
        wv_assert_ok!(reply_vmsg!(msg, "World"));

        0
    }));

    let reply_gate = RecvGate::def();

    let start_compute = CycleInstant::now();
    wv_assert_ok!(send_vmsg!(&sgate, reply_gate, "Hello"));

    let mut reply = wv_assert_ok!(recv_msg(reply_gate));

    act.wait().unwrap();

    println!(
        "total: {:?}",
        CycleInstant::now().duration_since(start_compute)
    );
}

fn tpu_ml_dist_dataflow_api() {
    let ctx_fn_a = || {
        let mut ctx_state = CtxState::new();

        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        ctx_state.recv_from(rgate_sel);

        // Read in image data from client: 128 images * (28*28*1) bytes per image
        let mut data_buf: [u8; 2048] = [0; 2048];
        let mut total = 0;
        while total < 0x18800 {
            ctx_state.read_from(shmem_sel, &mut data_buf, total);
            total += data_buf.len() as u64;
        }

        // Perform computation
        log!(LOG_COMP, "Starting TPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(6948680).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 1 byte per result, 0 or 1 depending on whether the face matches
        // the LBP histogram read from the LBP file
        let mut file = wv_assert_ok!(VFS::open(
            "/results-tpu-dist-a",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 0x2000 {
            total += wv_assert_ok!(file.write(&data_buf));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Notify main application session about completion of task
        ctx_state.reply_to(rgate_sel, "World".as_bytes());

        0
    };

    let ctx_fn_b = || {
        let mut ctx_state = CtxState::new();

        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        ctx_state.recv_from(rgate_sel);

        // Read in image data from client: 128 images * (28*28*1) bytes per image
        let mut data_buf: [u8; 2048] = [0; 2048];
        let mut total = 0;
        while total < 0x18800 {
            ctx_state.read_from(shmem_sel, &mut data_buf, total);
            total += data_buf.len() as u64;
        }

        // Perform computation
        log!(LOG_COMP, "Starting TPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(6948680).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 1 byte per result, 0 or 1 depending on whether the face matches
        // the LBP histogram read from the LBP file
        let mut file = wv_assert_ok!(VFS::open(
            "/results-tpu-dist-b",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 0x2000 {
            total += wv_assert_ok!(file.write(&data_buf));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Notify main application session about completion of task
        ctx_state.reply_to(rgate_sel, "World".as_bytes());

        0
    };

    let mut app_session = Session::new();

    let mut ctx1 = AppContext::new("core".to_string(), TILE_HASH, false, ctx_fn_a);
    let mut ctx2 = AppContext::new("core".to_string(), TILE_HASH, false, ctx_fn_b);

    let ctx1_sel = match app_session.insert(ctx1) {
        Ok(sel) => sel,
        Err(e) => match e.code() {
            Code::IdentityMatchFail => {
                panic!("Failed to find tile with specified identity")
            },
            Code::ExclusiveAccessFail => {
                panic!("Failed to find exclusive access to tile with specified identity")
            },
            _ => panic!(),
        },
    };

    let ctx2_sel = match app_session.insert(ctx2) {
        Ok(sel) => sel,
        Err(e) => match e.code() {
            Code::IdentityMatchFail => {
                panic!("Failed to find tile with specified identity")
            },
            Code::ExclusiveAccessFail => {
                panic!("Failed to find exclusive access to tile with specified identity")
            },
            _ => panic!(),
        },
    };

    // shared memory channel
    app_session.connect_to(ctx1_sel, Flags::R | Flags::W, SessionArgs::new(0x18800));

    // Message passing channel
    app_session.connect_to(ctx1_sel, Flags::S | Flags::G, SessionArgs::default());

    // shared memory channel
    app_session.connect_to(ctx2_sel, Flags::R | Flags::W, SessionArgs::new(0x18800));

    // Message passing channel
    app_session.connect_to(ctx2_sel, Flags::S | Flags::G, SessionArgs::default());

    app_session.run();

    let start_compute = CycleInstant::now();
    app_session.send_to(ctx1_sel, "Hello".as_bytes()).unwrap();
    app_session.send_to(ctx2_sel, "Hello".as_bytes()).unwrap();
    app_session.recv_from(ctx1_sel);
    app_session.recv_from(ctx2_sel);

    app_session.wait();

    println!(
        "total: {:?}",
        CycleInstant::now().duration_since(start_compute)
    );
}

fn tpu_ml_dist() {
    let tile_a = wv_assert_ok!(Tile::get("core"));

    let sh_mem_a = wv_assert_ok!(MemGate::new(0x31000, kif::Perm::RW));

    let rgate_a = wv_assert_ok!(RecvGate::new(8, 8));
    let sgate_a = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate_a).credits(1)));

    let mut act_a = wv_assert_ok!(ChildActivity::new_with(tile_a, ActivityArgs::new("reader")));

    wv_assert_ok!(act_a.delegate_obj(sh_mem_a.sel()));
    wv_assert_ok!(act_a.delegate_obj(rgate_a.sel()));
    let mut dst = act_a.data_sink();
    dst.push(sh_mem_a.sel());
    dst.push(rgate_a.sel());

    let tile_b = wv_assert_ok!(Tile::get("core"));

    let sh_mem_b = wv_assert_ok!(MemGate::new(0x31000, kif::Perm::RW));

    let rgate_b = wv_assert_ok!(RecvGate::new(8, 8));
    let sgate_b = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate_b).credits(1)));

    let mut act_b = wv_assert_ok!(ChildActivity::new_with(tile_b, ActivityArgs::new("reader")));

    wv_assert_ok!(act_b.delegate_obj(sh_mem_b.sel()));
    wv_assert_ok!(act_b.delegate_obj(rgate_b.sel()));
    let mut dst = act_b.data_sink();
    dst.push(sh_mem_b.sel());
    dst.push(rgate_b.sel());

    act_a.add_mount("/", "/");
    act_b.add_mount("/", "/");

    let act_a = wv_assert_ok!(act_a.run(|| {
        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        let mut shmem_gate = MemGate::new_bind(shmem_sel);
        let mut rgate = RecvGate::new_bind(rgate_sel, 8, 8);

        let mut data_buf: [u8; 2048] = [0; 2048];

        wv_assert_ok!(shmem_gate.activate());
        wv_assert_ok!(rgate.activate());

        let mut msg = wv_assert_ok!(recv_msg(&rgate));

        // Read in image data from client: 256 images * (28*28*1) bytes per image
        let mut total = 0;
        log!(LOG_MEM, "Starting read mgate");
        while total < 0x31000 {
            shmem_gate.read(&mut data_buf, total);
            total += data_buf.len() as u64;
        }
        log!(LOG_MEM, "Finished read mgate");

        // Perform computation
        log!(LOG_COMP, "Starting TPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(6948680).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 64 byte per result for each digit
        let mut file = wv_assert_ok!(VFS::open(
            "/results-tpu-dist-a",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 0x2000 {
            total += wv_assert_ok!(file.write(&data_buf));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Reply to notify that the computation was completed
        wv_assert_ok!(reply_vmsg!(msg, "World"));

        0
    }));

    let act_b = wv_assert_ok!(act_b.run(|| {
        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        let mut shmem_gate = MemGate::new_bind(shmem_sel);
        let mut rgate = RecvGate::new_bind(rgate_sel, 8, 8);

        let mut data_buf: [u8; 2048] = [0; 2048];

        wv_assert_ok!(shmem_gate.activate());
        wv_assert_ok!(rgate.activate());

        let mut msg = wv_assert_ok!(recv_msg(&rgate));

        // Read in image data from client: 256 images * (28*28*1) bytes per image
        let mut total = 0;
        log!(LOG_MEM, "Starting read mgate");
        while total < 0x31000 {
            shmem_gate.read(&mut data_buf, total);
            total += data_buf.len() as u64;
        }
        log!(LOG_MEM, "Finished read mgate");

        // Perform computation
        log!(LOG_COMP, "Starting TPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(6948680).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 64 byte per result for each digit
        let mut file = wv_assert_ok!(VFS::open(
            "/results-tpu-dist-b",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 0x2000 {
            total += wv_assert_ok!(file.write(&data_buf));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Reply to notify that the computation was completed
        wv_assert_ok!(reply_vmsg!(msg, "World"));

        0
    }));

    let mut reply_gate = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(7).msg_order(6)
    ));

    wv_assert_ok!(reply_gate.activate());

    let start_compute = CycleInstant::now();
    wv_assert_ok!(send_vmsg!(&sgate_a, &reply_gate, "Hello"));
    wv_assert_ok!(send_vmsg!(&sgate_b, &reply_gate, "Hello"));

    let mut reply = wv_assert_ok!(recv_msg(&reply_gate));
    let mut reply = wv_assert_ok!(recv_msg(&reply_gate));

    act_a.wait().unwrap();
    act_b.wait().unwrap();

    println!(
        "total: {:?}",
        CycleInstant::now().duration_since(start_compute)
    );
}

const CTRL_MSG: [u8; 128] = [0; 128];
const TILE_HASH: [u32; 8] = [0; 8];
const FS_HASH: [u8; 256] = [0; 256];

fn face_verif_dataflow_api() {
    let ctx_fn = || {
        let mut ctx_state = CtxState::new();

        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        ctx_state.recv_from(rgate_sel);

        // Read in face data from client: 256 images * 1024 bytes per image
        let mut data_buf: [u8; 2048] = [0; 2048];
        let mut total = 0;
        while total < 0x40000 {
            ctx_state.read_from(shmem_sel, &mut data_buf, total);
            total += data_buf.len() as u64;
        }

        // Read in the 256 LBP histograms
        let mut face_file =
            wv_assert_ok!(VFS::open("/large.txt", OpenFlags::R | OpenFlags::NEW_SESS));
        let mut total = 0;
        log!(LOG_FILE, "Start LBP hist read");
        while total < 0x10000 {
            total += wv_assert_ok!(face_file.read(&mut data_buf));
        }
        log!(LOG_FILE, "End LBP hist read");

        // Perform computation
        log!(LOG_COMP, "Starting GPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(698880).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 1 byte per result, 0 or 1 depending on whether the face matches
        // the LBP histogram read from the LBP file
        let mut file = wv_assert_ok!(VFS::open(
            "/results-gpu-face-verif",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 256 {
            total += wv_assert_ok!(file.write(&data_buf[..256]));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Notify main application session about completion of task
        ctx_state.reply_to(rgate_sel, "World".as_bytes());

        0
    };

    let mut app_session = Session::new();

    let mut ctx1 = AppContext::new("core".to_string(), TILE_HASH, false, ctx_fn);

    let ctx1_sel = match app_session.insert(ctx1) {
        Ok(sel) => sel,
        Err(e) => match e.code() {
            Code::IdentityMatchFail => {
                panic!("Failed to find tile with specified identity")
            },
            Code::ExclusiveAccessFail => {
                panic!("Failed to find exclusive access to tile with specified identity")
            },
            _ => panic!(),
        },
    };

    // shared memory channel
    app_session.connect_to(ctx1_sel, Flags::R | Flags::W, SessionArgs::new(0x40000));

    // Message passing channel
    app_session.connect_to(ctx1_sel, Flags::S | Flags::G, SessionArgs::default());

    app_session.run();

    let start_compute = CycleInstant::now();
    app_session.send_to(ctx1_sel, "Hello".as_bytes()).unwrap();
    app_session.recv_from(ctx1_sel);

    app_session.wait();

    println!(
        "total: {:?}",
        CycleInstant::now().duration_since(start_compute)
    );
}

fn face_verif() {
    let tile = wv_assert_ok!(Tile::get("core"));

    let sh_mem = wv_assert_ok!(MemGate::new(0x40000, kif::Perm::RW));

    let rgate = wv_assert_ok!(RecvGate::new(8, 8));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("reader")));

    wv_assert_ok!(act.delegate_obj(sh_mem.sel()));
    wv_assert_ok!(act.delegate_obj(rgate.sel()));
    let mut dst = act.data_sink();
    dst.push(sh_mem.sel());
    dst.push(rgate.sel());

    act.add_mount("/", "/");

    let act = wv_assert_ok!(act.run(|| {
        let mut src = Activity::own().data_source();
        let shmem_sel = src.pop().unwrap();
        let rgate_sel = src.pop().unwrap();

        let mut shmem_gate = MemGate::new_bind(shmem_sel);
        let mut rgate = RecvGate::new_bind(rgate_sel, 8, 8);

        let mut data_buf: [u8; 2048] = [0; 2048];

        wv_assert_ok!(shmem_gate.activate());
        wv_assert_ok!(rgate.activate());

        let mut msg = wv_assert_ok!(recv_msg(&rgate));

        // Read in face data from client: 256 images * 1024 bytes per image
        let mut total = 0;
        log!(LOG_MEM, "Starting read mgate");
        while total < 0x40000 {
            shmem_gate.read(&mut data_buf, total);
            total += data_buf.len() as u64;
        }
        log!(LOG_MEM, "Finished read mgate");

        // Read in the 256 LBP histograms
        let mut face_file =
            wv_assert_ok!(VFS::open("/large.txt", OpenFlags::R | OpenFlags::NEW_SESS));
        let mut total = 0;
        log!(LOG_FILE, "Start LBP hist read");
        while total < 0x10000 {
            total += wv_assert_ok!(face_file.read(&mut data_buf));
        }
        log!(LOG_FILE, "End LBP hist read");

        // Perform computation
        log!(LOG_COMP, "Starting GPU compute");
        let end = CycleInstant::now().as_cycles() + CycleDuration::from_raw(698880).as_raw();
        while CycleInstant::now().as_cycles() < end {}
        log!(LOG_COMP, "End compute");

        // Write results to a file: 1 byte per result, 0 or 1 depending on whether the face matches
        // the LBP histogram read from the LBP file
        let mut file = wv_assert_ok!(VFS::open(
            "/results-gpu-face-verif",
            OpenFlags::W | OpenFlags::CREATE | OpenFlags::NEW_SESS
        ));

        let mut total = 0;
        log!(LOG_FILE, "Writing results to file");
        while total < 256 {
            total += wv_assert_ok!(file.write(&data_buf[..256]));
        }
        log!(LOG_FILE, "Done writing results to file");

        // Reply to notify that the computation was completed
        wv_assert_ok!(reply_vmsg!(msg, "World"));

        0
    }));

    let reply_gate = RecvGate::def();

    let start_compute = CycleInstant::now();
    wv_assert_ok!(send_vmsg!(&sgate, reply_gate, "Hello"));

    let mut reply = wv_assert_ok!(recv_msg(reply_gate));

    act.wait().unwrap();

    println!(
        "total: {:?}",
        CycleInstant::now().duration_since(start_compute)
    );
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let args: Vec<&str> = env::args().collect();

    match args[1] {
        "gpu" => face_verif_dataflow_api(),
        "tpu-single" => tpu_ml_dataflow_api(),
        "tpu-dist" => tpu_ml_dist_dataflow_api(),
        s => panic!("unexpected component {}", s),
    }
    // face_verif();

    Ok(())
}
