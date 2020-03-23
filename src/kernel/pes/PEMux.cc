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

#include <base/log/Kernel.h>

#include "pes/PEMux.h"
#include "pes/VPEManager.h"
#include "TCU.h"
#include "Platform.h"
#include "SyscallHandler.h"

namespace kernel {

PEMux::PEMux(peid_t pe)
    : _pe(new PEObject(pe, EP_COUNT - m3::TCU::FIRST_USER_EP)),
      _caps(VPE::INVALID_ID),
      _vpes(),
      _rbufs_size(),
      _mem_base(),
      _eps(),
      _upcqueue(desc()) {
    for(epid_t ep = 0; ep < m3::TCU::FIRST_USER_EP; ++ep)
        _eps.set(ep);

#if defined(__gem5__)
    if(Platform::pe(pe).supports_pemux()) {
        // configure send EP
        TCU::config_remote_ep(0, pe, m3::TCU::KPEX_SEP, [this](m3::TCU::reg_t *ep_regs) {
            TCU::config_send(ep_regs, m3::KIF::PEMUX_VPE_ID, m3::ptr_to_label(this),
                             Platform::kernel_pe(), TCU::PEX_REP, KPEX_RBUF_ORDER, 1);
        });

        // configure receive EP
        uintptr_t rbuf = PEMUX_RBUF_SPACE;
        TCU::config_remote_ep(0, pe, m3::TCU::KPEX_REP, [rbuf](m3::TCU::reg_t *ep_regs) {
            TCU::config_recv(ep_regs, m3::KIF::PEMUX_VPE_ID, rbuf,
                             KPEX_RBUF_ORDER, KPEX_RBUF_ORDER, m3::TCU::NO_REPLIES);
        });
        rbuf += KPEX_RBUF_SIZE;

        // configure upcall receive EP
        TCU::config_remote_ep(0, pe, m3::TCU::PEXUP_REP, [rbuf](m3::TCU::reg_t *ep_regs) {
            TCU::config_recv(ep_regs, m3::KIF::PEMUX_VPE_ID, rbuf,
                             PEXUP_RBUF_ORDER, PEXUP_RBUF_ORDER, m3::TCU::PEXUP_RPLEP);
        });
    }
    #endif
}

void PEMux::handle_call(const m3::TCU::Message *msg) {
    auto req = reinterpret_cast<const m3::KIF::PEXCalls::Exit*>(msg->data);
    capsel_t vpe = req->vpe_sel;
    int exitcode = req->code;

    KLOG(PEXC, "PEMux[" << peid() << "] got exit(vpe=" << vpe << ", code=" << exitcode << ")");

    auto vpecap = static_cast<VPECapability*>(_caps.get(vpe, Capability::VIRTPE));
    if(vpecap != nullptr) {
        vpecap->obj->_flags |= VPE::F_STOPPED;
        vpecap->obj->stop_app(exitcode, true);
    }

    // give credits back
    TCU::reply(TCU::PEX_REP, nullptr, 0, msg);
}

void PEMux::add_vpe(VPECapability *vpe) {
    assert(_vpes == 0);
    _caps.obtain(vpe->obj->id(), vpe);
    _vpes++;
}

void PEMux::remove_vpe(UNUSED VPE *vpe) {
    // has already been revoked
    assert(_caps.get(vpe->id(), Capability::VIRTPE) == nullptr);
    _vpes--;
    _rbufs_size = 0;
    _mem_base = 0;
}

epid_t PEMux::find_eps(uint count) const {
    uint bit, start = _eps.first_clear();
    for(bit = start; bit < start + count && bit < EP_COUNT; ++bit) {
        if(_eps.is_set(bit))
            start = bit + 1;
    }
    if(bit != start + count)
        return EP_COUNT;
    return start;
}

bool PEMux::eps_free(epid_t start, uint count) const {
    for(epid_t ep = start; ep < start + count; ++ep) {
        if(_eps.is_set(ep))
            return false;
    }
    return true;
}

void PEMux::alloc_eps(epid_t first, uint count) {
    KLOG(EPS, "PEMux[" << peid() << "] allocating EPs " << first << " .. " << (first + count - 1));
    for(uint bit = first; bit < first + count; ++bit)
        _eps.set(bit, true);
}

void PEMux::free_eps(epid_t first, uint count) {
    KLOG(EPS, "PEMux[" << peid() << "] freeing EPs " << first << ".." << (first + count - 1));

    for(epid_t ep = first; ep < first + count; ++ep) {
        assert(_eps.is_set(ep));
        _eps.clear(ep);
    }
}

m3::Errors::Code PEMux::map(vpeid_t vpe, goff_t virt, gaddr_t phys, uint pages, uint perm) {
    m3::KIF::PEXUpcalls::Map req;
    req.opcode = static_cast<xfer_t>(m3::KIF::PEXUpcalls::MAP);
    req.vpe_sel = vpe;
    req.virt = virt;
    req.phys = phys;
    req.pages = pages;
    req.perm = static_cast<xfer_t>(perm);

    KLOG(PEXC, "PEMux[" << peid() << "] sending map(vpe=" << req.vpe_sel
        << ", virt=" << m3::fmt((void*)req.virt, "p") << ", phys=" << m3::fmt((void*)req.phys, "p")
        << ", pages=" << req.pages << ", perm=" << req.perm << ")");

    return upcall(&req, sizeof(req));
}

m3::Errors::Code PEMux::vpe_ctrl(VPE *vpe, m3::KIF::PEXUpcalls::VPEOp ctrl) {
    static const char *ctrls[] = {
        "INIT", "START", "STOP"
    };

    m3::KIF::PEXUpcalls::VPECtrl req;
    req.opcode = static_cast<xfer_t>(m3::KIF::PEXUpcalls::VPE_CTRL);
    req.vpe_sel = vpe->id();
    req.vpe_op = ctrl;
    req.eps_start = vpe->eps_start();

    KLOG(PEXC, "PEMux[" << peid() << "] sending VPECtrl(vpe="
        << req.vpe_sel << ", ctrl=" << ctrls[req.vpe_op] << ")");

    return upcall(&req, sizeof(req));
}

m3::Errors::Code PEMux::upcall(void *req, size_t size) {
    // send upcall
    event_t event = _upcqueue.send(m3::TCU::PEXUP_REP, 0, req, size, false);
    m3::ThreadManager::get().wait_for(event);

    // wait for reply
    auto reply_msg = reinterpret_cast<const m3::TCU::Message*>(m3::ThreadManager::get().get_current_msg());
    auto reply = reinterpret_cast<const m3::KIF::DefaultReply*>(reply_msg->data);
    return static_cast<m3::Errors::Code>(reply->error);
}

m3::Errors::Code PEMux::invalidate_ep(vpeid_t vpe, epid_t ep, bool force) {
    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = invalid");

    uint32_t unread_mask;
    m3::Errors::Code res = TCU::inval_ep_remote(vpe, peid(), ep, force, &unread_mask);
    if(res != m3::Errors::NONE)
        return res;

    if(unread_mask != 0) {
        m3::KIF::PEXUpcalls::RemMsgs req;
        req.vpe_sel = vpe;
        req.unread_mask = unread_mask;
        return upcall(&req, sizeof(req));
    }
    else
        return m3::Errors::NONE;
}

m3::Errors::Code PEMux::config_rcv_ep(epid_t ep, vpeid_t vpe, epid_t rpleps, RGateObject &obj) {
    assert(obj.activated());
    // it needs to be in the receive buffer space
    const goff_t addr = Platform::def_recvbuf(peid());
    const size_t size = Platform::pe(peid()).has_virtmem() ? RECVBUF_SIZE : RECVBUF_SIZE_SPM;
    // def_recvbuf() == 0 means that we do not validate it
    if(addr && (obj.addr < addr || obj.addr > addr + size || obj.addr + obj.size() > addr + size))
        return m3::Errors::INV_ARGS;
    if(obj.addr < addr + _rbufs_size)
        return m3::Errors::INV_ARGS;

    vpeid_t ep_vpe = Platform::is_shared(peid()) ? vpe : VPE::INVALID_ID;
    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = "
        "RGate[vpe=" << ep_vpe << ", addr=#" << m3::fmt(obj.addr, "x")
        << ", order=" << obj.order
        << ", msgorder=" << obj.msgorder
        << ", replyeps=" << rpleps
        << "]");

