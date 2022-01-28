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

use m3::cell::StaticRefCell;
use m3::com::MemGate;
use m3::io;
use m3::kif;
use m3::mem::AlignedBuf;
use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::session::Pipes;
use m3::test;
use m3::time::{CycleInstant, Profiler};
use m3::vfs::IndirectPipe;
use m3::{format, wv_assert_eq, wv_assert_ok, wv_perf, wv_run_test};

const DATA_SIZE: usize = 2 * 1024 * 1024;
const BUF_SIZE: usize = 8 * 1024;

static BUF: StaticRefCell<AlignedBuf<BUF_SIZE>> = StaticRefCell::new(AlignedBuf::new_zeroed());

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, child_to_parent);
    wv_run_test!(t, parent_to_child);
}

fn child_to_parent() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let mut prof = Profiler::default().repeats(2).warmup(1);

    let pe = wv_assert_ok!(PE::get("clone|own"));
    let res = prof.run::<CycleInstant, _>(|| {
        let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
        let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

        let mut vpe = wv_assert_ok!(VPE::new_with(pe.clone(), VPEArgs::new("writer")));
        vpe.files().set(
            io::STDOUT_FILENO,
            VPE::cur().files().get(pipe.writer_fd()).unwrap(),
        );
        wv_assert_ok!(vpe.obtain_fds());

        let act = wv_assert_ok!(vpe.run(|| {
            let output = VPE::cur().files().get(io::STDOUT_FILENO).unwrap();
            let buf = BUF.borrow();
            let mut rem = DATA_SIZE;
            while rem > 0 {
                wv_assert_ok!(output.borrow_mut().write(&buf[..]));
                rem -= BUF_SIZE;
            }
            0
        }));

        pipe.close_writer();

        let input = VPE::cur().files().get(pipe.reader_fd()).unwrap();
        let mut buf = BUF.borrow_mut();
        while wv_assert_ok!(input.borrow_mut().read(&mut buf[..])) > 0 {}

        wv_assert_eq!(act.wait(), Ok(0));
    });

    wv_perf!(
        format!(
            "c->p: {} KiB transfer with {} KiB buf",
            DATA_SIZE / 1024,
            BUF_SIZE / 1024
        ),
        res
    );
}

fn parent_to_child() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let mut prof = Profiler::default().repeats(2).warmup(1);

    let pe = wv_assert_ok!(PE::get("clone|own"));
    let res = prof.run::<CycleInstant, _>(|| {
        let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
        let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

        let mut vpe = wv_assert_ok!(VPE::new_with(pe.clone(), VPEArgs::new("reader")));
        vpe.files().set(
            io::STDIN_FILENO,
            VPE::cur().files().get(pipe.reader_fd()).unwrap(),
        );
        wv_assert_ok!(vpe.obtain_fds());

        let act = wv_assert_ok!(vpe.run(|| {
            let input = VPE::cur().files().get(io::STDIN_FILENO).unwrap();
            let mut buf = BUF.borrow_mut();
            while wv_assert_ok!(input.borrow_mut().read(&mut buf[..])) > 0 {}
            0
        }));

        pipe.close_reader();

        let output = VPE::cur().files().get(pipe.writer_fd()).unwrap();
        let buf = BUF.borrow();
        let mut rem = DATA_SIZE;
        while rem > 0 {
            wv_assert_ok!(output.borrow_mut().write(&buf[..]));
            rem -= BUF_SIZE;
        }

        pipe.close_writer();

        wv_assert_eq!(act.wait(), Ok(0));
    });

    wv_perf!(
        format!(
            "p->c: {} KiB transfer with {} KiB buf",
            DATA_SIZE / 1024,
            BUF_SIZE / 1024
        ),
        res
    );
}
