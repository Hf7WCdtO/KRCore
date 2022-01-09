#![no_std]
#![feature(get_mut_unchecked, allocator_api)]
#[warn(non_snake_case)]
#[warn(dead_code)]
extern crate alloc;

pub mod console_msgs;

use KRdmaKit::rust_kernel_rdma_base;
// rust_kernel_rdma_base
use rust_kernel_rdma_base::*;
use rust_kernel_rdma_base::linux_kernel_module;
// linux_kernel_module
use linux_kernel_module::{println, c_types};
use KRdmaKit::device::{RNIC, RContext};
use KRdmaKit::qp::{RC, RCOp, send_reg_mr};
use KRdmaKit::SAClient;
use KRdmaKit::thread_local::ThreadLocal;
use KRdmaKit::ib_path_explorer::IBExplorer;
use KRdmaKit::ctrl::RCtrl;
use KRdmaKit::mem::RMemVirt;
use KRdmaKit::Profile;

use alloc::vec::Vec;
use alloc::boxed::Box;
use core::pin::Pin;

use lazy_static::lazy_static;
use alloc::sync::Arc;

type UnsafeGlobal<T> = ThreadLocal<T>;

struct RCBreakDownTestModule {}

lazy_static! {
    static ref ALLNICS: UnsafeGlobal<Vec<RNIC>> = UnsafeGlobal::new(Vec::new());
    static ref ALLRCONTEXTS: UnsafeGlobal<Vec<RContext<'static>>> = UnsafeGlobal::new(Vec::new());
    static ref SA_CLIENT: UnsafeGlobal<SAClient> = UnsafeGlobal::new(SAClient::create());
}

unsafe extern "C" fn _add_one(dev: *mut ib_device) {
    let nic = RNIC::create(dev, 1);
    ALLNICS.get_mut().push(nic.ok().unwrap());
}

gen_add_dev_func!(_add_one, _new_add_one);

unsafe extern "C" fn _remove_one(dev: *mut ib_device, _client_data: *mut c_types::c_void) {
    println!("remove one dev {:?}", dev);
}

static mut CLIENT: Option<ib_client> = None;

unsafe fn get_global_client() -> &'static mut ib_client {
    match CLIENT {
        Some(ref mut x) => &mut *x,
        None => panic!(),
    }
}

/*
static mut CLIENT: ib_client = ib_client {
    name: b"raw-kernel-rdma-unit-test\0".as_ptr() as *mut c_types::c_char,
    add: Some(_add_one),
    remove: Some(_remove_one),
    get_net_dev_by_params: None,
    list: rust_kernel_rdma_base::bindings::list_head::new_as_nil(),
}; */

fn print_test_msgs(test_case_idx: usize, assert: bool) {
    if assert {
        println!("{:?}", crate::console_msgs::SUCC[test_case_idx]);
    } else {
        println!("{:?}", crate::console_msgs::ERR[test_case_idx]);
    }
}

fn ctx_init() {
    // register
    unsafe {
        CLIENT = Some(core::mem::MaybeUninit::zeroed().assume_init());
        get_global_client().name = b"kRdmaKit-unit-test\0".as_ptr() as *mut c_types::c_char;
        get_global_client().add = Some(_new_add_one);
        get_global_client().remove = Some(_remove_one);
        get_global_client().get_net_dev_by_params = None;
    }

    let err = unsafe { ib_register_client(get_global_client() as *mut ib_client) };
    print_test_msgs(0, err == 0);
    print_test_msgs(0, ALLNICS.len() > 0);

    // create all of the context according to NIC
    for i in 0..ALLNICS.len() {
        ALLRCONTEXTS.get_mut()
            .push(RContext::create(&mut ALLNICS.get_mut()[i]).unwrap());
        println!("create [{}] success", i);
    }
}

use rust_kernel_linux_util::timer::KTimer;
use alloc::string::String;
use core::ptr::null_mut;
use KRdmaKit::mem::{RMemPhy, Memory};

