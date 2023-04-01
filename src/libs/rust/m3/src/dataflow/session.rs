use base::col::ToString;
use base::errors::Error;
use base::{kif, log};

use super::app_helpers::{AppContext, CompGraph, Flags, Selector};
use crate::com::{recv_msg, RecvGate, SGateArgs, SendGate};
use crate::println;
use crate::tiles::{ActivityArgs, ChildActivity, RunningActivity, RunningProgramActivity, Tile};
use base::col::BTreeMap;
use base::col::Vec;

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
                panic!("Stopping tests here.")
            },
        }
    }};
}

const LOG_SESS: bool = true;

/// Arguments to configure context specific session parameters
pub struct SessionArgs {
    size: u64,
    perm: kif::Perm,
}

impl SessionArgs {
    pub fn new(size: u64, perm: kif::Perm) -> Self {
        Self { size, perm }
    }
}

/// Represents an application session including all it's offloaded contexts
pub struct Session {
    num_contexts: u64,
    comp_graph: CompGraph,
    ctx_map: BTreeMap<u64, (AppContext, ChildActivity)>,
    sgate_map: BTreeMap<u64, (SendGate, RecvGate)>,
    rgate_map: BTreeMap<u64, RecvGate>,
    running_activities: Vec<RunningProgramActivity>,
}

impl Session {
    pub fn new() -> Self {
        Session {
            num_contexts: 0,
            comp_graph: CompGraph::new(),
            ctx_map: BTreeMap::new(),
            sgate_map: BTreeMap::new(),
            rgate_map: BTreeMap::new(),
            running_activities: Vec::new(),
        }
    }

    /// Create a new child activity from the application context
    pub fn insert(&mut self, ctx: AppContext) -> Result<Selector, Error> {
        let tile = match Tile::get_with_props(
            ctx.get_target_tile().as_str(),
            ctx.get_target_tile_hash(),
            ctx.get_tile_sharing(),
        ) {
            Ok(t) => t,
            Err(e) => return Err(e),
        };

        let child_act = match ChildActivity::new_with(
            tile,
            ActivityArgs::new(&(self.num_contexts + 1).to_string()),
        ) {
            Ok(act) => act,
            Err(e) => return Err(e),
        };

        self.num_contexts += 1;

        self.ctx_map.insert(self.num_contexts, (ctx, child_act));

        Ok(self.num_contexts)
    }

    fn create_edge(&mut self, src_sel: Selector, dst_sel: Selector, flags: Flags) {
        if !(flags & Flags::S).is_empty() {
            if !(flags & Flags::G).is_empty() {
                self.comp_graph.create_conn(src_sel, dst_sel, Flags::G);
                self.comp_graph.create_conn(src_sel, dst_sel, Flags::S);
            }
            else {
                self.comp_graph.create_conn(src_sel, dst_sel, Flags::S);
            }
        }
        if !(flags & Flags::R).is_empty() {
            self.comp_graph.create_conn(src_sel, dst_sel, Flags::R);
        }
        if !(flags & Flags::W).is_empty() {
            self.comp_graph.create_conn(src_sel, dst_sel, Flags::W);
        }
    }

    /// Connect application context directly to this session
    pub fn connect_to(&mut self, ctx_sel: Selector, flags: Flags) {
        self.create_edge(0, ctx_sel, flags);
    }

    /// Connect two contexts together
    pub fn connect_src_to_sink(&mut self, src_sel: Selector, sink_sel: Selector, flags: Flags) {
        self.create_edge(src_sel, sink_sel, flags);
    }

