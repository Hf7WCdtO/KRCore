#[warn(non_snake_case)]
#[warn(dead_code)]
#[warn(unused_must_use)]
use KRdmaKit::rust_kernel_rdma_base;
// rust_kernel_rdma_base
use rust_kernel_rdma_base::*;
use rust_kernel_rdma_base::linux_kernel_module;
// linux_kernel_module
use linux_kernel_module::{println};
use KRdmaKit::device::{RContext};
use KRdmaKit::qp::{RC};
use KRdmaKit::ib_path_explorer::IBExplorer;
use KRdmaKit::ctrl::RCtrl;
use rust_kernel_linux_util::timer::KTimer;
use core::sync::atomic::compiler_fence;
use core::sync::atomic::Ordering;

use rust_kernel_linux_util::bindings::{bd_ssleep};

use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::string::String;
use core::pin::Pin;

use crate::{TOTAL_OP, TOTAL_PASSED_US};

static mut RCTRL_READY: bool = false;
static mut RUNNING: bool = true;

pub unsafe extern "C" fn server_thread(
    _data: *mut linux_kernel_module::c_types::c_void,
) -> linux_kernel_module::c_types::c_int {
    println!("server thread come in");

    println!("creating rctrl...");

    let mut ctrls: Vec<Pin<Box<RCtrl>>> = Vec::new();

    for i in 0..crate::THREADS {
        ctrls.push(RCtrl::create(crate::REMOTE_SERVICE_ID + i as usize,
                                 &mut crate::ALLRCONTEXTS.get_mut()[crate::CTX_IDX]).unwrap());
    }

    println!("rctrl created!");

    RCTRL_READY = true;
    RUNNING = true;
    for _i in 0..crate::RUNNING_SECS {
        // do nothing
        bd_ssleep(1);
    }
    RUNNING = false;
    bd_ssleep(crate::EXTRA_TIME);
    println!("server thread out");
    0
}

pub unsafe extern "C" fn client_thread(
    _data: *mut linux_kernel_module::c_types::c_void,
) -> linux_kernel_module::c_types::c_int {
    let thread_id = _data as u64;
    println!("client thread {} come in", thread_id);
    println!("waiting for rctrl to be ready");
    while !RCTRL_READY {
        compiler_fence(Ordering::SeqCst);
    }
    println!("after rctrl ready");
    let ctx: &mut RContext = &mut crate::ALLRCONTEXTS.get_mut()[crate::CTX_IDX];
    let sa_client = &mut crate::SA_CLIENT.get_mut();
    let gid_addr = ctx.get_gid_as_string();
    let gid_addr = String::from("fe80:0000:0000:0000:ec0d:9a03:00ca:31d8");
    let mut explorer = IBExplorer::new();

    let _ = explorer.resolve((crate::REMOTE_SERVICE_ID + thread_id as usize) as u64, ctx, &gid_addr, sa_client.get_inner_sa_client());

    // thus the path resolver could be split out of other module
    let path_res = explorer.get_path_result().unwrap();


    let mut op_count: u64 = 0;

    let timer: KTimer = KTimer::new();

    while RUNNING {
        compiler_fence(Ordering::SeqCst);
        let rc = RC::new(ctx, core::ptr::null_mut(), core::ptr::null_mut());
        let mut rc = rc.unwrap();
        let mrc = Arc::get_mut_unchecked(&mut rc);
        let res = mrc.connect(73,
                              path_res,
                              (crate::REMOTE_SERVICE_ID + thread_id as usize) as u64);
        if res.is_err() {
            println!("thread {} error in connection", thread_id);
            continue;
        }
        op_count += 1;
    }
    compiler_fence(Ordering::SeqCst);
    let passed_usec = timer.get_passed_usec() as u64;

    let lat_per_op: f64 = passed_usec as f64 / (op_count as f64);    // latency
    let thpt: f64 = op_count as f64 / (passed_usec) as f64 * (1000 * 1000) as f64;
    println!(
        "benchmark result:\n thpt:[ {:.5} ] op/s\tlat:[ {:.5} ] us",
        thpt, lat_per_op
    );
    TOTAL_OP += op_count;
    TOTAL_PASSED_US += passed_usec;
    println!("client thread out");
    0
}
