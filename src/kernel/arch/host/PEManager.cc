/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include "pes/PEManager.h"
#include "TCU.h"

namespace kernel {

void PEManager::init_vpe(UNUSED VPE *vpe) {
}

void PEManager::start_vpe(VPE *vpe) {
    vpe->init_memory();
}

void PEManager::stop_vpe(VPE *vpe) {
    TCU::kill_vpe(vpe->peid());
}

}
