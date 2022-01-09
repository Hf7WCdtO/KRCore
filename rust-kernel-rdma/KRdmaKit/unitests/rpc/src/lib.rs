#![no_std]
#![feature(get_mut_unchecked, allocator_api)]
#![feature(min_const_generics)]
#[warn(non_snake_case)]
#[warn(dead_code)]
#[warn(unused_must_use)]
extern crate alloc;

mod rpc;
pub mod console_msgs;

use rust_kernel_rdma_base::*;
use KRdmaKit::rust_kernel_rdma_base;
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
use nostd_async::{Runtime, Task};

type UnsafeGlobal<T> = ThreadLocal<T>;

struct RPCTestModule {}

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


use KRdmaKit::Profile;
use KRdmaKit::cm::{EndPoint, SidrCM};
use KRdmaKit::consts::DEFAULT_RPC_HINT;
use crate::rpc::data::{Header, IntValue, ReqType};
use crate::rpc::RPCClient;

// DCT uses an RC to register the memory
#[inline]
fn test_rpc() -> () {
    let ctx_idx = 0;
    let ctx: &mut RContext = &mut ALLRCONTEXTS.get_mut()[ctx_idx];
    let sa_client = &mut SA_CLIENT.get_mut();

    // preparation
    let mut explorer = IBExplorer::new();

    let gid_addr = ctx.get_gid_as_string();
    let _ = explorer.resolve(1, ctx, &gid_addr, sa_client.get_inner_sa_client());

    let path_res = explorer.get_path_result().unwrap();
    // param
    let remote_service_id = 50;
    let qd_hint = 73;

    let mut ctrl = RCtrl::create(remote_service_id, &mut ALLRCONTEXTS.get_mut()[0]).unwrap();
    // end of preparation
    // let cm = SidrCM::new(ctx, core::ptr::null_mut());

    let mut ud = UD::new(ctx).unwrap();
    let server_ud = UD::new(ctx).unwrap();
    ctrl.reg_ud(qd_hint, server_ud);
    println!("create UD success");

    let mut sidr_cm = SidrCM::new(ctx, core::ptr::null_mut()).unwrap();
    // sidr handshake. Only provide the `path` and the `service_id`
    let remote_info = sidr_cm.sidr_connect(
        path_res, remote_service_id as u64, qd_hint as u64);

    if remote_info.is_err() {
        return;
    }
    // ================= start to send message =====================
    let point = remote_info.unwrap();

    let runtime = Runtime::new();

    // RPC part
    let c_endpoint = EndPoint {
        qpn: ud.get_qp_num(),
        qkey: ud.get_qkey(),
        lid: ctx.get_lid(),
        gid: ctx.get_gid(),
        dct_num: ctrl.get_dc_num(),
        mr: Default::default(),
        ah: null_mut(),
    };
    let mut rpc_node_c = RPCClient::<1>::create(ctx, &ud, &c_endpoint);
    let mut rpc_node_s = RPCClient::<16>::create(ctx, ctrl.get_ud(qd_hint).unwrap(), &point);
    let mut num = 32 as u64;

    let mut server_task = Task::new(async {
        rpc_node_s.poll_all().await
    });

    let mut client_task = Task::new(async {
        for i in 0..20 {
            let mut header = Header { req_type: ReqType::Dummy.value(), rpc_id: i, end_point: Default::default() };
            let res = rpc_node_c.call::<u64, u64>(
                &point,
                &mut header,
                &mut num,
                5 * 1000).await;
        }
        0 as u64
    });

    let s_handler = server_task.spawn(&runtime);
    let c_handler = client_task.spawn(&runtime);

    // s_handler.join();
    c_handler.join();
}


impl linux_kernel_module::KernelModule for RPCTestModule {
    fn init() -> linux_kernel_module::KernelResult<Self> {
        ctx_init();
        test_rpc();
        Ok(Self {})
    }
}

impl Drop for RPCTestModule {
    fn drop(&mut self) {
        ALLRCONTEXTS.get_mut().clear();
        ALLNICS.get_mut().clear();
        unsafe { ib_unregister_client(get_global_client() as *mut ib_client) };
        SA_CLIENT.get_mut().reset();
    }
}

linux_kernel_module::kernel_module!(
    RPCTestModule,
    author: b"anonymous",
    description: b"A unit test for RPC",
    license: b"GPL"
);
