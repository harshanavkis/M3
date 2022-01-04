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

#pragma once

#include <base/col/SList.h>
#include <base/Env.h>

#include <m3/ObjCap.h>

#include <utility>

namespace m3 {

class EPMng;
class Gate;
class GenericFile;
class RecvGate;

/**
 * Represents a TCU endpoint that can be used for communication. This class only serves the purpose
 * to allocate a EP capability and revoke it on destruction. In the meantime, the EP capability can
 * be delegated to someone else.
 */
class EP : public SListItem, public ObjCap {
    friend class EPMng;
    friend class Gate;
    friend class GenericFile;
    friend class RecvGate;

    static EP alloc(uint replies = 0);
    static EP alloc_for(const VPE &vpe, epid_t ep = TOTAL_EPS, uint replies = 0);
    static EP bind(epid_t id) noexcept;

    explicit EP(capsel_t sel, epid_t id, uint replies, uint flags) noexcept
        : SListItem(),
          ObjCap(ObjCap::ENDPOINT, sel, flags),
          _id(id),
          _replies(replies) {
    }

public:
    explicit EP() noexcept;
    EP &operator=(EP &&ep) noexcept;
    EP(EP &&ep) noexcept
        : SListItem(std::move(ep)),
          ObjCap(std::move(ep)),
          _id(ep._id),
          _replies(ep._replies) {
    }

    /**
     * @return true if the endpoint is valid, i.e., has a selector and endpoint id
     */
    bool valid() const noexcept {
        return sel() != ObjCap::INVALID;
    }

    /**
     * @return the EP id in the TCU
     */
    epid_t id() const noexcept {
        return _id;
    }

    /**
     * @return the number of reply slots
     */
    uint replies() const noexcept {
        return _replies;
    }

    /**
     * @return if the EP is a standard EP
     */
    bool is_standard() const noexcept {
        return id() >= env()->first_std_ep && id() < env()->first_std_ep + TCU::STD_EPS_COUNT;
    }

private:
    void set_id(epid_t id) noexcept {
        _id = id;
    }

    epid_t _id;
    uint _replies;
};

}
