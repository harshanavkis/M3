/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#![feature(core_intrinsics)]
#![feature(trace_macros)]
#![no_std]

#[allow(unused_extern_crates)]
extern crate heap;

// init stuff
#[cfg(not(target_vendor = "host"))]
pub use arch::init::{env_run, exit};
#[cfg(target_vendor = "host")]
pub use arch::init::{exit, rust_deinit, rust_init};

#[macro_use]
pub mod io;
#[macro_use]
pub mod com;

/// Netstack related structures
pub mod net;

pub use base::{
    backtrace, borrow, boxed, build_vmsg, cell, cfg, col, cpu, elf, env, errors, format, function,
    goff, impl_boxitem, int_enum, kif, libc, llog, log, math, mem, parse, quota, random, rc, serde,
    serialize, sync, tcu, time, tmif, util, vec,
};

pub mod cap;
pub mod crypto;
pub mod policy_compiler;
pub mod dataflow;
pub mod server;
pub mod session;
pub mod syscalls;
#[macro_use]
pub mod test;
pub mod tiles;
pub mod vfs;

mod arch;
