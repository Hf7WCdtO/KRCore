#include <gflags/gflags.h>

#include <assert.h>
#include <vector>

#include "rlib/core/lib.hh"
#include "rlib/core/qps/mod.hh"

using namespace rdmaio;
using namespace rdmaio::qp;
using namespace rdmaio::rmem;

DEFINE_int64(target_num, 1, "alloc number");
DEFINE_int64(sq_depth, 128, "send sz");
DEFINE_int64(rq_depth, 2048, "rq sz");
DEFINE_int64(cq_depth, 128, "cq sz");

int
main(int argc, char** argv)
{
  gflags::ParseCommandLineFlags(&argc, &argv, true);

  auto config = QPConfig()
                  .set_max_send(FLAGS_sq_depth)
                  .set_max_recv(FLAGS_rq_depth);
  auto nic = RNic::create(RNicInfo::query_dev_names().at(0)).value();
  auto ud = UD::create(nic, QPConfig()).value();

  auto ud1 = UD::create(nic, config).value();
  // auto qp = RC::create(nic, config).value();
#if 0
  {
    // create qp, cq, recv_cq
    auto res = Impl::create_cq(nic, config.max_send_sz());
    ibv_cq* cq = std::get<0>(res.desc);

    auto res_recv = Impl::create_cq(nic, config.max_recv_sz());
    ibv_cq* recv_cq = std::get<0>(res_recv.desc);

    auto res_qp =
      Impl::create_qp(nic, IBV_QPT_UD, config, cq, recv_cq);
    ibv_destroy_qp(std::get<0>(res_qp.desc));
    ibv_destroy_cq(cq);
    ibv_destroy_cq(recv_cq);
  }
  // auto mem = Arc<RMem>(new RMem(1024 * 1024 * 4));
  // auto mr = RegHandler::create(mem, nic).value();
#endif
}
