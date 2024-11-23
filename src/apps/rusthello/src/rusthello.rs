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

static m3fs_read: &str = r#"
FS   = { "version": "latest", "ca": "0x1234", "name": "m3fs", "hash": "0x1234", "loc": "DE"}

dfg_node:-SWVersionIs(FS)

dfg_edge:-Src(MAIN) & Sink(FS)
dfg_edge:-Src(FS) & Sink(MAIN)
"#;

static m3fs_write: &str = r#"
FS   = { "version": "latest", "ca": "0x1234", "name": "m3fs", "hash": "0x1234", "loc": "DE"}

dfg_node:-SWVersionIs(FS)

dfg_edge:-Src(MAIN) & Sink(FS)
dfg_edge:-Src(FS) & Sink(MAIN)
"#;

static imgproc: &str = r#"
FFT = { "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "fft"}
MUL = { "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "mul"}
IFFT = { "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "ifft"}
FS   = { "version": "latest", "ca": "0x1234", "name": "m3fs", "hash": "0x1234", "loc": "DE"}

dfg_node:-HWVersionIs(FFT) & HWIsExclusive()
dfg_node:-HWVersionIs(MUL) & HWIsExclusive()
dfg_node:-HWVersionIs(IFFT) & HWIsExclusive()
dfg_node:-SWVersionIs(FS)

dfg_edge:-Src(MAIN) & Sink(FFT)
dfg_edge:-Src(MAIN) & Sink(FS)
dfg_edge:-Src(FS) & Sink(MAIN)
dfg_edge:-Src(FFT) & Sink(MUL)
dfg_edge:-Src(MUL) & Sink(IFFT)
dfg_edge:-Src(IFFT) & Sink(FS)
dfg_edge:-Src(FS) & Sink(IFFT)


send:-StageIs(MAIN) & Sink(FFT) & UseEncr(True)
recv:-StageIs(MAIN) & Src(FS) & & MaxTime(TIMESTAMP) & DataIsValid() & ObtainProof(MAIN, FS)

recv:-StageIs(FFT) & Src(MAIN) & MaxTime(TIMESTAMP) & DataIsValid()
send:-StageIs(FFT) & Sink(MUL) & UseEncr(True)

recv:-StageIs(MUL) & Src(FFT) & MaxTime(TIMESTAMP) & DataIsValid()
send:-StageIs(MUL) & Sink(IFFT) & UseEncr(True)

recv:-StageIs(IFFT) & Src(MUL) & MaxTime(TIMESTAMP) & DataIsValid()
send:-StageIs(IFFT) & Sink(FS) & UseEncr(True)
"#;

static facever: &str = r#"
GPU = { "vendor": "NVIDIA", "model": "A100", "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "gpu"}
FS   = { "version": "latest", "ca": "0x1234", "name": "m3fs", "hash": "0x1234", "loc": "DE"}

dfg_node:-HWVersionIs(GPU) & HWIsExclusive()
dfg_node:-SWVersionIs(FS)

dfg_edge:-Src(MAIN) & Sink(GPU)
df_edge:-Src(GPU) & Sink(FS)
df_edge:-Src(FS) & Sink(GPU)
"#;

static systolic: &str = r#"
GPU = {"vendor":"NVIDIA", "model":"A100", "ca":0x1234, "loc":"DE", "hash": "0x1234", "name": "gpu"}
TPU = {"vendor":"Google", "model":"v3",   "ca":0x1234, "loc":"DE", "hash": "0x1234", "name": "tpu"}
FS  = {"version":"latest", "ca":0x1234, "name":"m3fs", "loc":"DE", "hash": "0x1234"}

dfg_node:-HWVersionIs(GPU) & HWIsExclusive()
dfg_node:-HWVersionIs(TPU) & HWIsExclusive()
dfg_node:-SWVersionIs(FS)

dfg_edge:-Src(MAIN) & Sink(GPU)
dfg_edge:-Src(TPU) & Sink(MAIN)
dfg_edge:-Src(GPU) & Sink(FS)
dfg_edge:-Src(FS) & Sink(GPU)
dfg_edge:-Src(GPU) & Sink(TPU)
dfg_edge:-Src(TPU) & Sink(FS)
dfg_edge:-Src(FS) & Sink(TPU)"#;

static dist: &str = r#"
GPU = { "vendor": "NVIDIA", "model": "A100", "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "gpu"}
TPU1 = { "vendor": "Google", "model": "v3", "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "tpu"}
TPU2 = { "vendor": "Google", "model": "v3", "ca": "0x1234", "hash": "0x1234", "loc": "DE", "name": "tpu"}
FS   = { "version": "latest", "ca": "0x1234", "name": "m3fs", "hash": "0x1234", "loc": "DE"}

dfg_node:-HWVersionIs(GPU) & HWIsExclusive()
dfg_node:-HWVersionIs(TPU1) & HWIsExclusive()
dfg_node:-HWVersionIs(TPU2) & HWIsExclusive()
dfg_node:-SWVersionIs(FS)

dfg_edge:-Src(MAIN) & Sink(GPU)
dfg_edge:-Src(TPU1) & Sink(MAIN)
dfg_edge:-Src(GPU) & Sink(FS)
dfg_edge:-Src(FS) & Sink(GPU)
dfg_edge:-Src(GPU) & Sink(TPU1)
dfg_edge:-Src(TPU1) & Sink(FS)
dfg_edge:-Src(FS) & Sink(TPU1)
dfg_edge:-Src(TPU2) & Sink(MAIN)
dfg_edge:-Src(GPU) & Sink(TPU2)
dfg_edge:-Src(TPU2) & Sink(FS)
dfg_edge:-Src(FS) & Sink(TPU2)
"#;

#[no_mangle]
pub fn main() -> i32 {
    println!("Hello World");

    let start_cycles = CycleInstant::now();
    compile_policy(m3fs_read);
    let end_cycles = start_cycles.elapsed();
    println!("m3fs_read: {}", end_cycles.as_raw());

    let start_cycles = CycleInstant::now();
    compile_policy(m3fs_write);
    let end_cycles = start_cycles.elapsed();
    println!("m3fs_write: {}", end_cycles.as_raw());

    let start_cycles = CycleInstant::now();
    compile_policy(imgproc);
    let end_cycles = start_cycles.elapsed();
    println!("imgproc: {}", end_cycles.as_raw());

    let start_cycles = CycleInstant::now();
    compile_policy(facever);
    let end_cycles = start_cycles.elapsed();
    println!("facever: {}", end_cycles.as_raw());

    let start_cycles = CycleInstant::now();
    compile_policy(systolic);
    let end_cycles = start_cycles.elapsed();
    println!("systolic: {}", end_cycles.as_raw());

    let start_cycles = CycleInstant::now();
    compile_policy(dist);
    let end_cycles = start_cycles.elapsed();
    println!("dist: {}", end_cycles.as_raw());

    println!("Done");

    0
}