    /// Connect two contexts A and B such that context A sends messages and context B receives them
    /// Connection is established via a receive gate and send gate combination
    fn create_send_recv_conn(&mut self, ctx_a: Selector, ctx_b: Selector) {
        // TODO: Better error handling
        // TODO: Handle error when application tries to create the same connection again
        let rgate = wv_assert_ok!(RecvGate::new(8, 8));
        let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate)));

        if ctx_a == 0 {
            // Some gates belong to this session and we do not need to delegate them
            let (_, act_b) = self.ctx_map.get_mut(&ctx_b).unwrap();
            wv_assert_ok!(act_b.delegate_obj(rgate.sel()));

            let mut dst = act_b.data_sink();
            dst.push(rgate.sel());

            // Save sgate to be used later
            self.sgate_map.insert(ctx_b, (sgate, rgate));
        }
        else {
            let (_, act_a) = self.ctx_map.get_mut(&ctx_a).unwrap();
            wv_assert_ok!(act_a.delegate_obj(sgate.sel()));

            let mut dst = act_a.data_sink();
            dst.push(sgate.sel());

            if ctx_b == 0 {
                self.rgate_map.insert(ctx_a, rgate);
            }
            else {
                let (_, act_b) = self.ctx_map.get_mut(&ctx_b).unwrap();
                wv_assert_ok!(act_b.delegate_obj(rgate.sel()));

                let mut dst = act_b.data_sink();
                dst.push(rgate.sel());
            }
        }
        log!(LOG_SESS, "Finished creating send_recv conn");
    }

    /// Create a reply connection between contexts A and B. This allows context A to
    /// receive a reply to a message it sent to context B
    fn create_send_get_conn(&mut self, ctx_a: Selector, ctx_b: Selector) {
        if ctx_a == 0 {
            // Since reply is paired with a send for the session we only need to create a local
            // receive gate to receive replies
            // TODO: Handle error when only a reply connection is specified
            let mut recv_gate = wv_assert_ok!(RecvGate::new(8, 8));
            recv_gate.activate();
            self.rgate_map.insert(ctx_b, recv_gate);
            // self.rgate_map.insert(ctx_b, wv_assert_ok!(RecvGate::def()));
        }
        else {
            let rgate = wv_assert_ok!(RecvGate::new(8, 8));
            let (_, act_a) = self.ctx_map.get_mut(&ctx_a).unwrap();
            wv_assert_ok!(act_a.delegate_obj(rgate.sel()));

            let mut dst = act_a.data_sink();
            dst.push(rgate.sel());
        }
    }

    /// Run the contexts in the session
    pub fn run(&mut self) -> Result<(), Error> {
        // Create connections for message and data exchange
        for (src, dst, flag) in self.comp_graph.graph.clone() {
            if flag == Flags::S {
                self.create_send_recv_conn(src, dst);
            }
            else if flag == Flags::G {
                // Idea: store recv gate only when it needs reply
                self.create_send_get_conn(src, dst);
            }
        }

        // Run activities
        for i in 1..self.num_contexts + 1 {
            let (app_ctx, child_act) = self.ctx_map.remove(&i).unwrap();

            let child_act_handle = child_act.run(app_ctx.get_app_logic()).unwrap();
            self.running_activities.push(child_act_handle);
        }

        Ok(())
    }

    /// Wait for the contexts to finish execution
    pub fn wait(&self) {
        // TODO: Error handling
        for act in &self.running_activities {
            act.wait().unwrap();
        }
    }

    /// Send message to a context
    pub fn send_to(&self, send_sel: Selector, data: &[u8]) -> Result<(), Error> {
        let sgate = &self.sgate_map.get(&send_sel).unwrap().0;

        if self.rgate_map.contains_key(&send_sel) {
            log!(LOG_SESS, "send_to: rgate_map has key");
            // let rgate = self.rgate_map.get(&send_sel).unwrap();
            // send_vmsg!(&sgate, rgate, data);
            log!(LOG_SESS, "send_to: {:?}", sgate);
            send_vmsg!(&sgate, RecvGate::def(), data);

            log!(LOG_SESS, "send_to: send_vmsg success");
        }
        else {
            send_vmsg!(&sgate, RecvGate::def(), data);
        }
        Ok(())
    }

    /// Receive message from context
    pub fn recv_from(&mut self, recv_sel: Selector) -> &[u8] {
        let rgate = self.rgate_map.get_mut(&recv_sel).unwrap();

        // let msg = wv_assert_ok!(recv_msg(&rgate));
        let msg = wv_assert_ok!(recv_msg(RecvGate::def()));

        // TODO: To reply to this message maybe store this in a reply_map

        msg.msg().as_bytes()
    }
}
