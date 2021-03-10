/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use base::const_assert;
use m3::goff;
use m3::mem;

use bitflags::bitflags;

bitflags! {
    pub struct E1000_REG: goff {
        const CTRL            = 0x0;           /* device control register */
        const STATUS          = 0x8;           /* device status register */
        const EECD            = 0x10;          /* EEPROM control/data register */
        const EERD            = 0x14;          /* EEPROM read register */
        const VET             = 0x38;          /* VLAN ether type */

        const ICR             = 0xc0;          /* interrupt cause read register */
        const IMS             = 0xd0;          /* interrupt mask set/read register */
        const IMC             = 0xd8;          /* interrupt mask clear register */

        const RCTL            = 0x100;         /* receive control register */
        const TCTL            = 0x400;         /* transmit control register */

        const PBA             = 0x1000;        /* packet buffer allocation */
        const PBS             = 0x1008;        /* packet buffer size */

        const RDBAL           = 0x2800;        /* register descriptor base address low */
        const RDBAH           = 0x2804;        /* register descriptor base address high */
        const RDLEN           = 0x2808;        /* register descriptor length */
        const RDH             = 0x2810;        /* register descriptor head */
        const RDT             = 0x2818;        /* register descriptor tail */

        const RDTR            = 0x2820;        /* receive delay timer register */
        const RDCTL           = 0x2828;        /* transmit descriptor control */
        const RADV            = 0x282c;        /* receive absolute interrupt delay timer */

        const TDBAL           = 0x3800;        /* transmit descriptor base address low */
        const TDBAH           = 0x3804;        /* transmit descriptor base address high */
        const TDLEN           = 0x3808;        /* transmit descriptor length */
        const TDH             = 0x3810;        /* transmit descriptor head */
        const TDT             = 0x3818;        /* transmit descriptor tail */

        const TIDV            = 0x3820;        /* transmit interrupt delay value */
        const TDCTL           = 0x3828;        /* transmit descriptor control */
        const TADV            = 0x382c;        /* transmit absolute interrupt delay timer */

        const RAL             = 0x5400;        /* filtering: receive address low */
        const RAH             = 0x5404;        /* filtering: receive address high */

        const RXCSUM          = 0x5000;        /* receive checksum control */
    }
}

bitflags! {
    pub struct E1000_STATUS: u8 {
        const LU              = 1 << 1;        /* link up */
    }
}

bitflags! {
    pub struct E1000_CTL: u32 {
        const LRST            = 1 << 3;        /* link reset */
        const ASDE            = 1 << 5;        /* auto speed detection enable */
        const SLU             = 1 << 6;        /* set link up */
        const FRCSPD          = 1 << 11;       /* force speed */
        const FRCDPLX         = 1 << 12;       /* force duplex */
        const RESET           = 1 << 26;       /* 1 = device reset; self-clearing */
        const PHY_RESET       = 1 << 31;       /* 1 = PHY reset */
    }
}

bitflags! {
    pub struct E1000_XDCTL: u32 {
        const XDCTL_ENABLE    = 1 << 25;       /* queue enable */
    }
}

bitflags! {
    pub struct E1000_ICR: u8 {
        const LSC             = 1 << 2;        /* Link Status Change */
        const RXDMT0          = 1 << 4;        /* Receive Descriptor Minimum Threshold Reached */
        const RXO             = 1 << 6;        /* Receiver Overrun */
        const RXT0            = 1 << 7;        /* Receiver Timer Interrupt */
    }
}

bitflags! {
    pub struct E1000_RCTL: u32 {
        const ENABLE          = 1 << 1;
        const UPE             = 1 << 3;        /* unicast promiscuous mode */
        const MPE             = 1 << 4;        /* multicast promiscuous */
        const BAM             = 1 << 15;       /* broadcasts accept mode */
        const BSIZE_256       = 0x11 << 16;    /* receive buffer size = 256 bytes (if RCTL_BSEX = 0) */
        const BSIZE_512       = 0x10 << 16;    /* receive buffer size = 512 bytes (if RCTL_BSEX = 0) */
        const BSIZE_1K        = 0x01 << 16;    /* receive buffer size = 1024 bytes (if RCTL_BSEX = 0) */
        const BSIZE_2K        = 0x00 << 16;    /* receive buffer size = 2048 bytes (if RCTL_BSEX = 0) */
        const BSIZE_MASK      = 0x11 << 16;    /* mask for buffer size */
        const BSEX_MASK       = 0x01 << 25;    /* mask for size extension */
        const SECRC           = 1 << 26;       /* strip CRC */
    }
}

