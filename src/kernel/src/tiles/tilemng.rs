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

use base::cell::{LazyStaticRefCell, RefMut};
use base::col::Vec;
use base::errors::Error;
use base::kif;
use base::tcu::TileId;

use crate::arch::ktcu;
use crate::platform;
use crate::tiles::TileMux;

static INST: LazyStaticRefCell<Vec<TileMux>> = LazyStaticRefCell::default();

pub fn init() {
    deprivilege_tiles();

    let mut muxes = Vec::new();
    for tile in platform::user_tiles() {
        muxes.push(TileMux::new(tile));
    }
    INST.set(muxes);
}

pub fn tilemux(tile: TileId) -> RefMut<'static, TileMux> {
    assert!(tile > 0);
    RefMut::map(INST.borrow_mut(), |tiles| &mut tiles[tile as usize - 1])
}

pub fn find_tile(tiledesc: &kif::TileDesc) -> Option<TileId> {
    for tile in platform::user_tiles() {
        if platform::tile_desc(tile).isa() == tiledesc.isa()
            || platform::tile_desc(tile).tile_type() == tiledesc.tile_type()
        {
            return Some(tile);
        }
    }

    None
}

fn deprivilege_tiles() {
    let mut kern_chain_info = AttestInfo::new();
    let mut kern_nonce: [u8; 16] = [0; 16];

    for tile in platform::user_tiles() {
        // Generate a random nonce
        if let Ok(_) = generate_random_nonce(&mut kern_nonce) {
            klog!(DEF, "generate random nonce ok");
            kern_chain_info.nonce = kern_nonce;
        }
        else {
            klog!(DEF, "generate random nonce Err");
            kern_chain_info.nonce = [2 as u8; 16];
        }
        klog!(DEF, "Generated nonce: {:?}", kern_chain_info.nonce);

        // Generate an ecdsa signature
        let mut dummy_signature: [u8; 64] = [0; 64];
        let mut dummy_priv_key: [u8; 32] = [0; 32];
        let sign_data = generate_signature_data(&dummy_priv_key, &kern_nonce);
        if let Err(_) = generate_ecdsa_signature(&sign_data, &mut dummy_signature) {
            klog!(DEF, "generate ecdsa sign err");
        }
        klog!(DEF, "Generated ecdsa sign: {:?}", dummy_signature);

        // Verify ecdsa signature
        let dummy_msg: [u8; 16] = [0 as u8; 16];
        let dummy_pub_key: [u8; 64] = [0 as u8; 64];
        let dummy_signature: [u8; 64] = [0 as u8; 64];
        let verif_data = generate_verif_data(&dummy_msg, &dummy_signature, &dummy_pub_key);
        match verify_ecdsa_signature(16, &dummy_signature) {
            Ok(_) => klog!(DEF, "ECDSA signature verification successful!"),
            Err(_) => klog!(DEF, "ECDSA signature verification not successful"),
        };

        // Perform attestation
        attest_tile(tile, &kern_chain_info);

        // Take away kernel privileges from other tiles
        ktcu::deprivilege_tile(tile).expect("Unable to deprivilege tile");
    }
}

const CERT_LEN: u64 = 128;
const KERNEL_CHAIN_LEN: u64 = 4;
const ICU_CHAIN_LEN: u8 = 1;
const KERN_NONCE: [u8; 16] = [1; 16];

struct AttestInfo {
    nonce: [u8; 16],
    chain_length: u64,
    certificate_chain: Vec<u8>,
}

impl AttestInfo {
    fn new() -> Self {
        // TODO: Read chain length and certificate from ICU private memory
        // This info is set after secure boot
        let chain_size = CERT_LEN * (KERNEL_CHAIN_LEN as u64);
        let mut certificate_chain = Vec::with_capacity(chain_size as usize);
        certificate_chain.resize(chain_size as usize, 0);

        // TODO: Randomize nonce
        Self {
            nonce: KERN_NONCE,
            chain_length: KERNEL_CHAIN_LEN,
            certificate_chain,
        }
    }
}

fn generate_signature_data(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut sign_data = Vec::<u8>::new();
    sign_data.extend_from_slice(key);
    sign_data.extend_from_slice(data);

    sign_data
}

fn generate_verif_data(message: &[u8], signature: &[u8], pub_key: &[u8]) -> Vec<u8> {
    let mut verif_data = Vec::<u8>::new();
    verif_data.extend(message);
    verif_data.extend(signature);
    verif_data.extend(pub_key);

    verif_data
}

fn generate_random_nonce(dest: &mut [u8]) -> Result<(), Error> {
    base::tcu::TCU::gen_random(dest.as_mut_ptr())?;
    Ok(())
}

fn generate_ecdsa_signature(src: &[u8], dest: &mut [u8]) -> Result<(), Error> {
    base::tcu::TCU::gen_ecdsa_sign(src.len(), src.as_ptr(), dest.as_mut_ptr())?;
    Ok(())
}

fn verify_ecdsa_signature(msg_len: usize, src: &[u8]) -> Result<(), Error> {
    base::tcu::TCU::verify_ecdsa_sign(msg_len, src.as_ptr())?;
    Ok(())
}

fn attest_tile(tile: TileId, attest_info: &AttestInfo) -> Result<(), Error> {
    klog!(DEF, "Attesting tile: {}", tile);

    // Create memory endpoint to exchange attestation data

    // Start the attestation
    ktcu::attest_tile_remote(tile, 0);

    // Create receive endpoint locally and send endpoint at remote tile
    // Use Tcu::wait_for_msg to sleep until a message arrives

    Ok(())
}
