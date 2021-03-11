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

use base::col::{BitVec, Vec};
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::mem::GlobAddr;
use base::rc::{Rc, SRc, Weak};
use base::mem::{MsgBuf, MsgBufRef};
use base::tcu::{self, EpId, PEId, VPEId};
use core::cmp;

use crate::cap::{EPObject, MGateObject, PEObject, RGateObject, SGateObject};
use crate::ktcu;
use crate::pes::INVAL_ID;
use crate::platform;

pub struct PEMux {
    pe: SRc<PEObject>,
    vpes: Vec<VPEId>,
    #[cfg(target_os = "none")]
    queue: crate::com::SendQueue,
    pmp: Vec<Rc<EPObject>>,
    eps: BitVec,
    #[allow(dead_code)]
    mem_base: goff,
}

impl PEMux {
    pub fn new(pe: PEId) -> Self {
        let pe_obj = PEObject::new(pe, (tcu::AVAIL_EPS - tcu::FIRST_USER_EP) as u32, false);

        // create PMP EPObjects for this PE
        let mut pmp = Vec::new();
        for ep in 0..tcu::PMEM_PROT_EPS as EpId {
            pmp.push(EPObject::new(false, Weak::new(), ep, 0, &pe_obj));
        }

        let mut pemux = PEMux {
            pe: pe_obj,
            vpes: Vec::new(),
            #[cfg(target_os = "none")]
            queue: crate::com::SendQueue::new(pe as u64, pe),
            pmp,
            eps: BitVec::new(tcu::AVAIL_EPS as usize),
            mem_base: 0,
        };

        #[cfg(target_os = "none")]
        pemux.eps.set(0); // first EP is reserved for PEMux's memory region

        for ep in tcu::PMEM_PROT_EPS as EpId..tcu::FIRST_USER_EP {
            pemux.eps.set(ep as usize);
        }

        #[cfg(target_os = "none")]
        if platform::pe_desc(pe).supports_pemux() {
            pemux.init();
        }

        pemux
    }

    pub fn has_vpes(&self) -> bool {
        !self.vpes.is_empty()
    }

    pub fn add_vpe(&mut self, vpe: VPEId) {
        self.vpes.push(vpe);
    }

    pub fn rem_vpe(&mut self, vpe: VPEId) {
        assert!(!self.vpes.is_empty());
        self.vpes.retain(|id| *id != vpe);
    }

    #[cfg(target_os = "none")]
    fn init(&mut self) {
        use base::cfg;

        // configure send EP
        ktcu::config_remote_ep(self.pe_id(), tcu::KPEX_SEP, |regs| {
            ktcu::config_send(
                regs,
                kif::pemux::VPE_ID as VPEId,
                self.pe_id() as tcu::Label,
                platform::kernel_pe(),
                ktcu::KPEX_EP,
                cfg::KPEX_RBUF_ORD,
                1,
            );
        })
        .unwrap();

        // configure receive EP
        let mut rbuf = platform::rbuf_pemux(self.pe_id());
        ktcu::config_remote_ep(self.pe_id(), tcu::KPEX_REP, |regs| {
            ktcu::config_recv(
                regs,
                kif::pemux::VPE_ID as VPEId,
                rbuf,
                cfg::KPEX_RBUF_ORD,
                cfg::KPEX_RBUF_ORD,
                None,
            );
        })
        .unwrap();
        rbuf += 1 << cfg::KPEX_RBUF_ORD;

        // configure upcall EP
        ktcu::config_remote_ep(self.pe_id(), tcu::PEXSIDE_REP, |regs| {
            ktcu::config_recv(
                regs,
                kif::pemux::VPE_ID as VPEId,
                rbuf,
                cfg::PEXUP_RBUF_ORD,
                cfg::PEXUP_RBUF_ORD,
                Some(tcu::PEXSIDE_RPLEP),
            );
        })
        .unwrap();
    }

    pub fn pe(&self) -> &SRc<PEObject> {
        &self.pe
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.pe()
    }