    TCU::config_remote_ep(vpe, peid(), ep, [this, &obj, rpleps, ep_vpe](m3::TCU::reg_t *ep_regs) {
        TCU::config_recv(ep_regs, ep_vpe, rbuf_base() + obj.addr,
                         obj.order, obj.msgorder, rpleps);
    });

    m3::ThreadManager::get().notify(reinterpret_cast<event_t>(&obj));
    return m3::Errors::NONE;
}

m3::Errors::Code PEMux::config_snd_ep(epid_t ep, vpeid_t vpe, SGateObject &obj) {
    assert(obj.rgate->addr != 0);
    if(obj.activated)
        return m3::Errors::EXISTS;

    vpeid_t ep_vpe = Platform::is_shared(peid()) ? vpe : VPE::INVALID_ID;
    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = "
        "Send[vpe=" << ep_vpe << ", pe=" << obj.rgate->pe
        << ", ep=" << obj.rgate->ep
        << ", label=#" << m3::fmt(obj.label, "x")
        << ", msgsize=" << obj.rgate->msgorder
        << ", crd=#" << m3::fmt(obj.credits, "x")
        << "]");

    obj.activated = true;

    TCU::config_remote_ep(vpe, peid(), ep, [&obj, ep_vpe](m3::TCU::reg_t *ep_regs) {
        TCU::config_send(ep_regs, ep_vpe, obj.label, obj.rgate->pe, obj.rgate->ep,
                         obj.rgate->msgorder, obj.credits);
    });
    return m3::Errors::NONE;
}

m3::Errors::Code PEMux::config_mem_ep(epid_t ep, vpeid_t vpe, const MGateObject &obj, goff_t off) {
    if(off >= obj.size || obj.addr + off < off)
        return m3::Errors::INV_ARGS;

    vpeid_t ep_vpe = Platform::is_shared(peid()) ? vpe : VPE::INVALID_ID;
    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = "
        "Mem [vpe=" << ep_vpe << ", pe=" << obj.pe
        << ", addr=#" << m3::fmt(obj.addr + off, "x")
        << ", size=#" << m3::fmt(obj.size - off, "x")
        << ", perms=#" << m3::fmt(obj.perms, "x")
        << "]");

    TCU::config_remote_ep(vpe, peid(), ep, [&obj, ep_vpe, off](m3::TCU::reg_t *ep_regs) {
        TCU::config_mem(ep_regs, ep_vpe, obj.pe, obj.vpe,
                        obj.addr + off, obj.size - off, obj.perms);
    });
    return m3::Errors::NONE;
}

}