bitflags! {
    pub struct E1000_TCTL: u32 {
        const ENABLE          = 1 << 1;
        const PSP             = 1 << 3;        /* pad short packets */
        const COLL_TSH        = 0x0F << 4;     /* collision threshold; number of transmission attempts */
        const COLL_DIST       = 0x40 << 12;    /* collision distance; pad packets to X bytes; 64 here */
        const COLT_MASK       = 0xff << 4;
        const COLD_MASK       = 0x3ff << 12;
    }
}

bitflags! {
    pub struct E1000_RAH: u32 {
        const VALID           = 1 << 31;       /* marks a receive address filter as valid */
    }
}

bitflags! {
    pub struct E1000_RXCSUM: u16 {
        const PCSS_MASK       = 0xff;           /* Packet Checksum Start */
        const IPOFLD          = 1 << 8;         /* IP Checksum Off-load Enable */
        const TUOFLD          = 1 << 9;         /* TCP/UDP Checksum Off-load Enable */
        const IPV6OFL         = 1 << 10;        /* IPv6 Checksum Offload Enable */
    }
}

bitflags! {
    pub struct E1000_EEPROM: u8 {
        const OFS_MAC         = 0x0;           /* offset of the MAC in EEPROM */
    }
}

bitflags! {
    pub struct E1000_EERD: u8 {
        const START           = 1 << 0;        /* start command */
        const DONE_SMALL      = 1 << 4;        /* read done (small EERD) */
        const DONE_LARGE      = 1 << 1;        /* read done (large EERD) */
        const SHIFT_SMALL     = 8;             /* address shift (small) */
        const SHIFT_LARGE     = 2;             /* address shift (large) */
    }
}

bitflags! {
    pub struct E1000_TX: u8 {
        const CMD_EOP         = 0x01;          /* end of packet */
        const CMD_IFCS        = 0x02;          /* insert FCS/CRC */
    }
}

bitflags! {
    pub struct E1000_RXDS: u8 {
        const PIF             = 1 << 7; /* Passed in-exact filter */
        const IPCS            = 1 << 6; /* IP Checksum Calculated on Packet */
        const TCPCS           = 1 << 5; /* TCP Checksum Calculated on Packet */
        // Only in gem5 i8254xGBE?!
        const UDPCS           = 1 << 4; /* TCP Checksum Calculated on Packet */
        const VP              = 1 << 3; /* Packet is 802.1Q (matched VET) */
        const IXSM            = 1 << 2; /* Ignore Checksum Indication */
        const EOP             = 1 << 1; /* End of Packet */
        const DD              = 1 << 0; /* receive descriptor status; indicates that the HW has
         * finished the descriptor */
    }
}

bitflags! {
    pub struct E1000_RXDE: u8 {
        const RXE             = 1 << 7; /* RX Data Error */
        const IPE             = 1 << 6; /* IP Checksum Error */
        const TCPE            = 1 << 5; /* TCP/UDP Checksum Error */
        const SEQ             = 1 << 2; /* Sequence Error */
        const SE              = 1 << 1; /* Symbol Error */
        const CE              = 1 << 0; /* CRC Error or Alignment Error */
    }
}

/*
bitflags!{
    pub struct TxoProto: u8{
    const Unsupported = 1<<1;
    const IP = 1<<2 | TxoProto::Unsupported.bits();
    const UDP = 1<<3 | TxoProto::IP.bits();
    const TCP = 1<<4 | TxoProto::IP.bits();
    }
}
*/

