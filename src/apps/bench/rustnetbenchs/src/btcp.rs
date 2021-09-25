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

use m3::com::Semaphore;
use m3::errors::Code;
use m3::format;
use m3::net::{Endpoint, StreamSocketArgs, TcpSocket};
use m3::println;
use m3::profile::Results;
use m3::session::{NetworkDirection, NetworkManager};
use m3::tcu::TCU;
use m3::test;
use m3::time;
use m3::{wv_assert_eq, wv_assert_ok, wv_perf, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, latency);
    wv_run_test!(t, bandwidth);
}

fn latency() {
    let nm = wv_assert_ok!(NetworkManager::new("net"));
    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(&nm)));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));

    let samples = 5;

    let mut buf = [0u8; 1024];

    // warmup
    for _ in 0..5 {
        wv_assert_ok!(socket.send(&buf));
        let _res = socket.recv(&mut buf);
    }

    let packet_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];

    for pkt_size in &packet_sizes {
        let mut res = Results::new(samples);

        for i in 0..samples {
            let start = time::start(i);

            wv_assert_ok!(socket.send(&buf[0..*pkt_size]));

            let mut recv_size = 0;
            while recv_size < *pkt_size {
                recv_size += wv_assert_ok!(socket.recv(&mut buf));
            }

            let stop = time::stop(i);

            wv_assert_eq!(*pkt_size, recv_size as usize);
            res.push(stop - start);
        }

        wv_perf!(
            format!("network latency ({}b)", pkt_size),
            format!(
                "{:.4} ms (+/- {} with {} runs)",
                res.avg() as f32 / 3000000.,
                res.stddev() / 3000000.,
                res.runs()
            )
        );
    }

    wv_assert_ok!(socket.close());
}

fn bandwidth() {
    const PACKETS_TO_SEND: usize = 105;
    const BURST_SIZE: usize = 2;
    const TIMEOUT: u64 = 1_000_000_000; // 1sec

    let nm = wv_assert_ok!(NetworkManager::new("net"));
    let mut socket = wv_assert_ok!(TcpSocket::new(
        StreamSocketArgs::new(&nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024)
    ));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));

    let mut buf = [0u8; 1024];

    for _ in 0..10 {
        wv_assert_ok!(socket.send(&buf));
        wv_assert_ok!(socket.recv(&mut buf));
    }

    socket.set_blocking(false);

    let start = TCU::nanotime();
    let mut last_received = start;
    let mut sent_count = 0;
    let mut receive_count = 0;
    let mut received_bytes = 0;
    let mut failures = 0;

    loop {
        if failures > 9 {
            failures = 0;
            if sent_count >= PACKETS_TO_SEND {
                let waited = TCU::nanotime() - last_received;
                if waited > TIMEOUT {
                    break;
                }
                // we are not interested in output anymore
                nm.wait_for(TIMEOUT - waited, NetworkDirection::INPUT);
            }
            else {
                nm.wait(NetworkDirection::INPUT | NetworkDirection::OUTPUT);
            }
        }

        for _ in 0..BURST_SIZE {
            if sent_count >= PACKETS_TO_SEND {
                break;
            }

            match socket.send(&buf) {
                Err(e) => {
                    wv_assert_eq!(e.code(), Code::WouldBlock);
                    failures += 1;
                },
                Ok(_) => {
                    sent_count += 1;
                    failures = 0;
                },
            }
        }

        for _ in 0..BURST_SIZE {
            match socket.recv(&mut buf) {
                Err(e) => {
                    wv_assert_eq!(e.code(), Code::WouldBlock);
                    failures += 1;
                },
                Ok(size) => {
                    received_bytes += size as usize;
                    receive_count += 1;
                    last_received = TCU::nanotime();
                    failures = 0;
                },
            }
        }

        if received_bytes >= PACKETS_TO_SEND * buf.len() {
            break;
        }
    }

    println!("Sent packets: {}", sent_count);
    println!("Received packets: {}", receive_count);
    println!("Received bytes: {}", received_bytes);
    let duration = last_received - start;
    println!("Duration: {}", duration);
    let mbps = (received_bytes as f64 / (duration as f64 / 1e9)) / (1024f64 * 1024f64);
    wv_perf!(
        "TCP bandwidth",
        format!("{} MiB/s (+/- 0 with 1 runs)", mbps)
    );

    socket.set_blocking(true);
    wv_assert_ok!(socket.close());
}
