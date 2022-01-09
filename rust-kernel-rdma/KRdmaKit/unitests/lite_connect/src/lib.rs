#![no_std]
#![feature(get_mut_unchecked, allocator_api)]
#[warn(non_snake_case)]
#[warn(dead_code)]
#[warn(unused_must_use)]
extern crate alloc;

pub mod console_msgs;

use rust_kernel_rdma_base::*;
use KRdmaKit::{Profile, rust_kernel_rdma_base, to_ptr};
// rust_kernel_rdma_base
use rust_kernel_rdma_base::linux_kernel_module;
// linux_kernel_module
use linux_kernel_module::{c_types, println};
use KRdmaKit::ctrl::RCtrl;
use KRdmaKit::device::{RContext, RNIC};
use KRdmaKit::ib_path_explorer::IBExplorer;
use KRdmaKit::mem::{Memory, RMemPhy};
use KRdmaKit::qp::*;
use KRdmaKit::thread_local::ThreadLocal;
use KRdmaKit::SAClient;


use alloc::vec::Vec;

use alloc::sync::Arc;
use core::ptr::null_mut;
use lazy_static::lazy_static;
use crate::linux_kernel_module::KernelResult;

type UnsafeGlobal<T> = ThreadLocal<T>;

struct LITEConnectTestModule {}

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

/*
static mut CLIENT: ib_client = ib_client {
    name: b"raw-kernel-rdma-unit-test\0".as_ptr() as *mut c_types::c_char,
    add: Some(_add_one),
    remove: Some(_remove_one),
    get_net_dev_by_params: None,
    list: rust_kernel_rdma_base::bindings::list_head::new_as_nil(),
}; */

static mut CLIENT: Option<ib_client> = None;

unsafe fn get_global_client() -> &'static mut ib_client {
    match CLIENT {
        Some(ref mut x) => &mut *x,
        None => panic!(),
    }
}

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

    let _ = unsafe { ib_register_client(get_global_client() as *mut ib_client) };

    print_test_msgs(0, ALLNICS.len() > 0);

    // create all of the context according to NIC
    for i in 0..ALLNICS.len() {
        ALLRCONTEXTS
            .get_mut()
            .push(RContext::create(&mut ALLNICS.get_mut()[i]).unwrap());
        println!("create [{}] success", i);
    }
}

unsafe fn lite_connect() -> KernelResult<()> {
    let ctx_idx = 0;
    let ctx: &mut RContext = &mut ALLRCONTEXTS.get_mut()[ctx_idx];
    let client_rc = RC::new(ctx, null_mut(), null_mut()).unwrap();
    let server_rc = RC::new(ctx, null_mut(), null_mut()).unwrap();

    let client_qp = client_rc.get_qp();
    let server_qp = server_rc.get_qp();

    let mut profile: Profile = Profile::new();

    profile.reset_timer();

    profile.tick_record(0);
    rc_connector::bring_to_init(client_qp)?;
    profile.tick_record(1);
    rc_connector::bring_to_init(server_qp)?;
    profile.tick_record(2);
    rc_connector::bring_init_to_rtr(server_qp, client_rc.get_qp_num(), ctx.get_lid(), ctx.get_gid())?;
    profile.tick_record(3);
    rc_connector::bring_init_to_rtr(client_qp, server_rc.get_qp_num(), ctx.get_lid(), ctx.get_gid())?;
    profile.tick_record(4);
    rc_connector::bring_rtr_to_rts(client_qp)?;
    profile.tick_record(5);

    profile.increase_op(1);
    profile.report(6);
    let lkey = ctx.get_lkey();
    let mut test_mr = RMemPhy::new(1024 * 1024);

    let local_pa = test_mr.get_pa(0) as u64;
    let remote_pa = local_pa + 1024;

    let local_va = test_mr.get_ptr() as *mut u64;
    let remote_va = (local_va as u64 + 1024) as *mut u64;
    let sz = core::mem::size_of::<u64>();
    let mut op = RCOp::new(&client_rc);

    (*remote_va) = 1024;

    // send
    let _ = op.read(local_pa, lkey, sz, remote_pa, lkey);
    let result = *local_va;
    println!("res:{}", result);
    Ok(())
}

unsafe fn control_bench() -> KernelResult<()> {
    let ctx0: &mut RContext = &mut ALLRCONTEXTS.get_mut()[0];
    let ctx1: &mut RContext = &mut ALLRCONTEXTS.get_mut()[1];
    let mut profile: Profile = Profile::new();
    let run_cnt = 100;
    for _ in 0..run_cnt {
        let client_rc = RC::new(ctx0, null_mut(), null_mut()).unwrap();
        let server_rc = RC::new(ctx1, null_mut(), null_mut()).unwrap();

        let client_qp = client_rc.get_qp();
        let server_qp = server_rc.get_qp();

        profile.reset_timer();
        profile.tick_record(0);
        rc_connector::bring_to_init(client_qp)?;
        profile.tick_record(1);
        rc_connector::bring_to_init(server_qp)?;
        profile.tick_record(2);
        rc_connector::bring_init_to_rtr(server_qp, client_rc.get_qp_num(), ctx0.get_lid(), ctx0.get_gid())?;
        profile.tick_record(3);
        rc_connector::bring_init_to_rtr(client_qp, server_rc.get_qp_num(), ctx1.get_lid(), ctx1.get_gid())?;
        profile.tick_record(4);
        rc_connector::bring_rtr_to_rts(client_qp)?;
        profile.tick_record(5);
    }

    profile.increase_op(run_cnt);
    profile.report(6);
    Ok(())
}


impl linux_kernel_module::KernelModule for LITEConnectTestModule {
    fn init() -> linux_kernel_module::KernelResult<Self> {
        ctx_init();
        // unsafe { lite_connect(); }
        unsafe { control_bench(); }
        Ok(Self {})
    }
}

impl Drop for LITEConnectTestModule {
    fn drop(&mut self) {
        ALLRCONTEXTS.get_mut().clear();
        ALLNICS.get_mut().clear();
        unsafe { ib_unregister_client(get_global_client() as *mut ib_client) };
        SA_CLIENT.get_mut().reset();
    }
}

linux_kernel_module::kernel_module!(
    LITEConnectTestModule,
    author: b"anonymous",
    description: b"LITE connection test",
    license: b"GPL"
);
