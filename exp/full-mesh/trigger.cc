#include <gflags/gflags.h>

#include <assert.h>
#include <vector>

#include "../src/random.hh"
#include "../src/rdma/sop.hh"
#include "rlib/benchs/reporter.hh"
#include "rlib/benchs/thread.hh"
#include "rlib/core/lib.hh"
#include <random>

#define VAL

#ifdef VAL // the name of our cluster is VAL

#include "../huge_region.hh"
#include "../val/cpu.hh"
using namespace nvm;

using namespace xstore::platforms;
#endif

using namespace rdmaio; // warning: should not use it in a global space often
using namespace rdmaio::rmem;
using namespace rdmaio::qp;
using namespace r2;
using namespace r2::rdma;
using Thread_t = bench::Thread<usize>;

DEFINE_int64(threads, 12, "#Threads used.");
DEFINE_int64(payload_sz, 8, "Payload size (bytes)");
DEFINE_int64(write_imm, 0, "write imm");
// For RDMA
DEFINE_string(addr, "localhost", "Server address to connect to.");
DEFINE_int64(port, 8888, "Server address to connect to.");
DEFINE_int64(host, 0, "Server address to connect to.");
DEFINE_int64(use_nic_idx, 0, "Which NIC to create QP");
DEFINE_int64(reg_nic_name, 73, "The name to register an opened NIC at rctrl.");
DEFINE_int64(reg_mem_name, 73, "The name to register an MR at rctrl.");
DEFINE_int64(or_sz, 12, "We use batch size to annote the step");
DEFINE_int64(mem_sz, 1024, "Mr size");
DEFINE_uint32(nic_mod, 2, "NIC use mod");
DEFINE_int64(worker_num, 2, "Worker_num");

DEFINE_string(name_recv, "server-qp", "Server recv qp name");
DEFINE_string(name_client, "client-qp", "client qp name");
DEFINE_int64(seq_limit, 4096, "seq size for overflow test");
DEFINE_int64(id, 0, "node id");
DEFINE_int64(run_sec, 10, "running seconds");
DEFINE_string(server_gid,
"fe80:0000:0000:0000:248a:0703:009c:7c94",
"server gid to connect and send");
DEFINE_string(worker_addrs, "localhost",
"addr list"); // split by one blank
#define MR_SIZE 1024 * 1024 * 16

inline std::vector<std::string>
split(const std::string& str, const std::string& delim)
{
    std::vector<std::string> res;
    if ("" == str)
        return res;
    char* strs = new char[str.length() + 1];
    strcpy(strs, str.c_str());

    char* d = new char[delim.length() + 1];
    strcpy(d, delim.c_str());

    char* p = strtok(strs, d);
    while (p) {
        std::string s = p;
        res.push_back(s);
        p = strtok(NULL, d);
    }

    return res;
}

const u64 kMsgArea = 1024 * 8;

class SimpleAllocator : public AbsRecvAllocator
{
    RMem::raw_ptr_t buf = nullptr;
    usize total_mem = 0;
    mr_key_t key;

public:
    SimpleAllocator(Arc<RMem> mem, mr_key_t key)
            : buf(mem->raw_ptr)
            , total_mem(mem->sz)
            , key(key)
    {
        // RDMA_LOG(4) << "simple allocator use key: " << key;
    }

    r2::Option<std::pair<rmem::RMem::raw_ptr_t, rmem::mr_key_t>> alloc_one(
            const usize& sz) override
    {
        if (total_mem < sz)
            return {};
        auto ret = buf;
        buf = static_cast<char*>(buf) + sz;
        total_mem -= sz;
        return std::make_pair(ret, key);
    }

    r2::Option<std::pair<rmem::RMem::raw_ptr_t, rmem::RegAttr>>
    alloc_one_for_remote(const usize& sz) override
    {
        return {};
    }
};

usize
worker_fn(const usize& worker_id, Statics* s);



static inline unsigned long
get_random()
{
    static std::random_device rd;
    static std::mt19937_64 e(rd());
    return e();
}

bool volatile running = true;
static double passed_us = 0;
static unsigned int op_cnt = 0;
int
main(int argc, char** argv)
{
    gflags::ParseCommandLineFlags(&argc, &argv, true);

    std::vector<Thread_t*> workers;
    std::vector<Statics> worker_statics(FLAGS_threads); // setup workers

    for (uint i = 0; i < FLAGS_threads; ++i) {
        workers.push_back(
                new Thread_t(std::bind(worker_fn, i, &(worker_statics[i]))));
    }

    // start the workers
    for (auto w : workers) {
        w->start();
    }

//    Reporter::report_thpt(worker_statics, FLAGS_run_sec); // report for 10 seconds
    for(int i = 0 ;i < FLAGS_run_sec; ++i) {
        sleep(1);
        RDMA_LOG(2) << "latency: " << passed_us / op_cnt << " us/op";
        passed_us = op_cnt = 0;
    }
    running = false;                                      // stop workers

    // wait for workers to join
    for (auto w : workers) {
        w->join();
    }

}

