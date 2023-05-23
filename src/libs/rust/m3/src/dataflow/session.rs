use base::col::ToString;
use base::errors::Error;
use base::goff;
use base::{kif, log};

use super::app_helpers::{AppContext, CompGraph, Flags, Selector};
use crate::com::{recv_msg, MemGate, RecvGate, SGateArgs, SendGate};
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

const LOG_SESS: bool = false;

/// Arguments to configure context specific session parameters
pub struct SessionArgs {
    size: u64,
}

impl SessionArgs {
    pub fn new(size: u64) -> Self {
        Self { size }
    }

    pub fn default() -> Self {
        Self { size: 0 }
    }
}

/// Represents an application session including all it's offloaded contexts
pub struct Session {
    num_contexts: u64,
    comp_graph: CompGraph,
    ctx_map: BTreeMap<u64, (AppContext, ChildActivity)>,
    sgate_map: BTreeMap<u64, (SendGate, RecvGate)>,
    rgate_map: BTreeMap<u64, RecvGate>,
    mgate_map: BTreeMap<u64, MemGate>,
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
            mgate_map: BTreeMap::new(),
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

        let mut child_act = match ChildActivity::new_with(
            tile,
            ActivityArgs::new(&(self.num_contexts + 1).to_string()),
        ) {
            Ok(act) => act,
            Err(e) => return Err(e),
        };

        child_act.add_mount("/", "/");

        self.num_contexts += 1;

        self.ctx_map.insert(self.num_contexts, (ctx, child_act));