    #[cfg(target_os = "linux")]
    pub fn eps_base(&mut self) -> goff {
        self.mem_base
    }

    pub fn set_mem_base(&mut self, addr: goff) {
        self.mem_base = addr;
    }

    pub fn pmp_ep(&self, ep: EpId) -> &Rc<EPObject> {
        &self.pmp[ep as usize]
    }

    pub fn find_eps(&self, count: u32) -> Result<EpId, Error> {
        // the PMP EPs cannot be allocated
        let mut start = cmp::max(tcu::FIRST_USER_EP as usize, self.eps.first_clear());
        let mut bit = start;
        while bit < start + count as usize && bit < tcu::AVAIL_EPS as usize {
            if self.eps.is_set(bit) {
                start = bit + 1;
            }
            bit += 1;
        }

        if bit != start + count as usize {
            Err(Error::new(Code::NoSpace))
        }
        else {
            Ok(start as EpId)
        }
    }

    pub fn eps_free(&self, start: EpId, count: u32) -> bool {
        for ep in start..start + count as EpId {
            if self.eps.is_set(ep as usize) {
                return false;
            }
        }
        true
    }

    pub fn alloc_eps(&mut self, start: EpId, count: u32) {
        klog!(
            EPS,
            "PEMux[{}] allocating EPS {}..{}",
            self.pe_id(),
            start,
            start as u32 + count - 1
        );
        for bit in start..start + count as EpId {
            assert!(!self.eps.is_set(bit as usize));
            self.eps.set(bit as usize);
        }
    }

    pub fn free_eps(&mut self, start: EpId, count: u32) {
        klog!(
            EPS,
            "PEMux[{}] freeing EPS {}..{}",
            self.pe_id(),
            start,
            start as u32 + count - 1
        );
        for bit in start..start + count as EpId {
            assert!(self.eps.is_set(bit as usize));
            self.eps.clear(bit as usize);
        }
    }

    fn ep_vpe_id(&self, vpe: VPEId) -> VPEId {
        match platform::is_shared(self.pe_id()) {
            true => vpe,
            false => INVAL_ID,
        }
    }

    pub fn config_snd_ep(
        &mut self,
        ep: EpId,
        vpe: VPEId,
        obj: &SRc<SGateObject>,
    ) -> Result<(), Error> {
        let rgate = obj.rgate();
        assert!(rgate.activated());

        klog!(EPS, "PE{}:EP{} = {:?}", self.pe_id(), ep, obj);

        ktcu::config_remote_ep(self.pe_id(), ep, |regs| {
            let vpe = self.ep_vpe_id(vpe);
            let (rpe, rep) = rgate.location().unwrap();
            ktcu::config_send(
                regs,
                vpe,
                obj.label(),
                rpe,
                rep,
                rgate.msg_order(),
                obj.credits(),
            );
        })
    }

    pub fn config_rcv_ep(
        &mut self,
        ep: EpId,
        vpe: VPEId,
        reply_eps: Option<EpId>,
        obj: &SRc<RGateObject>,
    ) -> Result<(), Error> {
        klog!(EPS, "PE{}:EP{} = {:?}", self.pe_id(), ep, obj);

        ktcu::config_remote_ep(self.pe_id(), ep, |regs| {
            let vpe = self.ep_vpe_id(vpe);
            ktcu::config_recv(
                regs,
                vpe,
                obj.addr(),
                obj.order(),
                obj.msg_order(),
                reply_eps,
            );
        })?;

        thread::ThreadManager::get().notify(obj.get_event(), None);
        Ok(())
    }

