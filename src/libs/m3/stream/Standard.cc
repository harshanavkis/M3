/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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
#include <base/Init.h>

#include <m3/stream/Standard.h>

namespace m3 {

// create them after activity::self() has finished, because otherwise the file objects are not
// available
INIT_PRIO_STREAM FStream cerr(STDERR_FD, FILE_W, 256, FStream::FL_LINE_BUF);
INIT_PRIO_STREAM FStream cout(STDOUT_FD, FILE_W, 256, FStream::FL_LINE_BUF);
INIT_PRIO_STREAM FStream cin(STDIN_FD, FILE_R, 128);

}
