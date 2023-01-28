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

use m3::col::{String, ToString, Vec};
use m3::com::{recv_msg, GateIStream, MemGate, RGateArgs, RecvGate, SendGate};
use m3::errors::Error;
use m3::kif::Perm;
use m3::math::next_log2;
use m3::mem::{size_of, MsgBuf};
use m3::tcu;
use m3::time::{CycleDuration, CycleInstant, Duration};
use m3::{env, reply_vmsg};
use m3::{log, println, vec};
use core::cmp;
use m3::cell::StaticRefCell;
use m3::mem::AlignedBuf;
use m3::io::{Read, Write};
use m3::wv_assert_ok;
use m3::vfs::{OpenFlags, VFS};

const LOG_MSGS: bool = true;
const LOG_MEM: bool = true;
const LOG_COMP: bool = true;

static BUF: StaticRefCell<AlignedBuf<4096>> = StaticRefCell::new(AlignedBuf::new_zeroed());

fn create_reply_gate(ctrl_msg_size: usize) -> Result<RecvGate, Error> {
    RecvGate::new_with(
        RGateArgs::default()
            .order(next_log2(ctrl_msg_size + size_of::<tcu::Header>()))
            .msg_order(next_log2(ctrl_msg_size + size_of::<tcu::Header>())),
    )
}

struct Node {
    name: String,
    ctrl_msg: MsgBuf,
    data_buf: Vec<u8>,
}

impl Node {
    fn new(name: String, ctrl_msg_size: usize, data_size: usize) -> Self {
        let mut ctrl_msg = MsgBuf::new();
        ctrl_msg.set(vec![0u8; ctrl_msg_size]);
        let data_buf = vec![0u8; data_size];
        Self {
            name,
            ctrl_msg,
            data_buf,
        }
    }

    fn compute_for(&self, duration: CycleDuration) {
        log!(LOG_COMP, "{}: computing for {:?}", self.name, duration);

        let end = CycleInstant::now().as_cycles() + duration.as_raw();
        while CycleInstant::now().as_cycles() < end {}
    }

    fn receive_request<'r>(
        &self,
        src: &str,
        rgate: &'r RecvGate,
    ) -> Result<GateIStream<'r>, Error> {
        let request = recv_msg(rgate)?;
        log!(LOG_MSGS, "{} <- {}", self.name, src);
        Ok(request)
    }

    fn send_reply(&self, dest: &str, request: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LOG_MSGS, "{} -> {}", self.name, dest);
        request.reply(&self.ctrl_msg)
    }

    fn call_and_ack(
        &self,
        dest: &str,
        sgate: &SendGate,
        reply_gate: &RecvGate,
    ) -> Result<(), Error> {
        log!(LOG_MSGS, "{} -> {}", self.name, dest);
        let reply = sgate.call(&self.ctrl_msg, reply_gate)?;
        log!(LOG_MSGS, "{} <- {}", self.name, dest);
        reply_gate.ack_msg(reply)
    }

    fn write_to(&self, dest: &str, mgate: &MemGate, data_size: usize) -> Result<(), Error> {
        log!(LOG_MEM, "{}: writing to {}", self.name, dest);
        mgate.write(&self.data_buf[0..data_size], 0)
    }
}

fn client(args: &[&str]) {
    if args.len() != 5 {
        panic!("Usage: {} <ctrl-msg-size> <data-size> <runs>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let data_size = args[3]
        .parse::<usize>()
        .expect("Unable to parse compute time");
    let runs = args[4]
        .parse::<u64>()
        .expect("Unable to parse number of runs");

    let node = Node::new("client".to_string(), ctrl_msg_size, data_size);

    let mut reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");
    reply_gate.activate().unwrap();
    let mut sgate = SendGate::new_named("gpu").expect("Unable to create named SendGate req");

    let mem_gate = if data_size > 0 {
        Some(MemGate::new(data_size, Perm::W).expect("Unable to create memory gate"))
    }
    else {
        None
    };

    let mut gpu_rgate = RecvGate::new_named("gpures").expect("Unable to create named RecvGate gpures");
    gpu_rgate.activate().unwrap();

    for _ in 0..runs {
        let start = CycleInstant::now();

        if let Some(ref mg) = mem_gate {
            let start = CycleInstant::now();
            node.write_to("gpu", mg, data_size)
                .expect("Writing data failed");
            let duration = CycleInstant::now().duration_since(start);
            println!("xfer: {:?}", duration);
        }

        node.call_and_ack("gpu", &sgate, &reply_gate)
            .expect("Request failed");

        let mut gpu_res = node
            .receive_request("gpu", &gpu_rgate)
            .expect("Receiving GPU result failed");
        reply_vmsg!(gpu_res, 0).expect("Reply to GPU failed");

        let duration = CycleInstant::now().duration_since(start);
        // compensate for running on a 100MHz core (in contrast to the computing computes that run
        // on a 80MHz core).
        // let duration = ((duration.as_raw() as f64) * 0.8) as u64;
        println!("total: {:?}", duration);
    }
}

fn gpu(args: &[&str]) {
    if args.len() != 6 {
        panic!("Usage: {} <ctrl-msg-size> <compute-millis> <read-in-size> <write-out-size>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let compute_time = args[3]
        .parse::<u64>()
        .expect("Unable to parse compute time");
    let read_in_size = args[4]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let write_out_size = args[5]
        .parse::<usize>()
        .expect("Unable to parse control message size");

    let node = Node::new("gpu".to_string(), ctrl_msg_size, cmp::max(read_in_size, write_out_size));

    let buf = &mut BUF.borrow_mut()[..];

    let res_sgate = SendGate::new_named("gpures").expect("Unable to create named SendGate gpures");

    let mut reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    reply_gate.activate().unwrap();

    let mut req_rgate = RecvGate::new_named("gpu").expect("Unable to create named RecvGate gpu");
    req_rgate.activate().unwrap();
    loop {
        let mut request = node
            .receive_request("client", &req_rgate)
            .expect("Receiving request failed");
        reply_vmsg!(request, 0).expect("Reply to client failed");

        if read_in_size != 0 {
            let mut file = wv_assert_ok!(VFS::open("/data/4096k.txt", OpenFlags::R));
            let mut amount = 0;

            while read_in_size > amount {
                // TODO: Change this to exact amount
                let read_amount = wv_assert_ok!(file.read(buf));

                amount += read_amount;
            }
        }

        node.compute_for(CycleDuration::from_raw(compute_time));

        if write_out_size != 0 {
            let mut file = wv_assert_ok!(VFS::open("/results", OpenFlags::W | OpenFlags::CREATE | OpenFlags::TRUNC));
            let mut amount = 0;

            // loop {
                // TODO: Change this to exact amount
                let write_amount = wv_assert_ok!(file.write(buf));

                if write_amount == 0 {
                    break;
                }
            // }
        }

        node.call_and_ack("client", &res_sgate, &reply_gate)
            .expect("GPU-result send failed");
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let args: Vec<&str> = env::args().collect();

    match args[1] {
        "client" => client(&args),
        "gpu" => gpu(&args),
        s => panic!("unexpected component {}", s),
    }

    Ok(())
}