        Ok(self.num_contexts)
    }

    fn create_edge(
        &mut self,
        src_sel: Selector,
        dst_sel: Selector,
        flags: Flags,
        sess_args: SessionArgs,
    ) {
        if !(flags & Flags::S).is_empty() {
            if !(flags & Flags::G).is_empty() {
                self.comp_graph
                    .create_conn(src_sel, dst_sel, Flags::G, sess_args.size);
                self.comp_graph
                    .create_conn(src_sel, dst_sel, Flags::S, sess_args.size);
            }
            else {
                self.comp_graph
                    .create_conn(src_sel, dst_sel, Flags::S, sess_args.size);
            }
        }
        if !(flags & Flags::R).is_empty() {
            // Assume that a read connection is always accompanied by a write connection
            self.comp_graph
                .create_conn(src_sel, dst_sel, Flags::R, sess_args.size);
        }
        //if !(flags & Flags::W).is_empty() {
        //    self.comp_graph.create_conn(src_sel, dst_sel, Flags::W);
        //}
    }

    /// Connect application context directly to this session
    pub fn connect_to(&mut self, ctx_sel: Selector, flags: Flags, sess_args: SessionArgs) {
        self.create_edge(0, ctx_sel, flags, sess_args);
    }

    /// Connect two contexts together
    pub fn connect_src_to_sink(
        &mut self,
        src_sel: Selector,
        sink_sel: Selector,
        flags: Flags,
        sess_args: SessionArgs,
    ) {
        self.create_edge(src_sel, sink_sel, flags, sess_args);
    }

    /// Connect two contexts A and B such that context A sends messages and context B receives them
    /// Connection is established via a receive gate and send gate combination
    fn create_send_recv_conn(
        &mut self,
        ctx_a: Selector,
        ctx_b: Selector,
        ctx_del_map: &mut BTreeMap<u64, Vec<Selector>>,
    ) {
        log!(LOG_SESS, "create_send_recv_conn");
        // TODO: Better error handling
        // TODO: Handle error when application tries to create the same connection again
        let rgate = wv_assert_ok!(RecvGate::new(8, 8));
        let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate)));

        if ctx_a == 0 {
            // Some gates belong to this session and we do not need to delegate them
            let (_, act_b) = self.ctx_map.get_mut(&ctx_b).unwrap();
            // wv_assert_ok!(act_b.delegate_obj(rgate.sel()));

            // let mut dst = act_b.data_sink();
            // dst.push(rgate.sel());
            if !ctx_del_map.contains_key(&ctx_b) {
                ctx_del_map.insert(ctx_b, Vec::new());
            }
            let mut ctxb_del = ctx_del_map.get_mut(&ctx_b).unwrap();
            ctxb_del.push(rgate.sel());

            // Save sgate to be used later
            self.sgate_map.insert(ctx_b, (sgate, rgate));
        }
        else {
            let (_, act_a) = self.ctx_map.get_mut(&ctx_a).unwrap();
            // wv_assert_ok!(act_a.delegate_obj(sgate.sel()));

            //let mut dst = act_a.data_sink();
            //dst.push(sgate.sel());
            if !ctx_del_map.contains_key(&ctx_a) {
                ctx_del_map.insert(ctx_a, Vec::new());
            }
            let mut ctxa_del = ctx_del_map.get_mut(&ctx_a).unwrap();
            ctxa_del.push(sgate.sel());

            if ctx_b == 0 {
                self.rgate_map.insert(ctx_a, rgate);
            }
            else {
                let (_, act_b) = self.ctx_map.get_mut(&ctx_b).unwrap();
                // wv_assert_ok!(act_b.delegate_obj(rgate.sel()));

                // let mut dst = act_b.data_sink();
                // dst.push(rgate.sel());
                if !ctx_del_map.contains_key(&ctx_b) {
                    ctx_del_map.insert(ctx_b, Vec::new());
                }
                let mut ctxb_del = ctx_del_map.get_mut(&ctx_b).unwrap();
                ctxb_del.push(rgate.sel());
            }
        }
        log!(LOG_SESS, "Finished creating send_recv conn");
    }

    /// Create a reply connection between contexts A and B. This allows context A to
    /// receive a reply to a message it sent to context B
    fn create_send_get_conn(
        &mut self,
        ctx_a: Selector,
        ctx_b: Selector,
        ctx_del_map: &mut BTreeMap<u64, Vec<Selector>>,
    ) {
        log!(LOG_SESS, "create_send_get_conn");
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
            // wv_assert_ok!(act_a.delegate_obj(rgate.sel()));

            //let mut dst = act_a.data_sink();
            //dst.push(rgate.sel());
            if !ctx_del_map.contains_key(&ctx_a) {
                ctx_del_map.insert(ctx_a, Vec::new());
            }
            let mut ctxa_del = ctx_del_map.get_mut(&ctx_a).unwrap();
            ctxa_del.push(rgate.sel());
        }
    }

    /// Create a shared memory region that is used to exchange data between the application and
    /// it's contexts
    fn create_read_write_conn(
        &mut self,
        ctx_a: Selector,
        ctx_b: Selector,
        size: u64,
        ctx_del_map: &mut BTreeMap<u64, Vec<Selector>>,
    ) {
        log!(LOG_SESS, "create_read_write_conn: {}", size);
        let mut mem_gate = wv_assert_ok!(MemGate::new(size as usize, kif::Perm::RW));
        if ctx_a == 0 {
            // Shared memory between session and a context
            let (_, act_b) = self.ctx_map.get_mut(&ctx_b).unwrap();
            // wv_assert_ok!(act_b.delegate_obj(mem_gate.sel()));

            log!(LOG_SESS, "delegated mem gate to child context");

            // let mut dst = act_b.data_sink();
            // dst.push(mem_gate.sel());
            if !ctx_del_map.contains_key(&ctx_b) {
                ctx_del_map.insert(ctx_b, Vec::new());
            }
            let mut ctxb_del = ctx_del_map.get_mut(&ctx_b).unwrap();
            ctxb_del.push(mem_gate.sel());

            self.mgate_map.insert(ctx_b, mem_gate);
        }
    }

    /// Run the contexts in the session
    pub fn run(&mut self) -> Result<(), Error> {
        let mut ctx_del_map = BTreeMap::<u64, Vec<Selector>>::new();

        // Create connections for message and data exchange
        for (src, dst, flag, size) in self.comp_graph.graph.clone() {
            if flag == Flags::S {
                self.create_send_recv_conn(src, dst, &mut ctx_del_map);
            }
            else if flag == Flags::G {
                // Idea: store recv gate only when it needs reply
                self.create_send_get_conn(src, dst, &mut ctx_del_map);
            }
            else if flag == Flags::R {
                // Create a read and write connection i.e. via shared memory
                self.create_read_write_conn(src, dst, size, &mut ctx_del_map);
            }
        }

        // Run activities
        for i in 1..self.num_contexts + 1 {
            let (app_ctx, mut child_act) = self.ctx_map.remove(&i).unwrap();

            // Add selectors to data sinks
            let del_vec = ctx_del_map.get(&i).unwrap();
            for i in del_vec {
                child_act.delegate_obj(*i);
            }

            let mut dst = child_act.data_sink();
            for i in del_vec {
                dst.push(i);
            }

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

    /// Write data to shared memory region
    pub fn write_to(&mut self, write_sel: Selector, data: &[u8], off: goff) -> Result<(), Error> {
        let mgate = self.mgate_map.get_mut(&write_sel).unwrap();
        log!(LOG_SESS, "write_to: writing - {:?}", data);
        mgate.write_bytes(data.as_ptr(), data.len(), off).unwrap();

        Ok(())
    }
}