    pub fn config_mem_ep(
        &mut self,
        ep: EpId,
        vpe: VPEId,
        obj: &SRc<MGateObject>,
        pe_id: PEId,
    ) -> Result<(), Error> {
        if ep < tcu::PMEM_PROT_EPS as EpId {
            klog!(EPS, "PE{}:PMPEP{} = {:?}", self.pe_id(), ep, obj);
        }
        else {
            klog!(EPS, "PE{}:EP{} = {:?}", self.pe_id(), ep, obj);
        }

        ktcu::config_remote_ep(self.pe_id(), ep, |regs| {
            let vpe = self.ep_vpe_id(vpe);
            ktcu::config_mem(
                regs,
                vpe,
                pe_id,
                obj.offset(),
                obj.size() as usize,
                obj.perms(),
            );
        })
    }

    pub fn invalidate_ep(
        &mut self,
        vpe: VPEId,
        ep: EpId,
        force: bool,
        notify: bool,
    ) -> Result<(), Error> {
        klog!(EPS, "PE{}:EP{} = invalid", self.pe_id(), ep);

        let unread = ktcu::invalidate_ep_remote(self.pe_id(), ep, force)?;
        if unread != 0 && notify {
            let mut msg = MsgBuf::borrow_def();
            msg.set(kif::pemux::RemMsgs {
                op: kif::pemux::Sidecalls::REM_MSGS.val as u64,
                vpe_sel: vpe as u64,
                unread_mask: unread as u64,
            });

            self.send_sidecall::<kif::pemux::RemMsgs>(Some(vpe), &msg)
                .map(|_| ())
        }
        else {
            Ok(())
        }
    }

    pub fn invalidate_reply_eps(
        &self,
        recv_pe: PEId,
        recv_ep: EpId,
        send_ep: EpId,
    ) -> Result<(), Error> {
        klog!(
            EPS,
            "PE{}:EP{} = invalid reply EPs at PE{}:EP{}",
            self.pe_id(),
            send_ep,
            recv_pe,
            recv_ep
        );

        ktcu::inv_reply_remote(recv_pe, recv_ep, self.pe_id(), send_ep)
    }
}

#[cfg(target_os = "none")]
impl PEMux {
    pub fn handle_call_async(&mut self, msg: &tcu::Message) {
        use crate::pes::VPEMng;

        let req = msg.get_data::<kif::pemux::Exit>();
        let vpe_id = req.vpe_sel as VPEId;
        let exitcode = req.code as i32;

        klog!(PEXC, "PEMux[{}] received {:?}", self.pe_id(), req);

        if self.vpes.contains(&vpe_id) {
            let vpe = VPEMng::get().vpe(vpe_id).unwrap();
            vpe.stop_app_async(exitcode, true);
        }

        let mut reply = MsgBuf::borrow_def();
        reply.set(kif::DefaultReply { error: 0 });
        ktcu::reply(ktcu::KPEX_EP, &reply, msg).unwrap();
    }

    pub fn vpe_ctrl_async(
        &mut self,
        vpe: VPEId,
        eps_start: EpId,
        ctrl: base::kif::pemux::VPEOp,
    ) -> Result<(), Error> {
        let mut msg = MsgBuf::borrow_def();
        msg.set(kif::pemux::VPECtrl {
            op: kif::pemux::Sidecalls::VPE_CTRL.val as u64,
            vpe_sel: vpe as u64,
            vpe_op: ctrl.val as u64,
            eps_start: eps_start as u64,
        });

        self.send_receive_sidecall_async::<kif::pemux::VPECtrl>(None, msg)
            .map(|_| ())
    }

    pub fn map_async(
        &mut self,
        vpe: VPEId,
        virt: goff,
        glob: GlobAddr,
        pages: usize,
        perm: kif::PageFlags,
    ) -> Result<(), Error> {
        let mut msg = MsgBuf::borrow_def();
        msg.set(kif::pemux::Map {
            op: kif::pemux::Sidecalls::MAP.val as u64,
            vpe_sel: vpe as u64,
            virt: virt as u64,
            global: glob.raw(),
            pages: pages as u64,
            perm: perm.bits() as u64,
        });

        self.send_receive_sidecall_async::<kif::pemux::Map>(Some(vpe), msg)
            .map(|_| ())
    }

