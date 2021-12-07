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

#include <base/stream/Serial.h>
#include <base/time/Instant.h>

#include <m3/accel/StreamAccel.h>
#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>

#include "accelchain.h"

using namespace m3;

static constexpr bool VERBOSE           = 1;
static constexpr size_t PIPE_SHM_SIZE   = 512 * 1024;

class Chain {
    static const size_t MAX_NUM     = 8;

public:
    explicit Chain(Pipes &pipesrv, Reference<File> in, Reference<File> out, size_t _num,
                   CycleDuration comptime, Mode _mode)
        : num(_num),
          mode(_mode),
          vpes(),
          accels(),
          pipes(),
          mems() {
        // create VPEs
        for(size_t i = 0; i < num; ++i) {
            OStringStream name;
            name << "chain" << i;

            if(VERBOSE) Serial::get() << "Creating VPE " << name.str() << "\n";

            pes[i] = PE::get("copy");
            vpes[i] = std::make_unique<VPE>(pes[i], name.str());

            accels[i] = std::make_unique<StreamAccel>(vpes[i], comptime);

            if(mode == Mode::DIR_SIMPLE && i + 1 < num) {
                mems[i] = std::make_unique<MemGate>(
                    MemGate::create_global(PIPE_SHM_SIZE, MemGate::RW));
                pipes[i] = std::make_unique<IndirectPipe>(
                    pipesrv, *mems[i], PIPE_SHM_SIZE);
            }
        }

        if(VERBOSE) Serial::get() << "Connecting input and output...\n";

        // connect input/output
        accels[0]->connect_input(static_cast<GenericFile*>(in.get()));
        accels[num - 1]->connect_output(static_cast<GenericFile*>(out.get()));
        for(size_t i = 0; i < num; ++i) {
            if(i > 0) {
                if(mode == Mode::DIR_SIMPLE) {
                    Reference<File> rd = VPE::self().fds()->get(pipes[i - 1]->reader_fd());
                    accels[i]->connect_input(static_cast<GenericFile*>(rd.get()));
                }
                else
                    accels[i]->connect_input(accels[i - 1].get());
            }
            if(i + 1 < num) {
                if(mode == Mode::DIR_SIMPLE) {
                    Reference<File> wr = VPE::self().fds()->get(pipes[i]->writer_fd());
                    accels[i]->connect_output(static_cast<GenericFile*>(wr.get()));
                }
                else
                    accels[i]->connect_output(accels[i + 1].get());
            }
        }
    }

    void start() {
        for(size_t i = 0; i < num; ++i) {
            vpes[i]->start();
            running[i] = true;
        }
    }

    void add_running(capsel_t *sels, size_t *count) {
        for(size_t i = 0; i < num; ++i) {
            if(running[i])
                sels[(*count)++] = vpes[i]->sel();
        }
    }
    void terminated(capsel_t vpe, int exitcode) {
        for(size_t i = 0; i < num; ++i) {
            if(running[i] && vpes[i]->sel() == vpe) {
                if(exitcode != 0) {
                    cerr << "chain" << i
                         << " terminated with exit code " << exitcode << "\n";
                }
                if(mode == Mode::DIR_SIMPLE) {
                    if(pipes[i])
                        pipes[i]->close_writer();
                    if(i > 0 && pipes[i - 1])
                        pipes[i - 1]->close_reader();
                }
                running[i] = false;
                break;
            }
        }
    }

private:
    size_t num;
    Mode mode;
    Reference<PE> pes[MAX_NUM];
    std::unique_ptr<VPE> vpes[MAX_NUM];
    std::unique_ptr<StreamAccel> accels[MAX_NUM];
    std::unique_ptr<IndirectPipe> pipes[MAX_NUM];
    std::unique_ptr<MemGate> mems[MAX_NUM];
    bool running[MAX_NUM];
};

void chain_direct(Reference<File> in, Reference<File> out, size_t num,
                  CycleDuration comptime, Mode mode) {
    Pipes pipes("pipes");
    Chain ch(pipes, in, out, num, comptime, mode);

    if(VERBOSE) Serial::get() << "Starting chain...\n";

    auto start = CycleInstant::now();

    ch.start();

    // wait for their completion
    for(size_t rem = num; rem > 0; --rem) {
        size_t count = 0;
        capsel_t sels[num];
        ch.add_running(sels, &count);

        capsel_t vpe;
        int exitcode = Syscalls::vpe_wait(sels, rem, 0, &vpe);
        ch.terminated(vpe, exitcode);
    }

    auto end = CycleInstant::now();
    Serial::get() << "Total time: " << end.duration_since(start) << "\n";
}

void chain_direct_multi(Reference<File> in, Reference<File> out, size_t num,
                        CycleDuration comptime, Mode mode) {
    Pipes pipes("pipes");
    Chain ch1(pipes, in, out, num, comptime, mode);

    fd_t outfd = VFS::open("/tmp/out2.txt", FILE_W | FILE_TRUNC | FILE_CREATE);
    auto in2 = in->clone();
    Chain ch2(pipes, in2, VPE::self().fds()->get(outfd), num, comptime, mode);

    if(VERBOSE) Serial::get() << "Starting chains...\n";

    auto start = CycleInstant::now();

    ch1.start();
    ch2.start();

    // wait for their completion
    for(size_t rem = num * 2; rem > 0; --rem) {
        size_t count = 0;
        capsel_t sels[num * 2];
        ch1.add_running(sels, &count);
        ch2.add_running(sels, &count);

        capsel_t vpe;
        int exitcode = Syscalls::vpe_wait(sels, rem, 0, &vpe);
        ch1.terminated(vpe, exitcode);
        ch2.terminated(vpe, exitcode);
    }

    auto end = CycleInstant::now();
    Serial::get() << "Total time: " << end.duration_since(start) << "\n";

    VFS::close(outfd);
}
