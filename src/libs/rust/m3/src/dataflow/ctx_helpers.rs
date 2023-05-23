use base::col::BTreeMap;
use base::errors::Error;
use base::mem::MsgBuf;
use base::tcu::Message;
use base::{build_vmsg, log};

use crate::com::{recv_msg, GateIStream, MemGate, RecvGate};
use crate::reply_vmsg;

use super::app_helpers::Selector;

const LOG_CTX_STATE: bool = false;

macro_rules! wv_assert_ok {
    ($res:expr) => {{
        let res = $res;
        match res {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "! {}:{}  expected Ok for {}, got {:?} FAILED",
                    file!(),
                    line!(),
                    stringify!($res),
                    e
                );
                panic!("wv_assert_ok failed")
            },
        }
    }};
}

pub struct CtxState {
    rgate_map: BTreeMap<Selector, RecvGate>,
    reply_map: BTreeMap<Selector, &'static Message>,
    mgate_map: BTreeMap<Selector, MemGate>,
}

impl CtxState {
    pub fn new() -> Self {
        Self {
            rgate_map: BTreeMap::new(),
            reply_map: BTreeMap::new(),
            mgate_map: BTreeMap::new(),
        }
    }

    pub fn recv_from(&mut self, recv_sel: Selector) -> &[u8] {
        log!(LOG_CTX_STATE, "CtxState::recv_from");
        if !self.rgate_map.contains_key(&recv_sel) {
            let mut rgate = RecvGate::new_bind(recv_sel, 8, 8);
            wv_assert_ok!(rgate.activate());
            log!(LOG_CTX_STATE, "CtxState::recv_from - rgate activated");

            self.rgate_map.insert(recv_sel, rgate);
        }
        let rgate = self.rgate_map.get_mut(&recv_sel).unwrap();

        // let msg = wv_assert_ok!(recv_msg(&rgate));
        let msg = wv_assert_ok!(rgate.receive(None));
        log!(LOG_CTX_STATE, "CtxState::recv_from - recv_msg");

        self.reply_map.insert(recv_sel, msg);

        msg.as_bytes()
    }

    pub fn reply_to(&mut self, reply_sel: Selector, data: &[u8]) -> Result<(), Error> {
        let mut reply_msg = MsgBuf::borrow_def();
        build_vmsg!(reply_msg, data);

        let rgate = self.rgate_map.get(&reply_sel).unwrap();
        rgate.reply(&reply_msg, self.reply_map.get(&reply_sel).unwrap());
        //let msg = *self.reply_map.get(&reply_sel).unwrap();
        //wv_assert_ok!(reply_vmsg!(msg, data));
        Ok(())
    }

    pub fn read_from(
        &mut self,
        read_sel: Selector,
        data: &mut [u8],
        off: u64,
    ) -> Result<(), Error> {
        if !self.mgate_map.contains_key(&read_sel) {
            let mut read_mgate = MemGate::new_bind(read_sel);
            log!(
                LOG_CTX_STATE,
                "CtxState::read_from - activating mgate: {:?}",
                read_mgate
            );
            wv_assert_ok!(read_mgate.activate());

            log!(LOG_CTX_STATE, "CtxState::read_from - mgate activated");

            self.mgate_map.insert(read_sel, read_mgate);
        }

        let mut read_mgate = self.mgate_map.get_mut(&read_sel).unwrap();
        log!(LOG_CTX_STATE, "CtxState::read_from {:?}", read_mgate);
        read_mgate.read_bytes(data.as_mut_ptr(), data.len(), off);
        Ok(())
    }
}