bitflags! {
    pub struct TxoProto: u8 {
        const Unsupported     = 1 << 1;
        const IP              = 1 << 2 | TxoProto::Unsupported.bits();
        const UDP             = 1 << 3 | TxoProto::IP.bits();
        const TCP             = 1 << 4 | TxoProto::IP.bits();
    }
}

pub const IP_PROTO_UDP: u8 = 17;
pub const IP_PROTO_TCP: u8 = 6;

pub const TCP_CHECKSUM_OFFSET: u8 = 0x10;
pub const UDP_CHECKSUM_OFFSET: u8 = 0x06;

pub const ETH_HWADDR_LEN: usize = 6;
pub const ETHTYPE_IP: u16 = 0x0008;

pub const WORD_LEN_LOG2: usize = 1;
// TODO: Use a sensible value, the current one is chosen arbitrarily
pub const MAX_WAIT_NANOS: u64 = 100000;

pub const RESET_SLEEP_TIME: u64 = 20 * 1000;

pub const MAX_RECEIVE_COUNT_PER_INTERRUPT: usize = 5;

pub const RX_BUF_COUNT: usize = 256;
pub const TX_BUF_COUNT: usize = 256;
pub const RX_BUF_SIZE: usize = 2048;
pub const TX_BUF_SIZE: usize = 2048;

#[repr(C, align(4))]
pub struct TxDesc {
    pub buffer: u64,
    pub length: u16,
    pub checksumOffset: u8,
    pub cmd: u8,
    pub status: u8,
    pub checksumStart: u8,
    pub pad: u16,
}

// TODO: Allocation details of bit fields are implementation-defined...
#[repr(C, align(4))]
pub struct TxContextDesc {
    pub IPCSS: u8,
    pub IPCSO: u8,
    pub IPCSE: u16,
    pub TUCSS: u8,
    pub TUCSO: u8,
    pub TUCSE: u16,

    //20bits PAYLEN, then 4bit DTYP then 8bit TUCMD, use getter/setter to get those
    pub PAYLEN_DTYP_TUCMD: u32,
    //First 4 are STA, second 4 are RSV, use getter/setter to get those
    pub STA_RSV: u8,
    pub HDRLEN: u8,
    pub MSS: u16,
}

impl TxContextDesc {
    pub fn set_paylen(&mut self, paylen: u32) {
        assert!(paylen < (1 << 20));
        self.PAYLEN_DTYP_TUCMD =
            (self.PAYLEN_DTYP_TUCMD & 0xff_f0_00_00) | (paylen & 0x00_0f_ff_ff);
    }

    //sets the 4bits of the dtyp
    pub fn set_dtyp(&mut self, dtyp: u8) {
        self.PAYLEN_DTYP_TUCMD =
            (self.PAYLEN_DTYP_TUCMD & 0xff_0f_ff_ff) | (((dtyp as u32) << 20) & 0x00_f0_00_00);
    }

    pub fn set_tucmd(&mut self, tucmd: u8) {
        self.PAYLEN_DTYP_TUCMD =
            (self.PAYLEN_DTYP_TUCMD & 0x00_ff_ff_ff) | (((tucmd as u32) << 24) & 0xff_00_00_00);
    }

    pub fn set_sta(&mut self, sta: u8) {
        self.STA_RSV = (self.STA_RSV & 0xf0) | (sta & 0x0f);
    }

    pub fn set_rsv(&mut self, rsv: u8) {
        self.STA_RSV = (self.STA_RSV & 0x0f) | (rsv & 0xf0);
    }
}

#[repr(C, align(4))]
pub struct TxDataDesc {
    pub buffer: u64,
    //first 20bits are length, then 4 bits DTYP, then 8bits DCMD, use getter/setter
    pub length_DTYP_DCMD: u32,
    //first 4bits are STA, second 4 bits are RSV
    pub STA_RSV: u8,
    pub POPTS: u8,
    pub special: u16,
}

impl TxDataDesc {
    pub fn set_length(&mut self, length: u32) {
        let new_length = (self.length_DTYP_DCMD & 0xff_f0_00_00) | (length & 0x00_0f_ff_ff);
        self.length_DTYP_DCMD = new_length;
    }

