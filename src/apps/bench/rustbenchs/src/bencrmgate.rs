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

use m3::cell::StaticRefCell;
use m3::com::MemGate;
use m3::kif;
use m3::mem::AlignedBuf;
use m3::test::WvTester;
use m3::time::{CycleInstant, Profiler};
use m3::{wv_perf, wv_run_test};

const SIZE: usize = 2 * 1024 * 1024;

static BUF: StaticRefCell<AlignedBuf<{ 8192 + 64 }>> = StaticRefCell::new(AlignedBuf::new_zeroed());

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, read512);
    wv_run_test!(t, read1024);
    wv_run_test!(t, read2048);
    wv_run_test!(t, read4096);
    //wv_run_test!(t, read8192);
    // wv_run_test!(t, read_unaligned512);
    // wv_run_test!(t, read_unaligned1024);
    // wv_run_test!(t, read_unaligned2048);
    // wv_run_test!(t, read_unaligned4096);
    // wv_run_test!(t, read_unaligned8192);
    wv_run_test!(t, write512);
    wv_run_test!(t, write1024);
    wv_run_test!(t, write2048);
    wv_run_test!(t, write4096);
    //wv_run_test!(t, write8192);
    // wv_run_test!(t, write_unaligned512);
    // wv_run_test!(t, write_unaligned1024);
    // wv_run_test!(t, write_unaligned2048);
    // wv_run_test!(t, write_unaligned4096);
    // wv_run_test!(t, write_unaligned8192);
}

fn read512(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[..512];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "read 2 MiB with 512B buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read1024(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[..1024];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "read 2 MiB with 1K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read2048(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[..2048];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "read 2 MiB with 2K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read4096(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[..4096];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "read 2 MiB with 4K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read8192(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[..8192];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "read 2 MiB with 8K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read_unaligned512(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[64..576];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read unaligned 2 MiB with 512B buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read_unaligned1024(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[64..1088];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read unaligned 2 MiB with 1K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read_unaligned2048(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[64..2112];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read unaligned 2 MiB with 2K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read_unaligned4096(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[64..4160];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read unaligned 2 MiB with 4K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn read_unaligned8192(_t: &mut dyn WvTester) {
    let buf = &mut BUF.borrow_mut()[64..];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "read unaligned 2 MiB with 8K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn write512(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[..512];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "write 2 MiB with 512B buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}

fn write1024(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[..1024];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "write 2 MiB with 1K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}

fn write2048(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[..2048];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "write 2 MiB with 2K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}

fn write4096(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[..4096];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "write 2 MiB with 4K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}

fn write8192(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[..8192];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        "write 2 MiB with 8K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}

fn write_unaligned512(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[64..576];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write unaligned 2 MiB with 512B buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}

fn write_unaligned1024(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[64..1088];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write unaligned 2 MiB with 1K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}
fn write_unaligned2048(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[64..2112];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write unaligned 2 MiB with 2K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}
fn write_unaligned4096(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[64..4160];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write unaligned 2 MiB with 4K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}
fn write_unaligned8192(_t: &mut dyn WvTester) {
    let buf = &BUF.borrow()[64..];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(2).warmup(1);

    wv_perf!(
        "write unaligned 2 MiB with 8K buf",
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}