// path resolve
fn kernel_cm_breakdown() {
    let ctx: &mut RContext = &mut ALLRCONTEXTS.get_mut()[0];
    let sa_client = &mut SA_CLIENT.get_mut();
    let mut rc_profile = Profile::new();
    let mut explorer = IBExplorer::new();
    let remote_service_id = 1;
    let qd_hint = 73;

    let op_count = 1000;
    let mut outer_profile = Profile::new();

    for i in 0..op_count {
        outer_profile.reset_timer();

        let gid_addr = ctx.get_gid_as_string();
        let gid_addr = String::from("fe80:0000:0000:0000:ec0d:9a03:0078:6262");   // val13, nic_0
        let _ = explorer.resolve(remote_service_id, ctx, &gid_addr, sa_client.get_inner_sa_client());
        outer_profile.tick_record(0); // >>>>>> cm

        // get path
        // thus the path resolver could be split out of other module
        let path_res = explorer.get_path_result().unwrap();
        // >>>>>>> Start to connect <<<<<<<<<
        // create RC at one side
        let rc = RC::new(ctx,
                         null_mut(),
                         null_mut());
        outer_profile.tick_record(1); // >>>>>>>> create qp

        let mut rc = rc.unwrap();
        let mrc = unsafe { Arc::get_mut_unchecked(&mut rc) };
        // connect to the server (Ctrl)
        let _ = mrc.connect(qd_hint + i, path_res, remote_service_id as u64);
        outer_profile.tick_record(2); // >>>>>>>>>> connect


        println!("[RC report]");
        rc.profile.report(4);
    }
    println!("[Outer report]");
    outer_profile.increase_op(op_count);
    outer_profile.report(3);
}


fn test_case_req_lat() {
    let ctx: &mut RContext = &mut ALLRCONTEXTS.get_mut()[0];
    let sa_client = &mut SA_CLIENT.get_mut();
    let mut explorer = IBExplorer::new();
    let remote_service_id = 1;
    let qd_hint = 73;

    let gid_addr = ctx.get_gid_as_string();
    let _ = explorer.resolve(1, ctx, &gid_addr, sa_client.get_inner_sa_client());

    // get path
    let boxed_ctx = Box::pin(ctx);
    // thus the path resolver could be split out of other module
    let path_res = explorer.get_path_result().unwrap();
    println!("finish path get");

    let rc = RC::new(&mut ALLRCONTEXTS.get_mut()[0], core::ptr::null_mut(), core::ptr::null_mut());

    let mut rc = rc.unwrap();
    let mrc = unsafe { Arc::get_mut_unchecked(&mut rc) };
    // connect to the server (Ctrl)
    let _ = mrc.connect(qd_hint + 1, path_res, remote_service_id as u64);

    // 1. generate MR
    let mut test_mr = RMemPhy::new(1024 * 1024);
    // 2. get all of the params for later operation
    let lkey = unsafe { boxed_ctx.get_lkey() } as u32;
    let rkey = lkey as u32;
    let laddr = test_mr.get_pa(0) as u64;   // use pa for RCOp
    let raddr = test_mr.get_pa(1024) as u64;
    let sz = 8 as usize;

    // va of the mr. These addrs are used to read/write value in test function
    let va_buf = (test_mr.get_ptr() as u64 + 1024) as *mut i8;
    let target_buf = (test_mr.get_ptr()) as *mut i8;

    // 3. send request of type `READ`
    let mut op = RCOp::new(&rc);


    // >>>>>>>>>>>>>>> now start to record profile <<<<<<<<<<<<<<<<
    let mut profile = Profile::new();
    let _ = op.read(laddr, lkey, sz, raddr, rkey);

    let op_count = 2;

    for i in 0..op_count {
        let _ = op.read(laddr, lkey, sz, raddr, rkey);
    }
    profile.tick_record(0);
    profile.increase_op(op_count);
    println!("[Request report][1]");
    profile.report(1);

    profile.reset_timer();
    let op_count = 100000;

    for i in 0..op_count {
        let _ = op.read(laddr, lkey, sz, raddr, rkey);
    }
    profile.tick_record(0);
    profile.increase_op(op_count);
    println!("[Request report][All]");
    profile.report(1);
}

impl linux_kernel_module::KernelModule for RCBreakDownTestModule {
    fn init() -> linux_kernel_module::KernelResult<Self> {
        ctx_init();
        kernel_cm_breakdown();
        // test_case_req_lat();
        Ok(Self {})
    }
}

impl Drop for RCBreakDownTestModule {
    fn drop(&mut self) {
        unsafe { ib_unregister_client(get_global_client() as *mut ib_client) };
        SA_CLIENT.get_mut().reset();
    }
}


linux_kernel_module::kernel_module!(
    RCBreakDownTestModule,
    author: b"noone",
    description: b"A unit test for rc breakdown",
    license: b"GPL"
);