    //sets the 4bits of the dtyp
    pub fn set_dtyp(&mut self, dtyp: u8) {
        //self.length_DTYP_DCMD = (self.length_DTYP_DCMD & 0xff_ff_f0_ff) | (((dtyp as u32) << 20) & 0x00_00_0f_00);
        self.length_DTYP_DCMD =
            (self.length_DTYP_DCMD & 0xff_0f_ff_ff) | (((dtyp as u32) << 20) & 0x00_f0_00_00);
    }

    pub fn set_dcmd(&mut self, dcmd: u8) {
        self.length_DTYP_DCMD =
            (self.length_DTYP_DCMD & 0x00_ff_ff_ff) | (((dcmd as u32) << 24) & 0xff_00_00_00);
        //self.length_DTYP_DCMD = (self.length_DTYP_DCMD & 0xff_ff_ff_00) | (((dcmd as u32) << 24) & 0x00_00_00_ff);
    }

    pub fn set_sta(&mut self, sta: u8) {
        self.STA_RSV = (self.STA_RSV & 0x0f) | (sta & 0xf0);
    }

    pub fn set_rsv(&mut self, rsv: u8) {
        self.STA_RSV = (self.STA_RSV & 0xf0) | (rsv & 0x0f);
    }
}

//TODO ignore packed?
#[repr(C, align(4))]
pub struct RxDesc {
    pub buffer: u64,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub error: u8,
    pub pad: u16,
}

impl Default for RxDesc {
    fn default() -> Self {
        const_assert!(core::mem::size_of::<RxDesc>() == 16);
        RxDesc {
            buffer: 0,
            length: 0,
            checksum: 0,
            status: 0,
            error: 0,
            pad: 0,
        }
    }
}

#[repr(C, align(16))]
pub struct Buffers {
    pub rx_descs: [RxDesc; RX_BUF_COUNT], //TODO does the align16 work here? ALIGNED(16); // 0
    pub tx_descs: [TxDesc; TX_BUF_COUNT], // ALIGNED(16); // 128
    pub rx_buf: [u8; RX_BUF_COUNT * RX_BUF_SIZE], // 16384 + 256
    pub tx_buf: [u8; TX_BUF_COUNT * TX_BUF_SIZE],
}

//TODO not packed yet
#[repr(C)]
pub struct EthAddr(pub [u8; ETH_HWADDR_LEN]);
impl core::fmt::Display for EthAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Addr[{:x}, {:x}, {:x}, {:x}, {:x}, {:x}]",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}
//TODO not packed yet
#[repr(C)]
pub struct EthHdr {
    pub dest: EthAddr,
    pub src: EthAddr,
    pub ty: u16,
}

impl core::fmt::Display for EthHdr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        const_assert!(core::mem::size_of::<EthHdr>() == 14);
        write!(
            f,
            "ETH[dest={}, src={}, ty={:b}, size={}]",
            self.dest,
            self.src,
            self.ty,
            core::mem::size_of::<EthHdr>()
        )
    }
}

//TODO not packed yet
#[repr(C)]
pub struct Ip4Addr(pub u32);
impl core::fmt::Display for Ip4Addr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a, b, c, d] = self.0.to_be_bytes();
        write!(f, "Ipv4[{}, {}, {}, {}]", a, b, c, d)
    }
}

//TODO not packed yet
#[repr(C)]
pub struct IpHdr {
    pub v_hl: u8,
    pub tos: u8,
    pub len: u16,
    pub id: u16,
    pub offset: u16,
    pub ttl: u8,
    pub proto: u8,
    pub chksum: u16,
    pub src: Ip4Addr,
    pub dest: Ip4Addr,
}

impl core::fmt::Display for IpHdr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        const_assert!(core::mem::size_of::<IpHdr>() == 20);
        write!(
            f,
            "
ETH[
v_hl={},\
tos={},
len={},
id={},
offset={},
ttl={},
proto={:x},
chksum={:x},
src={},
dest={}]",
            self.v_hl,
            self.tos,
            self.len,
            self.id,
            self.offset,
            self.ttl,
            self.proto,
            self.chksum,
            self.src,
            self.dest,
        )
    }
}
