/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/stream/Serial.h>
#include <base/CPU.h>
#include <base/Env.h>
#include <base/Exceptions.h>

#include <exception>
#include <stdlib.h>

typedef void (*constr_func)();

extern constr_func CTORS_BEGIN;
extern constr_func CTORS_END;

EXTERN_C void _init();

namespace m3 {

void Env::pre_init() {
}

void Env::post_init() {
    m3::Exceptions::init();

    std::set_terminate([] {
        m3::Serial::get() << "Unhandled exception. Terminating.\n";
        abort();
    });

    // call constructors
    _init();
    for(constr_func *func = &CTORS_BEGIN; func < &CTORS_END; ++func)
        (*func)();
}

void Env::pre_exit() {
}

void Env::jmpto(uintptr_t addr) {
    CPU::jumpto(addr);
}

}
