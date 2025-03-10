/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::cfg;
use base::col::{String, Vec};
use base::format;
use base::goff;
use base::kif::{boot, TileDesc, TileISA, TileType};
use base::libc;
use base::mem::{size_of, GlobAddr};
use base::tcu::{EpId, TileId};
use base::vec;
use core::ptr;

use crate::arch;
use crate::args;
use crate::mem;
use crate::platform;

pub fn init(args: &[String]) -> platform::KEnv {
    let mut info = boot::Info::default();

    // tiles
    let mut tiles = Vec::new();
    for _ in 0..cfg::TILE_COUNT {
        tiles.push(TileDesc::new(
            TileType::COMP_IMEM,
            TileISA::X86,
            1024 * 1024,
        ));
    }
    if args::get().disk {
        tiles.push(TileDesc::new(TileType::COMP_IMEM, TileISA::IDE_DEV, 0));
    }
    if args::get().net_bridge.is_some() {
        tiles.push(TileDesc::new(TileType::COMP_IMEM, TileISA::NIC_DEV, 0));
        tiles.push(TileDesc::new(TileType::COMP_IMEM, TileISA::NIC_DEV, 0));
    }
    let mut utiles = Vec::new();
    for (i, tile) in tiles[1..].iter().enumerate() {
        utiles.push(boot::Tile::new((i + 1) as u32, *tile));
    }
    info.tile_count = utiles.len() as u64;

    let mems = build_mems();
    info.mem_count = mems.len() as u64;

    let mods = build_modules(args);
    info.mod_count = mods.len() as u64;

    // build kinfo page
    let bsize = size_of::<boot::Info>()
        + info.mod_count as usize * size_of::<boot::Mod>()
        + info.tile_count as usize * size_of::<boot::Tile>()
        + info.mem_count as usize * size_of::<boot::Mem>();
    let binfo_mem = mem::borrow_mut()
        .allocate(mem::MemType::KERNEL, bsize as goff, 1)
        .expect("Unable to allocate mem for boot info");

    unsafe {
        // info
        let mut dest = binfo_mem.global().offset();
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            &info as *const boot::Info as *const libc::c_void,
            size_of::<boot::Info>(),
        );
        dest += size_of::<boot::Info>() as goff;

        // modules
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            mods.as_ptr() as *const libc::c_void,
            mods.len() * size_of::<boot::Mod>(),
        );
        dest += (mods.len() * size_of::<boot::Mod>()) as goff;

        // tiles
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            utiles.as_ptr() as *const libc::c_void,
            utiles.len() * size_of::<boot::Tile>(),
        );
        dest += (utiles.len() * size_of::<boot::Tile>()) as goff;

        // memories
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            mems.as_ptr() as *const libc::c_void,
            mems.len() * size_of::<boot::Mem>(),
        );
    }

    platform::KEnv::new(info, binfo_mem.global(), mods, tiles)
}

fn build_mems() -> Vec<boot::Mem> {
    // create memory
    let base = unsafe {
        libc::mmap(
            ptr::null_mut(),
            cfg::TOTAL_MEM_SIZE,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANON | libc::MAP_PRIVATE,
            -1,
            0,
        )
    };
    assert!(base != libc::MAP_FAILED);
    let mut off = base as goff;

    // fs image
    mem::borrow_mut().add(mem::MemMod::new(
        mem::MemType::OCCUPIED,
        kernel_tile(),
        off,
        cfg::FS_MAX_SIZE as goff,
    ));
    off += cfg::FS_MAX_SIZE as goff;

    // kernel memory
    mem::borrow_mut().add(mem::MemMod::new(
        mem::MemType::KERNEL,
        kernel_tile(),
        off,
        args::get().kmem as goff,
    ));
    off += args::get().kmem as goff;

    // boot module memory
    let boot_off = off;
    mem::borrow_mut().add(mem::MemMod::new(
        mem::MemType::BOOT,
        kernel_tile(),
        off,
        cfg::FIXED_ROOT_MEM as goff,
    ));
    off += cfg::FIXED_ROOT_MEM as goff;

    // user memory
    let user_size =
        cfg::TOTAL_MEM_SIZE - (cfg::FS_MAX_SIZE + args::get().kmem + cfg::FIXED_ROOT_MEM);
    mem::borrow_mut().add(mem::MemMod::new(
        mem::MemType::USER,
        kernel_tile(),
        off,
        user_size as goff,
    ));

    // set memories
    vec![
        boot::Mem::new(
            GlobAddr::new_with(kernel_tile(), 0),
            cfg::FS_MAX_SIZE as goff,
            true,
        ),
        boot::Mem::new(
            GlobAddr::new_with(kernel_tile(), boot_off),
            cfg::FIXED_ROOT_MEM as goff,
            true,
        ),
        boot::Mem::new(
            GlobAddr::new_with(kernel_tile(), off),
            user_size as goff,
            false,
        ),
    ]
}

fn build_modules(args: &[String]) -> Vec<boot::Mod> {
    let mut mods = Vec::new();

    for arg in args {
        // copy boot module into memory
        unsafe {
            let path = format!("{}\0", arg);
            let fd = libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDONLY);
            if fd == -1 {
                panic!("Opening {} for reading failed", arg);
            }
            let mut finfo: libc::stat = core::mem::zeroed();
            if libc::fstat(fd, &mut finfo) == -1 {
                panic!("Stat for {} failed", arg);
            }

            let alloc = mem::borrow_mut()
                .allocate(mem::MemType::BOOT, finfo.st_size as goff, 1)
                .expect("Unable to alloc mem for boot module");
            let dest = alloc.global().offset() as *mut u8 as *mut libc::c_void;
            if libc::read(fd, dest, alloc.size() as usize) == -1 {
                panic!("Reading from {} failed", arg);
            }
            libc::close(fd);

            let mod_name = arg.rsplit('/').next().unwrap();
            mods.push(boot::Mod::new(alloc.global(), alloc.size(), mod_name));
        }
    }

    mods
}

pub fn init_serial(dest: Option<(TileId, EpId)>) {
    arch::input::start(dest);
}

pub fn kernel_tile() -> TileId {
    0
}
pub fn user_tiles() -> platform::TileIterator {
    platform::TileIterator::new(1, (platform::tiles().len() - 1) as TileId)
}

pub fn is_shared(_tile: TileId) -> bool {
    false
}

pub fn rbuf_tilemux(_tile: TileId) -> goff {
    0
}