    pub fn unmap_async(&mut self, vpe: VPEId, virt: goff, pages: usize) -> Result<(), Error> {
        self.map_async(vpe, virt, GlobAddr::new(0), pages, kif::PageFlags::empty())
    }

    pub fn translate_async(
        &mut self,
        vpe: VPEId,
        virt: goff,
        perm: kif::Perm,
    ) -> Result<GlobAddr, Error> {
        use base::cfg::PAGE_MASK;

        let mut msg = MsgBuf::borrow_def();
        msg.set(kif::pemux::Translate {
            op: kif::pemux::Sidecalls::TRANSLATE.val as u64,
            vpe_sel: vpe as u64,
            virt: virt as u64,
            perm: perm.bits() as u64,
        });

        self.send_receive_sidecall_async::<kif::pemux::Translate>(Some(vpe), msg)
            .map(|reply| GlobAddr::new(reply.val & !(PAGE_MASK as goff)))
    }

    pub fn notify_invalidate(&mut self, vpe: VPEId, ep: EpId) -> Result<(), Error> {
        let mut msg = MsgBuf::borrow_def();
        msg.set(kif::pemux::EpInval {
            op: kif::pemux::Sidecalls::EP_INVAL.val as u64,
            vpe_sel: vpe as u64,
            ep: ep as u64,
        });

        self.send_sidecall::<kif::pemux::EpInval>(Some(vpe), &msg)
            .map(|_| ())
    }

    fn send_sidecall<R: core::fmt::Debug>(
        &mut self,
        vpe: Option<VPEId>,
        req: &MsgBuf,
    ) -> Result<thread::Event, Error> {
        use crate::pes::{State, VPEMng};

        // if the VPE has no app anymore, don't send the notify
        if let Some(id) = vpe {
            if !VPEMng::get()
                .vpe(id)
                .map(|v| v.state() != State::DEAD)
                .unwrap_or(false)
            {
                return Err(Error::new(Code::VPEGone));
            }
        }

        klog!(PEXC, "PEMux[{}] sending {:?}", self.pe_id(), req.get::<R>());

        self.queue.send(tcu::PEXSIDE_REP, 0, req)
    }

    fn send_receive_sidecall_async<R: core::fmt::Debug>(
        &mut self,
        vpe: Option<VPEId>,
        req: MsgBufRef<'_>,
    ) -> Result<&'static kif::pemux::Response, Error> {
        use crate::com::SendQueue;

        let event = self.send_sidecall::<R>(vpe, &req)?;
        drop(req);

        let reply = SendQueue::receive_async(event)?;

        let reply = reply.get_data::<kif::pemux::Response>();
        if reply.error == 0 {
            Ok(reply)
        }
        else {
            Err(Error::new(Code::from(reply.error as u32)))
        }
    }
}

#[cfg(target_os = "linux")]
impl PEMux {
    pub fn update_eps(&mut self) -> Result<(), Error> {
        ktcu::update_eps(self.pe_id(), self.mem_base)
    }

    pub fn vpe_ctrl_async(
        &mut self,
        _vpe: VPEId,
        _eps_start: EpId,
        _ctrl: base::kif::pemux::VPEOp,
    ) -> Result<(), Error> {
        Ok(())
    }

    pub fn map_async(
        &mut self,
        _vpe: VPEId,
        _virt: goff,
        _glob: GlobAddr,
        _pages: usize,
        _perm: kif::PageFlags,
    ) -> Result<(), Error> {
        Ok(())
    }

    pub fn unmap_async(&mut self, _vpe: VPEId, _virt: goff, _pages: usize) -> Result<(), Error> {
        Ok(())
    }

    pub fn notify_invalidate(&mut self, _vpe: VPEId, _ep: EpId) -> Result<(), Error> {
        Ok(())
    }

    fn send_sidecall<R: core::fmt::Debug>(
        &mut self,
        _vpe: Option<VPEId>,
        _req: &MsgBuf,
    ) -> Result<thread::Event, Error> {
        Err(Error::new(Code::NotSup))
    }
}
