/*
 * Device-to-device DMA bandwidth benchmark: this activity (on an SPM tile) reads from and
 * writes to a 2 MiB buffer in the scratchpad of a child activity on another SPM tile, with
 * 4 KiB, 64 KiB, 256 KiB and 1 MiB per MemGate command.
 */

#![no_std]

use m3::cell::StaticRefCell;
use m3::com::{recv_msg, MemGate, RecvGate, SGateArgs, SendGate};
use m3::kif;
use m3::mem::AlignedBuf;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::time::{CycleInstant, Profiler};
use m3::{println, reply_vmsg, send_vmsg, wv_assert_ok, wv_perf, wv_run_suite, wv_run_test};

const SIZE: usize = 2 * 1024 * 1024;
const BUF_SIZE: usize = 1024 * 1024;

// local buffer (source/destination of the transfers)
static BUF: StaticRefCell<AlignedBuf<BUF_SIZE>> = StaticRefCell::new(AlignedBuf::new_zeroed());
// remote buffer (in the child's scratchpad)
static RBUF: StaticRefCell<AlignedBuf<SIZE>> = StaticRefCell::new(AlignedBuf::new_zeroed());

fn read_with(mgate: &MemGate, cmd: usize, name: &str) {
    let buf = &mut BUF.borrow_mut()[..cmd];
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

fn write_with(mgate: &MemGate, cmd: usize, name: &str) {
    let buf = &BUF.borrow()[..cmd];
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

fn p2p(_t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("imem+riscv"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("peer")));

    // channel for the peer to tell us its buffer address and to wait for our "done"
    let mut rgate = wv_assert_ok!(RecvGate::new(8, 8));
    wv_assert_ok!(rgate.activate());
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));
    wv_assert_ok!(act.delegate_obj(sgate.sel()));
    let mut dst = act.data_sink();
    dst.push(sgate.sel());

    let run = wv_assert_ok!(act.run(|| {
        let sgate_sel = Activity::own().data_source().pop().unwrap();
        let sgate = SendGate::new_bind(sgate_sel);
        let addr = RBUF.borrow().as_ptr() as usize;
        // announce our buffer and wait for the benchmark to finish
        wv_assert_ok!(send_vmsg!(&sgate, RecvGate::def(), addr));
        let mut reply = wv_assert_ok!(recv_msg(RecvGate::def()));
        let _ = reply.pop::<u64>();
        0
    }));

    let mut msg = wv_assert_ok!(recv_msg(&rgate));
    let addr = msg.pop::<usize>().unwrap();
    println!("peer buffer at {:#x}", addr);

    let mgate = wv_assert_ok!(run.activity().get_mem(addr as u64, SIZE as u64, kif::Perm::RW));

    read_with(&mgate, 4 * 1024, "read 2 MiB with 4K cmd");
    read_with(&mgate, 64 * 1024, "read 2 MiB with 64K cmd");
    read_with(&mgate, 256 * 1024, "read 2 MiB with 256K cmd");
    read_with(&mgate, 1024 * 1024, "read 2 MiB with 1M cmd");
    write_with(&mgate, 4 * 1024, "write 2 MiB with 4K cmd");
    write_with(&mgate, 64 * 1024, "write 2 MiB with 64K cmd");
    write_with(&mgate, 256 * 1024, "write 2 MiB with 256K cmd");
    write_with(&mgate, 1024 * 1024, "write 2 MiB with 1M cmd");

    wv_assert_ok!(reply_vmsg!(msg, 0u64));
    wv_assert_ok!(run.wait());
}

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, p2p);
}

#[no_mangle]
pub fn main() -> i32 {
    let mut tester = DefaultWvTester::default();
    wv_run_suite!(tester, run);
    0
}
