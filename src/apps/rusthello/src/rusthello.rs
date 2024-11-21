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
use m3::policy_compiler::compile_policy;
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::time::{CycleInstant, Duration, Instant};
use m3::{println, reply_vmsg, send_vmsg, wv_assert_ok};

const TILE_HASH: [u32; 8] = [0 as u32; 8];

#[no_mangle]
pub fn main() -> i32 {
    println!("Hello World");

    let example_policy = r#"
GPU = { "vendor": "NVIDIA", "model": "A100", "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "gpu"}
TPU = { "vendor": "Google", "model": "v3", "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "tpu"}
FS   = { "version": "latest", "ca": "0x1234", "name": "m3fs", "hash": "0x1234", "loc": "DE"}

create_node:-StageIs(GPU) & HWVersionIs(GPU) & HWIsExclusive()
create_node:-StageIs(TPU) & HWVersionIs(TPU) & HWIsExclusive()
create_node:-StageIs(FS) & SWVersionIs(FS)

send:-StageIs(MAIN) & Sink(GPU) & UseEncr(True)

recv:-StageIs(GPU) & Src(FS) & MaxTime(TIMESTAMP) & DataIsValid() & ObtainProof(GPU, FS)
recv :- StageIs(GPU) & Src(MAIN) & MaxTime(TIMESTAMP) & DataIsValid()
send:-StageIs(GPU) & Sink(TPU) & UseEncr(True) & DataReuse(0)

# Stage 2 policy
recv:-StageIs(TPU) & Src(GPU) & MaxTime(TIMESTAMP) & DataIsValid()
send:-StageIs(TPU) & Sink(FS) & UseEncr(True)"#;

    let start_cycles = CycleInstant::now();
    compile_policy(example_policy);
    let end_cycles = start_cycles.elapsed();
    println!("Elapsed time: {}", end_cycles.as_raw());

    println!("Done");

    0
}
