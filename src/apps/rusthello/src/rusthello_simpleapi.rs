/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

use m3::col::String;
use m3::col::ToString;
use m3::com::{recv_msg, RecvGate, SGateArgs, SendGate};
use m3::dataflow::CtxState;
use m3::dataflow::{AppContext, Flags, Session};
use m3::errors::{Code, Error};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::{println, wv_assert_ok};

const TILE_HASH: [u32; 8] = [0 as u32; 8];

#[no_mangle]
pub fn main() -> i32 {
    // Secure data processing across a set of heterogeneous nodes

    // send_to, recv_from, reply_to, read_from, write_to
    // connect_to with operations: "S", "G", "R", "W"
    // send message, get message, read data, write data

    let ctx_fn = || {
        let mut ctx_state = CtxState::new();

        let recv_sel = Activity::own().data_source().pop().unwrap();
        let msg = ctx_state.recv_from(recv_sel);

        println!("Child");

        // Change this line to new api
        println!("Child: {}", String::from_utf8(msg.to_vec()).unwrap());
        ctx_state.reply_to(recv_sel, "World".as_bytes());

        0
    };

    // ------------------INITIALIZATION------------------
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

    // If you want RW to be non-secure use connect_to
    app_session.connect_to(ctx1_sel, Flags::S | Flags::G);
    app_session.connect_to(ctx1_sel, Flags::R | Flags::W); // sets up shared memory

    // app_session.secure_connect_src_to_sink(&ctx1, &ctx2, op);
    //----------------------------------------------------

    //------------------OFFLOAD---------------------------
    app_session.run();

    // send first message
    wv_assert_ok!(app_session.send_to(ctx1_sel, "Hello".as_bytes()));
    let mut reply = app_session.recv_from(ctx1_sel);
    println!("Parent: {}", String::from_utf8(reply.to_vec()).unwrap());
    //----------------------------------------------------

    //-------------------CLEANUP--------------------------
    app_session.wait();
    //----------------------------------------------------

    0
}
