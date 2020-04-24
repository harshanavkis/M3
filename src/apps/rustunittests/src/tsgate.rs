/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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

use m3::col::String;
use m3::com::{recv_msg, recv_reply, RecvGate, SGateArgs, SendGate};
use m3::errors::Code;
use m3::math;
use m3::test;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, create);
    wv_run_test!(t, send_errors);
    wv_run_test!(t, send_recv);
    wv_run_test!(t, send_reply);
}

fn create() {
    let rgate = wv_assert_ok!(RecvGate::new(math::next_log2(512), math::next_log2(256)));
    wv_assert_err!(
        SendGate::new_with(SGateArgs::new(&rgate).sel(1)),
        Code::InvArgs
    );
}

fn send_errors() {
    let mut rgate = wv_assert_ok!(RecvGate::new(math::next_log2(256), math::next_log2(256)));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).label(0x1234)));
    wv_assert_ok!(rgate.activate());

    {
        wv_assert_ok!(send_vmsg!(&sgate, &rgate, 1, 2));

        let mut is = wv_assert_ok!(rgate.receive(Some(&sgate)));
        wv_assert_eq!(is.pop(), Ok(1));
        wv_assert_eq!(is.pop(), Ok(2));
        wv_assert_err!(is.pop::<u32>(), Code::InvArgs);
    }

    {
        wv_assert_ok!(send_vmsg!(&sgate, &rgate, 4));

        let mut is = wv_assert_ok!(rgate.receive(Some(&sgate)));
        wv_assert_err!(is.pop::<String>(), Code::InvArgs);
    }

    {
        wv_assert_ok!(send_vmsg!(&sgate, &rgate, 0, "123"));

        let mut is = wv_assert_ok!(rgate.receive(Some(&sgate)));
        wv_assert_err!(is.pop::<String>(), Code::InvArgs);
    }
}

fn send_recv() {
    let mut rgate = wv_assert_ok!(RecvGate::new(math::next_log2(512), math::next_log2(256)));
    let sgate = wv_assert_ok!(SendGate::new_with(
        SGateArgs::new(&rgate).credits(2).label(0x1234)
    ));
    wv_assert_ok!(rgate.activate());

    let data = [0u8; 16];
    wv_assert_ok!(sgate.send(&data, RecvGate::def()));
    wv_assert_ok!(sgate.send(&data, RecvGate::def()));
    wv_assert_err!(sgate.send(&data, RecvGate::def()), Code::MissCredits);

    {
        let is = wv_assert_ok!(rgate.receive(Some(&sgate)));
        wv_assert_eq!(is.label(), 0x1234);
    }

    {
        let is = wv_assert_ok!(rgate.receive(Some(&sgate)));
        wv_assert_eq!(is.label(), 0x1234);
    }
}

fn send_reply() {
    let reply_gate = RecvGate::def();
    let mut rgate = wv_assert_ok!(RecvGate::new(math::next_log2(64), math::next_log2(64)));
    let sgate = wv_assert_ok!(SendGate::new_with(
        SGateArgs::new(&rgate).credits(1).label(0x1234)
    ));
    wv_assert_ok!(rgate.activate());

    wv_assert_ok!(send_vmsg!(&sgate, &reply_gate, 0x123, 12, "test"));

    // sgate -> rgate
    {
        let mut msg = wv_assert_ok!(recv_msg(&rgate));
        let (i1, i2, s): (i32, i32, String) = (
            wv_assert_ok!(msg.pop()),
            wv_assert_ok!(msg.pop()),
            wv_assert_ok!(msg.pop()),
        );
        wv_assert_eq!(i1, 0x123);
        wv_assert_eq!(i2, 12);
        wv_assert_eq!(s, "test");

        wv_assert_ok!(reply_vmsg!(msg, 44, 3));
    }

    // rgate -> reply_gate
    {
        let mut reply = wv_assert_ok!(recv_reply(&reply_gate, Some(&sgate)));
        let (i1, i2): (i32, i32) = (wv_assert_ok!(reply.pop()), wv_assert_ok!(reply.pop()));
        wv_assert_eq!(i1, 44);
        wv_assert_eq!(i2, 3);
    }
}
