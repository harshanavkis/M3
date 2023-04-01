use base::col::BTreeMap;
use base::errors::Error;
use base::mem::MsgBuf;
use base::tcu::Message;
use base::{build_vmsg, log};

use crate::com::{recv_msg, GateIStream, RecvGate};
use crate::reply_vmsg;

use super::app_helpers::Selector;

const LOG_CTX_STATE: bool = true;

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
}

impl CtxState {
    pub fn new() -> Self {
        Self {
            rgate_map: BTreeMap::new(),
            reply_map: BTreeMap::new(),
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
}
