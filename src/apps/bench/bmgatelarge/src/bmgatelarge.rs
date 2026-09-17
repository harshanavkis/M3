/*
 * DMA bandwidth benchmark with large commands: reads/writes a 2 MiB memory region with
 * 4 KiB, 64 KiB, 256 KiB and 1 MiB per MemGate command. On tiles without virtual memory,
 * each command is handed to the TCU as a whole, so that it can pipeline the packets.
 */

#![no_std]

use m3::cell::StaticRefCell;
use m3::com::MemGate;
use m3::kif;
use m3::mem::AlignedBuf;
use m3::test::{DefaultWvTester, WvTester};
use m3::time::{CycleInstant, Profiler};
use m3::{wv_perf, wv_run_suite, wv_run_test};

const SIZE: usize = 2 * 1024 * 1024;
const BUF_SIZE: usize = 1024 * 1024;

static BUF: StaticRefCell<AlignedBuf<BUF_SIZE>> = StaticRefCell::new(AlignedBuf::new_zeroed());

fn read_with(cmd: usize, name: &str) {
    let buf = &mut BUF.borrow_mut()[..cmd];
    let mgate = MemGate::new(SIZE, kif::Perm::R).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        name,
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.read(buf, total as u64).expect("Reading failed");
                total += buf.len();
            }
        })
    );
}

fn write_with(cmd: usize, name: &str) {
    let buf = &BUF.borrow()[..cmd];
    let mgate = MemGate::new(SIZE, kif::Perm::W).expect("Unable to create mgate");

    let mut prof = Profiler::default().repeats(5).warmup(1);

    wv_perf!(
        name,
        prof.run::<CycleInstant, _>(|| {
            let mut total = 0;
            while total < SIZE {
                mgate.write(buf, total as u64).expect("Writing failed");
                total += buf.len();
            }
        })
    );
}

fn read4k(_t: &mut dyn WvTester) {
    read_with(4 * 1024, "read 2 MiB with 4K cmd");
}
fn read64k(_t: &mut dyn WvTester) {
    read_with(64 * 1024, "read 2 MiB with 64K cmd");
}
fn read256k(_t: &mut dyn WvTester) {
    read_with(256 * 1024, "read 2 MiB with 256K cmd");
}
fn read1m(_t: &mut dyn WvTester) {
    read_with(1024 * 1024, "read 2 MiB with 1M cmd");
}
fn write4k(_t: &mut dyn WvTester) {
    write_with(4 * 1024, "write 2 MiB with 4K cmd");
}
fn write64k(_t: &mut dyn WvTester) {
    write_with(64 * 1024, "write 2 MiB with 64K cmd");
}
fn write256k(_t: &mut dyn WvTester) {
    write_with(256 * 1024, "write 2 MiB with 256K cmd");
}
fn write1m(_t: &mut dyn WvTester) {
    write_with(1024 * 1024, "write 2 MiB with 1M cmd");
}

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, read4k);
    wv_run_test!(t, read64k);
    wv_run_test!(t, read256k);
    wv_run_test!(t, read1m);
    wv_run_test!(t, write4k);
    wv_run_test!(t, write64k);
    wv_run_test!(t, write256k);
    wv_run_test!(t, write1m);
}

#[no_mangle]
pub fn main() -> i32 {
    let mut tester = DefaultWvTester::default();
    wv_run_suite!(tester, run);
    0
}
