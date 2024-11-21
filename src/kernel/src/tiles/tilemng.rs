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
use base::tcu::{ActId, EpId, TileId, KPEX_SEP};
use base::time::{CycleInstant, Duration};
use base::{cfg, kif, tcu};

use crate::arch::ktcu::{self, KPEX_EP};
use crate::ktcu::gen_key_kernel;
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

pub fn attest_tile_master(tile: TileId) {
    klog!(DEF, "Attesting tile: {}", tile);
    // Start cycle counter
    let start_cycles = CycleInstant::now();
    let mut kern_chain_info = AttestInfo::new();
    let mut kern_nonce: [u8; 16] = [0; 16];

    // Generate a random nonce
    if let Ok(_) = generate_random_nonce(&mut kern_nonce) {
        // klog!(DEF, "generate random nonce ok");
        kern_chain_info.nonce = kern_nonce;
    }
    else {
        // klog!(DEF, "generate random nonce Err");
        kern_chain_info.nonce = [2 as u8; 16];
    }
    // klog!(DEF, "Generated nonce: {:?}", kern_chain_info.nonce);

    // Generate an ecdsa signature
    let mut dummy_signature: [u8; 64] = [0; 64];
    let mut dummy_priv_key: [u8; 32] = [0; 32];
    let sign_data = generate_signature_data(&dummy_priv_key, &kern_nonce);
    if let Err(_) = generate_ecdsa_signature(&sign_data, &mut dummy_signature) {
        // klog!(DEF, "generate ecdsa sign err");
    }
    // klog!(DEF, "Generated ecdsa sign: {:?}", dummy_signature);

    // Verify ecdsa signature
    let dummy_msg: [u8; 16] = [0 as u8; 16];
    let dummy_pub_key: [u8; 64] = [0 as u8; 64];
    let dummy_signature: [u8; 64] = [0 as u8; 64];
    let verif_data = generate_verif_data(&dummy_msg, &dummy_signature, &dummy_pub_key);
    match verify_ecdsa_signature(16, &dummy_signature) {
        // Ok(_) => klog!(DEF, "ECDSA signature verification successful!"),
        Ok(_) => (),
        Err(_) => klog!(DEF, "ECDSA signature verification not successful"),
    };

    // Perform attestation
    attest_tile(tile, &mut kern_chain_info);

    let end_cycles = start_cycles.elapsed();

    klog!(
        DEF,
        "Attestation complete: {}, {} cycles",
        tile,
        end_cycles.as_raw()
    );
}

fn deprivilege_tiles() {
    let mut kern_chain_info = AttestInfo::new();
    let mut kern_nonce: [u8; 16] = [0; 16];

    for tile in platform::user_tiles() {
        attest_tile_master(tile);
        // Take away kernel privileges from other tiles
        // TODO: Move this after kernel has configure local ICU to be attested
        ktcu::deprivilege_tile(tile).expect("Unable to deprivilege tile");
    }

    // let mem = crate::mem::borrow_mut();

    // for m in mem.mods() {
    //     // Attest memory tile
    //     attest_tile_master(m.addr().tile());
    //     // attest_tile(m.addr().tile(), &mut kern_chain_info);
    // }

    att_done();
}

const CERT_LEN: u64 = 128;
const KERNEL_CHAIN_LEN: u64 = 3;
const ICU_CHAIN_LEN: u8 = 1;
const KERN_NONCE: [u8; 16] = [1; 16];
const CA_PUB_KEY: [u8; 64] = [0; 64];
const KERNEL_PRIV_KEY: [u8; 32] = [0; 32];
const ECDH_KEY: [u8; 32] = [0; 32];

struct AttestInfo {
    nonce: [u8; 16],
    kern_key: [u8; 32],
    icu_key: [u8; 32],
    chain_length: u64,
    reply_ep: u64,
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
            kern_key: ECDH_KEY,
            icu_key: ECDH_KEY,
            chain_length: KERNEL_CHAIN_LEN,
            reply_ep: KPEX_SEP as u64,
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

fn att_done() -> Result<(), Error> {
    base::tcu::TCU::att_done()?;
    Ok(())
}

fn attest_tile(tile: TileId, attest_info: &mut AttestInfo) -> Result<(), Error> {
    // Generate nonce material
    generate_random_nonce(&mut attest_info.nonce);

    // Generate signed nonce
    let kern_nonce = generate_signature_data(&KERNEL_PRIV_KEY, &attest_info.nonce);
    let mut kern_nonce_ecdsa_sign: [u8; 64] = [0; 64];
    generate_ecdsa_signature(&kern_nonce, &mut kern_nonce_ecdsa_sign);

    // Generate key exchange material
    generate_random_nonce(&mut attest_info.kern_key[..16]);
    generate_random_nonce(&mut attest_info.kern_key[16..]);

    // Generate signed key material
    let kern_key_material = generate_signature_data(&KERNEL_PRIV_KEY, &attest_info.kern_key);
    let mut kern_key_material_ecdsa_sign: [u8; 64] = [0; 64];
    generate_ecdsa_signature(&kern_key_material, &mut kern_key_material_ecdsa_sign);

    // Write nonce to remote ICU
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64,
        kern_nonce.as_ptr() as *const u8,
        16,
    );

