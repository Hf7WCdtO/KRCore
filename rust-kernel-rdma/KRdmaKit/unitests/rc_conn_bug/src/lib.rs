#![no_std]
#![feature(get_mut_unchecked)]
#[warn(non_snake_case)]
#[warn(dead_code)]
extern crate alloc;

pub mod console_msgs;
mod rc_read;


use KRdmaKit::rust_kernel_rdma_base;
// rust_kernel_rdma_base
use rust_kernel_rdma_base::*;
use rust_kernel_rdma_base::linux_kernel_module;
// linux_kernel_module
use linux_kernel_module::{println, c_types};
use KRdmaKit::device::{RNIC, RContext};
use KRdmaKit::SAClient;
use KRdmaKit::thread_local::ThreadLocal;

use alloc::vec::Vec;

use lazy_static::lazy_static;
use rust_kernel_linux_util::bindings::{bd_ssleep, bd_kthread_run};

// config params

// service id
static REMOTE_SERVICE_ID: usize = 1;
// qd hint
static QD_HINT: u64 = 73;
// running seconds
static RUNNING_SECS: u32 = 10;
// client thread number
static THREADS: u32 = 2;
// use nic idx
static CTX_IDX: usize = 0;
// rctrl exit extra time
static EXTRA_TIME: u32 = 1;

// statistics
static mut TOTAL_OP: u64 = 0;
static mut TOTAL_PASSED_US: u64 = 0;

type UnsafeGlobal<T> = ThreadLocal<T>;

use rc_read::client_thread;
use rc_read::server_thread;

static mut CLIENT: Option<ib_client> = None;

unsafe fn get_global_client() -> &'static mut ib_client {
    match CLIENT {
        Some(ref mut x) => &mut *x,
        None => panic!(),
    }
}

struct RCConnTestModule {}

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

impl linux_kernel_module::KernelModule for RCConnTestModule {
    fn init() -> linux_kernel_module::KernelResult<Self> {
        ctx_init();
        let t_server = unsafe {
            bd_kthread_run(
                Some(server_thread),
                core::ptr::null_mut(),
                b"RLib CM\0".as_ptr() as *const i8,
            )
        };
        assert!(!t_server.is_null());

        for i in 0..THREADS {
            let t_client = unsafe {
                bd_kthread_run(
                    Some(client_thread),
                    i as *mut core::ffi::c_void,
                    b"RLib CM\0".as_ptr() as *const i8,
                )
            };
            assert!(!t_client.is_null());
        }

        for i in 0..RUNNING_SECS + EXTRA_TIME {
            unsafe {
                bd_ssleep(1);
            }
            println!("sleep for {} sec", i+1);
        }
        // total
        unsafe {
            let lat_per_op: f64 = TOTAL_PASSED_US as f64 / (TOTAL_OP as f64);    // latency
            let thpt: f64 = THREADS as f64 *
                TOTAL_OP as f64 / (TOTAL_PASSED_US) as f64 * (1000 * 1000) as f64;
            println!(
                "[total] benchmark result:\n thpt:[ {:.5} ] op/s\tlat:[ {:.5} ] us",
                thpt, lat_per_op
            );
        }

        Ok(Self {})
    }
}

impl Drop for RCConnTestModule {
    fn drop(&mut self) {
        unsafe { ib_unregister_client(get_global_client() as *mut ib_client) };
        SA_CLIENT.get_mut().reset();
        ALLRCONTEXTS.get_mut().clear();
    }
}


linux_kernel_module::kernel_module!(
    RCConnTestModule,
    author: b"anonymous",
    description: b"A unit test for qp basic creation",
    license: b"GPL"
);
