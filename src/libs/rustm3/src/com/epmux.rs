use cap::{CapFlags, Selector};
use cell::StaticCell;
use com::gate::Gate;
use dtu::{self, EpId};
use errors::{Code, Error};
use kif::INVALID_SEL;
use syscalls;
use vpe;

pub struct EpMux {
    gates: [Option<*const Gate>; dtu::EP_COUNT],
    next_victim: usize,
}

static EP_MUX: StaticCell<EpMux> = StaticCell::new(EpMux::new());

impl EpMux {
    const fn new() -> Self {
        EpMux {
            gates: [None; dtu::EP_COUNT],
            next_victim: 1,
        }
    }

    pub fn get() -> &'static mut EpMux {
        EP_MUX.get_mut()
    }

    pub fn reserve(&mut self, ep: EpId) {
        // take care that some non-fixed gate could already use that endpoint
        if let Some(g) = self.gate_at_ep(ep) {
            syscalls::activate(0, INVALID_SEL, g.ep().unwrap(), 0).ok();
            g.unset_ep();
        }
        self.gates[ep] = None;
    }

    pub fn switch_to(&mut self, g: &Gate) -> Result<EpId, Error> {
        match g.ep() {
            Some(idx) => Ok(idx),
            None      => {
                let idx = self.select_victim()?;
                syscalls::activate(0, g.sel(), idx, 0)?;
                self.gates[idx] = Some(g);
                g.set_ep(idx);
                Ok(idx)
            }
        }
    }

    pub fn switch_cap(&mut self, g: &Gate, sel: Selector) -> Result<(), Error> {
        if let Some(ep) = g.ep() {
            syscalls::activate(0, sel, ep, 0)?;
            if sel == INVALID_SEL {
                self.gates[ep] = None;
                g.unset_ep();
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, g: &Gate) {
        if let Some(ep) = g.ep() {
            self.gates[ep] = None;
            // only necessary if we won't revoke the gate anyway
            if !(g.flags() & CapFlags::KEEP_CAP).is_empty() {
                syscalls::activate(0, INVALID_SEL, ep, 0).ok();
            }
            g.unset_ep();
        }
    }

    pub fn reset(&mut self) {
        for ep in 0..dtu::EP_COUNT {
            if let Some(g) = self.gate_at_ep(ep) {
                g.unset_ep();
            }
            self.gates[ep] = None;
        }
    }

    fn select_victim(&mut self) -> Result<EpId, Error> {
        let mut victim = self.next_victim;
        for _ in 0..dtu::EP_COUNT {
            if vpe::VPE::cur().is_ep_free(victim) {
                break;
            }

            victim = (victim + 1) % dtu::EP_COUNT;
        }

        if !vpe::VPE::cur().is_ep_free(victim) {
            Err(Error::new(Code::NoSpace))
        }
        else {
            if let Some(g) = self.gate_at_ep(victim) {
                g.unset_ep();
            }
            self.next_victim = (victim + 1) % dtu::EP_COUNT;
            Ok(victim)
        }
    }

    fn gate_at_ep(&self, ep: EpId) -> Option<&Gate> {
        if let Some(g) = self.gates[ep] {
            Some(unsafe { &*g })
        }
        else {
            None
        }
    }
}