    // Write nonce signature to remote ICU
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 16,
        kern_nonce_ecdsa_sign.as_ptr() as *const u8,
        64,
    );

    // Write key material
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 80,
        kern_key_material.as_ptr() as *const u8,
        32,
    );

    // Write signature of key material
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 112,
        kern_key_material_ecdsa_sign.as_ptr() as *const u8,
        64,
    );

    // Write chain length to remote ICU
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 176,
        &(3_u64.to_le_bytes()) as *const u8,
        8,
    );

    // Write reply endpoint id to remote ICU
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 184,
        &(attest_info.reply_ep.to_le_bytes()) as *const u8,
        8,
    );

    // Write chain to remote ICU
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 192,
        attest_info.certificate_chain.as_ptr() as *const u8,
        attest_info.certificate_chain.len(),
    );

    // Create receive endpoint locally and send endpoint at remote tile
    crate::ktcu::config_remote_ep(tile, tcu::KPEX_SEP, |regs| {
        crate::ktcu::config_send(
            regs,
            kif::tilemux::ACT_ID as ActId,
            tile as tcu::Label,
            platform::kernel_tile(),
            crate::ktcu::KPEX_EP,
            cfg::KPEX_RBUF_ORD,
            1,
        )
    })
    .unwrap();

    // Start the attestation
    crate::ktcu::attest_tile_remote(tile, 0).unwrap();

    // Use Tcu::wait_for_msg to sleep until a message arrives
    tcu::TCU::sleep().unwrap();
    //klog!(DEF, "Kernel woken up!");
    if let Some(msg) = crate::ktcu::fetch_msg(crate::ktcu::KPEX_EP) {
        // klog!(DEF, "Attestation message arrived: {:?}", msg);
        crate::ktcu::ack_msg(crate::ktcu::KPEX_EP, msg);
    }

    // TODO: Change stuff below this

    // Read ICU's signed nonce and challenge nonce
    let mut icu_nonce = [0 as u8; 16];
    let mut icu_signed_nonce = [0 as u8; 64];
    let mut icu_cert_chain = [0 as u8; 128];
    let mut icu_key_material = [0 as u8; 32];
    let mut icu_key_material_sign = [0 as u8; 64];
    crate::ktcu::read_slice(tile, base::tcu::TCU::attest_addr() as u64, &mut icu_nonce);
    crate::ktcu::read_slice(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 16,
        &mut icu_signed_nonce,
    );
    crate::ktcu::read_slice(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 80,
        &mut icu_cert_chain,
    );
    crate::ktcu::read_slice(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 80 + 128,
        &mut icu_key_material,
    );
    crate::ktcu::read_slice(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 80 + 128 + 32,
        &mut icu_key_material_sign,
    );

    // TODO: Use CA's public key
    let verif_data = generate_verif_data(
        &icu_cert_chain[0..64],
        &icu_cert_chain[64..128],
        &CA_PUB_KEY,
    );

    // Verify ICU public key
    match verify_ecdsa_signature(64, &verif_data) {
        //Ok(_) => klog!(DEF, "ICU cert ECDSA signature verification successful!"),
        Ok(_) => (),
        Err(_) => klog!(DEF, "ICU cert ECDSA signature verification not successful"),
    };

    // Verify ICU signed nonce
    let verif_data = generate_verif_data(
        &attest_info.nonce,
        &icu_signed_nonce,
        &icu_cert_chain[0..64],
    );

    // Verify ICU signed nonce
    match verify_ecdsa_signature(16, &verif_data) {
        // Ok(_) => klog!(DEF, "ICU nonce ECDSA signature verification successful!"),
        Ok(_) => (),
        Err(_) => klog!(DEF, "ICU nonce ECDSA signature verification not successful"),
    };

    // Verify ICU signed key material
    let verif_data = generate_verif_data(
        &icu_key_material,
        &icu_key_material_sign,
        &icu_cert_chain[0..64],
    );

    match verify_ecdsa_signature(32, &verif_data) {
        // Ok(_) => klog!(DEF, "ICU nonce ECDSA signature verification successful!"),
        Ok(_) => (),
        Err(_) => klog!(
            DEF,
            "ICU key material ECDSA signature verification not successful"
        ),
    };

    // Sign challenge nonce from ICU and write to the ICU
    // TODO: Use kernel private key
    let sign_data = generate_signature_data(&KERNEL_PRIV_KEY, &icu_nonce);
    let mut kern_nonce_signature: [u8; 64] = [0; 64];
    if let Err(_) = generate_ecdsa_signature(&sign_data, &mut kern_nonce_signature) {
        klog!(DEF, "generate ecdsa sign err");
    }

    // klog!(DEF, "Writing signed ICU challenge to ICU");
    // Write chain to remote ICU
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 32 + 512,
        kern_nonce_signature.as_ptr() as *const u8,
        64,
    );

    // Write reply endpoint id to remote ICU
    // klog!(DEF, "reply ep: {}", attest_info.reply_ep);
    crate::ktcu::write_mem(
        tile,
        base::tcu::TCU::attest_addr() as u64 + 184,
        &(attest_info.reply_ep.to_le_bytes()) as *const u8,
        8,
    );

    // Notify ICU of the write through ATTEST cmd, arg: 1
    // TODO: Agree upon ECDH parameters: m, f(x), a, b, G, n, h
    crate::ktcu::gen_key_tile_remote(tile).unwrap();

    // Wait for ICU to acknowledge the key exchange
    tcu::TCU::sleep().unwrap();
    // klog!(DEF, "Kernel woken up!");
    if let Some(msg) = crate::ktcu::fetch_msg(crate::ktcu::KPEX_EP) {
        // klog!(DEF, "Key generated at remote ICU: {:?}", msg);
        crate::ktcu::ack_msg(crate::ktcu::KPEX_EP, msg);
    }

    crate::ktcu::invalidate_ep_remote(tile, tcu::KPEX_SEP, true).unwrap();

    gen_key_kernel(&mut attest_info.kern_key, &icu_key_material);

    Ok(())
}
