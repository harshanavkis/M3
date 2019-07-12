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

#include <base/util/Time.h>

#include <m3/accel/StreamAccel.h>
#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>

using namespace m3;

static constexpr int REPEATS            = 24;
static constexpr cycles_t COMP_TIME     = 1024;

int main() {
    std::unique_ptr<RecvGate> rgates[2];
    std::unique_ptr<SendGate> outs[2];
    std::unique_ptr<SendGate> ins[2];
    std::unique_ptr<StreamAccel> accels[2];
    std::unique_ptr<VPE> vpes[2];

    // create VPEs
    for(size_t i = 0; i < 2; ++i) {
        OStringStream name;
        name << "chain" << i;

        vpes[i] = std::make_unique<VPE>(
            name.str(), VPEArgs().pedesc(PEDesc(PEType::COMP_IMEM, PEISA::ACCEL_FFT))
                                 .flags(VPE::MUXABLE)
        );
        accels[i] = std::make_unique<StreamAccel>(vpes[i], COMP_TIME);
    }

    // create and activate gates
    for(size_t i = 0; i < 2; ++i) {
        rgates[i] = std::make_unique<RecvGate>(
            RecvGate::create(nextlog2<64 * 2>::val, nextlog2<64>::val));
        rgates[i]->activate();

        ins[i] = std::make_unique<SendGate>(
            SendGate::create(rgates[i].get(), SendGateArgs().label(StreamAccel::LBL_IN_REQ)
                                                            .credits(64))
        );
        ins[i]->activate_for(*vpes[i], StreamAccel::EP_IN_SEND);

        outs[i] = std::make_unique<SendGate>(
            SendGate::create(rgates[i].get(), SendGateArgs().label(StreamAccel::LBL_OUT_REQ)
                                                            .credits(64))
        );
        outs[i]->activate_for(*vpes[i], StreamAccel::EP_OUT_SEND);
    }

    // start VPEs
    for(size_t i = 0; i < 2; ++i)
        vpes[i]->start();

    cycles_t start, end, total = 0;
    for(int i = 0; i < REPEATS; ++i) {
        {
            GateIStream msg = receive_msg(*rgates[i % 2]);
            assert(msg.label<label_t>() == StreamAccel::LBL_IN_REQ);

            start = Time::start(0x1234);
            reply_vmsg(msg, Errors::NONE, static_cast<uint64_t>(0), static_cast<uint64_t>(8));
        }

        {
            GateIStream msg = receive_msg(*rgates[i % 2]);
            assert(msg.label<label_t>() == StreamAccel::LBL_OUT_REQ);
            end = Time::stop(0x1234);

            reply_vmsg(msg, Errors::NONE, static_cast<uint64_t>(0), static_cast<uint64_t>(8));
        }

        total += end - start;
    }

    for(int i = 0; i < 2; ++i) {
        GateIStream msg = receive_msg(*rgates[i]);
        assert(msg.label<label_t>() == StreamAccel::LBL_IN_REQ);
        reply_vmsg(msg, Errors::NONE, static_cast<uint64_t>(0), static_cast<uint64_t>(0));
    }

    vpes[0]->wait();
    vpes[1]->wait();

    cout << "Time: " << (total / REPEATS) << " cycles\n";
    return 0;
}
