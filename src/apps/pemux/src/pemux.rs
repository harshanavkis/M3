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

#![feature(asm)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![no_std]

#[macro_use]
extern crate base;
extern crate paging;

mod arch;
mod corereq;
mod helper;
mod pexcalls;
mod upcalls;
mod vma;
mod vpe;

use base::cell::StaticCell;
use base::cfg;
use base::envdata;
use base::io;
use base::kif;
use base::libc;
use base::tcu;
use base::util;
use core::intrinsics;

/// Logs errors
pub const LOG_ERR: bool = true;
/// Logs pexcalls
pub const LOG_CALLS: bool = false;
/// Logs VPE operations
pub const LOG_VPES: bool = false;
/// Logs upcalls
pub const LOG_UPCALLS: bool = false;
/// Logs foreign messages
pub const LOG_FOREIGN_MSG: bool = false;

extern "C" {
    fn heap_init(begin: usize, end: usize);
    fn gem5_shutdown(delay: u64);

    static isr_stack_low: libc::c_void;
}

#[used]
static mut HEAP: [u64; 8 * 1024] = [0; 8 * 1024];

pub struct PagefaultMessage {
    pub op: u64,
    pub virt: u64,
    pub access: u64,
}

// ensure that there is no page-boundary within the messages
#[repr(align(4096))]
pub struct Messages {
    pub pagefault: PagefaultMessage,
    pub exit_notify: kif::pemux::Exit,
    pub upcall_reply: kif::DefaultReply,
}

static MSGS: StaticCell<Messages> = StaticCell::new(Messages {
    pagefault: PagefaultMessage {
        op: 0,
        virt: 0,
        access: 0,
    },
    exit_notify: kif::pemux::Exit {
        code: 0,
        op: 0,
        vpe_sel: 0,
    },
    upcall_reply: kif::DefaultReply { error: 0 },
});

pub fn msgs_mut() -> &'static mut Messages {
    const_assert!(util::size_of::<Messages>() <= cfg::PAGE_SIZE);
    MSGS.get_mut()
}

#[no_mangle]
pub extern "C" fn abort() {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) {
    unsafe { gem5_shutdown(0) };
}

pub fn env() -> &'static mut envdata::EnvData {
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}

#[no_mangle]
pub fn sleep() {
    loop {
        // ack events since to VPE is currently not running
        tcu::TCU::fetch_events();
        tcu::TCU::sleep().ok();
    }
}

static SCHED: StaticCell<Option<vpe::ScheduleAction>> = StaticCell::new(None);

fn leave(state: &mut arch::State) -> *mut libc::c_void {
    upcalls::check();

    if let Some(action) = SCHED.set(None) {
        vpe::schedule(state as *mut _ as usize, action) as *mut libc::c_void
    }
    else {
        state as *mut _ as *mut libc::c_void
    }
}

pub fn reg_scheduling(action: vpe::ScheduleAction) {
    SCHED.set(Some(action));
}

pub extern "C" fn unexpected_irq(state: &mut arch::State) -> *mut libc::c_void {
    log!(LOG_ERR, "Unexpected IRQ with {:?}", state);
    vpe::remove_cur(1);

    leave(state)
}

pub extern "C" fn mmu_pf(state: &mut arch::State) -> *mut libc::c_void {
    if arch::handle_mmu_pf(state).is_err() {
        vpe::remove_cur(1);
    }

    leave(state)
}

pub extern "C" fn pexcall(state: &mut arch::State) -> *mut libc::c_void {
    pexcalls::handle_call(state);

    leave(state)
}

pub extern "C" fn tcu_irq(state: &mut arch::State) -> *mut libc::c_void {
    #[cfg(any(target_arch = "arm", target_arch = "riscv64"))]
    tcu::TCU::clear_irq();

    // core request from TCU?
    let core_req = tcu::TCU::get_core_req();
    if core_req != 0 {
        // acknowledge the request
        tcu::TCU::set_core_req(0);

        if (core_req & 0x1) != 0 {
            corereq::handle_recv(core_req);
        }
        else {
            vma::handle_xlate(core_req)
        }
    }

    leave(state)
}

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        arch::init();

        heap_init(
            &HEAP as *const u64 as usize,
            &HEAP as *const u64 as usize + HEAP.len() * 8,
        );
    }

    io::init(env().pe_id, "pemux");
    vpe::init(
        env().pe_id,
        kif::PEDesc::new_from(env().pe_desc),
        env().pe_mem_base,
        env().pe_mem_size,
    );

    // switch to idle
    let state_addr =
        unsafe { &isr_stack_low as *const _ as usize } - util::size_of::<arch::State>();
    vpe::idle().start(state_addr);
    vpe::schedule(state_addr, vpe::ScheduleAction::Preempt);
}