// main func for every single worker
usize
worker_fn(const usize& worker_id, Statics* s)
{
    int nic_idx = FLAGS_use_nic_idx;
#ifdef VAL
    nic_idx = (worker_id) % 2;
    VALBinder::bind(nic_idx, worker_id % 24);
#endif
    // 1. create a local QP to use
    r2::util::FastRandom r(0xdeafbeaf + worker_id + 73 * FLAGS_id);

    std::vector<std::string> gid_workers = split(FLAGS_worker_addrs, " ");
    const int worker_num = gid_workers.size();
    std::string name = std::to_string(FLAGS_host);
    auto nic = RNic::create(RNicInfo::query_dev_names().at(nic_idx)).value();

    auto recv_cq_res =
            ::rdmaio::qp::Impl::create_cq(nic, 16); // buffer with [entry_cnt]
    RDMA_ASSERT(recv_cq_res == IOCode::Ok);
    auto recv_cq = std::get<0>(recv_cq_res.desc);

    auto region = HugeRegion::create(MR_SIZE).value();
    auto local_mem = region->convert_to_rmem().value();
    auto local_mr = RegHandler::create(local_mem, nic).value();
    char* buf = (char*)(local_mr->get_reg_attr().value().buf);

    auto recv_mem = Arc<RMem>(new RMem(MR_SIZE));
    auto handler = RegHandler::create(recv_mem, nic).value();

    SimpleAllocator alloc(recv_mem, handler->get_reg_attr().value().key);

    auto recv_rs = RecvEntriesFactory<SimpleAllocator, 512, 64>::create(alloc);

    std::vector<ConnectManager> cm_list;
    std::vector<Arc<RC>> rc_list;
    std::vector<int> keys;
    std::vector<std::string> names;

    // for each worker address
    for (auto addr : gid_workers) {

        auto qp = RC::create(nic, QPConfig(), recv_cq).value();

        std::string addr_port =
                addr + ":" + std::to_string((worker_id + FLAGS_port));
        name = addr_port + "@" + std::to_string(FLAGS_host);;
        names.push_back(name);
        // 2. create the pair QP at server using CM
        ConnectManager cm(addr_port);
        cm_list.push_back(cm);
        rc_list.push_back(qp);
        if (cm.wait_ready(1000000, 50) ==
            IOCode::Timeout) // wait 1 second for server to ready, retry 2 times
            RDMA_ASSERT(false) << "cm connect to server timeout";

        int key = 0;
        while(running) {
            auto qp_res = cm.cc_rc_msg(
                    name, FLAGS_name_recv, 1024, qp, FLAGS_reg_nic_name, QPConfig());
            if (qp_res == IOCode::Ok) {
                key = std::get<1>(qp_res.desc);
                break;
            }
        }

        auto fetch_res =
                cm.fetch_remote_mr(FLAGS_reg_mem_name); // rpc call to get remote mr
        RDMA_ASSERT(fetch_res == IOCode::Ok) << std::get<0>(fetch_res.desc);
        rmem::RegAttr remote_attr = std::get<1>(fetch_res.desc);

        qp->bind_remote_mr(remote_attr);
        qp->bind_local_mr(local_mr->get_reg_attr().value());

        keys.push_back(key);
        sleep(0.1);
    }
    SROp op;
    int mod = MR_SIZE / FLAGS_payload_sz;

    auto total_buf = FLAGS_threads * kMsgArea;
    auto buf_start = total_buf * FLAGS_id + worker_id * kMsgArea;
    auto offset = 0;

    // setup operation
    op.set_imm(FLAGS_host)
            .set_payload(buf, FLAGS_payload_sz)
            .set_remote_addr(buf_start + offset)
            .set_op(IBV_WR_SEND_WITH_IMM);

    int post_cnt = FLAGS_or_sz;
    bool bootstrap = false;

    // r2::util::FastRandom r(0xdeafbeaf + worker_id + FLAGS_id * 73);
    Timer timer;
    while (running) {
        for(int i = 0 ;i < 20; ++i) {
            r2::compile_fence();
            timer.reset();
            for (int i = 0; i < worker_num; ++i) {
                auto qp = rc_list[i];
                // post recv at first
                qp->post_recvs(*recv_rs, FLAGS_or_sz);

                RDMA_ASSERT(op.execute_sync(qp, IBV_SEND_SIGNALED) == IOCode::Ok);
            }

            int polled_cnt = 0;
            while (polled_cnt < worker_num && running) {
                polled_cnt += ibv_poll_cq(recv_cq, worker_num, recv_rs->wcs);
            }
            s->increment(1); // finish one batch
            ++op_cnt;
            passed_us += timer.passed_msec();

        }
        sleep(1);
    }

    for (int i = 0; i < worker_num; ++i) {
        auto cm = cm_list[i];
        auto del_res = cm.delete_remote_rc(names[i], keys[i]);
    }

    return 0;
}
