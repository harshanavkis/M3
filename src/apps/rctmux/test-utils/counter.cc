/**
* Copyright (C) 2015, René Küttner <rene.kuettner@.tu-dresden.de>
* Economic rights: Technische Universität Dresden (Germany)
*
* This file is part of M3 (Microkernel for Minimalist Manycores).
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

#include <m3/Common.h>
#include <m3/stream/Serial.h>
#include <m3/cap/VPE.h>
#include <m3/Syscalls.h>
#include <m3/DTU.h>

using namespace m3;

int main(int argc, char **argv)
{
    unsigned int counter = 0;
    char *name = argv[0];

    Serial::get() << "Counter program started...\n";

    // this program simply counts and prints a message at every step

    while (1) {
        Serial::get() << "Message " << counter << " from " << name << "\n";
        ++counter;
    }

    return 0;
}
