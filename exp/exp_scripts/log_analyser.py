import os
import re

import numpy as np


def parse_line(line):
    machine_pattern = re.compile(r"^@(.*?)\s.*?$")

    thpt_pattern = re.compile(r"^.*?thpt:\s(.*?)\sreqs/sec.*?$")

    lat_pattern = re.compile(r"^.*?epoch\.\s(.*?)\sus")
    if thpt_pattern.match(line) is None:
        return None, None, None
    machine = machine_pattern.findall(line)[-1]
    thpt = float(thpt_pattern.findall(line)[-1])
    lat = float(lat_pattern.findall(line)[-1])
    return machine, thpt, lat


def analyse(file_path):
    exit_pattern = re.compile(r"^.*?exit.*?$")
    with open(file_path) as f:
        thpt_hash = {}
        lat_hash = {}
        for line in f:
            if exit_pattern.match(line) is not None: break
            machine, thpt, lat = parse_line(line)
            if machine is None: continue
            if machine not in thpt_hash.keys():
                thpt_hash[machine] = []
            if machine not in lat_hash.keys():
                lat_hash[machine] = []
            thpt_hash[machine].append(thpt)
            lat_hash[machine].append(lat)

        thpt_avg = {}
        lat_avg = {}
        for machine, thpts in thpt_hash.items():
            # thpt_avg[machine] = np.mean(np.sort(thpts))
            thpt_avg[machine] = np.mean(np.sort(thpts)[1:5])
        for machine, lats in lat_hash.items():
            # lat_avg[machine] = np.mean(np.sort(lats))
            lat_avg[machine] = np.mean(np.sort(lats)[-5:-1])

    sum_up_thpt = sum(list(thpt_avg.values()))
    avg_lat = np.mean(list(lat_avg.values()))
    return sum_up_thpt, avg_lat


def trigger_bootstrap(boost_path, dictorys):
    from os.path import join, getsize

    for dictory in dictorys:
        again = True  # Do exp again
        # again = False       # Just get result
        for root, dirs, files in os.walk(dictory):
            for f in files:
                pattern = re.compile(r"^run[1-9]*[0-9]+.*?toml$")
                # pattern = re.compile(r"^run.*?.toml$")
                if pattern.match(f) is not None:
                    log_path = "{}/{}.txt".format(dictory, f)
                    if again:
                        os.system("python {} -f {} > {}".format(
                            boost_path, os.path.join(root, f), log_path))
                    thpt, lat = analyse(log_path)
                    print(log_path)
                    print("{} {}".format(thpt, lat))
        print("=======================================================================================")


if __name__ == '__main__':
    trigger_bootstrap('bootstrap.py',
                      [

                          # inbound write
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/write/run-user-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/write/run-user-sync-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/write/run-krc-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/write/run-krc-sync-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/write/run-kdc-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/write/run-kdc-sync-inbound',

                          # inbound read
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-user-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-user-sync-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-krc-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-krc-sync-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-kdc-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-kdc-sync-inbound',


                          # payload - latency
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-payload-latency-user',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-one-sided/read/run-payload-latency-krc',

                          # motiv-thpt-latency
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-motiv-thpt-latency/hybrid-connect'

                          # two sided
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-urc-sync-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-urc-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-krc-sync-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-krc-async-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-kdc-sync-inbound',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-kdc-async-inbound',

                          # payload latency
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-user-payload-latency',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-krc-payload-latency',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-two-sided/run-krc-zero-copy-payload-latency',
                          # test
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/',

                          # control path
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-control-path/krcore',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-control-path/user',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-control-path/kernel',

                          # DC meta cache
                          '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-dc-meta-server/meta-server',
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-dc-meta-server/handshake',

                          # factor analysis
                          # '/Users/lufangming/Documents/repos/rust-kernel-rdma/scripts/run-factor-analysis',

                      ]
                      )

    # print(analyse('run/run-peak.toml.txt'))
