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
use m3::com::{recv_msg, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::{println, reply_vmsg, send_vmsg, wv_assert_ok};

const TILE_HASH: [u32; 8] = [0 as u32; 8];

#[no_mangle]
pub fn main() -> i32 {
    // Identity based; exclusive access;
    // let tile = wv_assert_ok!(Tile::get("core"));

    // Secure data processing across a set of heterogeneous nodes

    let tile = match Tile::get_with_props("core", &TILE_HASH, false) {
        Ok(t) => t,
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

    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    let rgate = wv_assert_ok!(RecvGate::new_secure(8, 8));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.secure_run(|| {
        let rgate_sel = Activity::own().data_source().pop().unwrap();
        let mut rgate = RecvGate::new_bind(rgate_sel, 8, 8);

        wv_assert_ok!(rgate.activate());
        let mut msg = wv_assert_ok!(recv_msg(&rgate));
        println!("Child: {}", msg.pop::<String>().unwrap());
        wv_assert_ok!(reply_vmsg!(msg, "World"));

        0
    }));

    let reply_gate = RecvGate::def();
    wv_assert_ok!(send_vmsg!(&sgate, reply_gate, "Hello"));
    let mut reply = wv_assert_ok!(recv_msg(reply_gate));

    println!("Parent: {}", reply.pop::<String>().unwrap());

    act.wait().unwrap();

    0
}
