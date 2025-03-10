/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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
#include <base/util/Util.h>

#include "../assert.h"
#include "../tcuif.h"
#include "../tiles.h"

using namespace m3;

static constexpr epid_t MEP = TCU::FIRST_USER_EP;

static ALIGNED(8) uint8_t buf1[1024];
static ALIGNED(8) uint8_t buf2[1024];
static ALIGNED(8) uint8_t buf3[1024];

int main() {
    Tile own_tile = static_cast<Tile>(env()->tile_id);
    Tile partner_tile = static_cast<Tile>((static_cast<tileid_t>(own_tile) + 1) % 8);

    Serial::get() << "Hello from Tile" << static_cast<tileid_t>(own_tile) << " (partner Tile"
                  << static_cast<tileid_t>(partner_tile) << ")!\n";

    kernel::TCU::config_mem(MEP, tile_id(partner_tile), reinterpret_cast<uintptr_t>(buf1),
                            sizeof(buf1), TCU::R | TCU::W);

    for(size_t i = 0; i < ARRAY_SIZE(buf2); ++i)
        buf2[i] = static_cast<tileid_t>(own_tile) + i;

    for(int i = 0; i < 10000; ++i) {
        if(i % 1000 == 0)
            Serial::get() << "read-write test " << i << "\n";

        ASSERT_EQ(kernel::TCU::write(MEP, buf2, sizeof(buf2), 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(MEP, buf3, sizeof(buf3), 0), Errors::NONE);

        for(size_t i = 0; i < ARRAY_SIZE(buf2); ++i)
            ASSERT_EQ(buf2[i], buf3[i]);
    }

    Serial::get() << "\x1B[1;32mAll tests successful!\x1B[0;m\n";

    // give the other tiles some time
    for(volatile int i = 0; i < 1000000; ++i)
        ;

    // for the test infrastructure
    Serial::get() << "Shutting down\n";
    return 0;
}
