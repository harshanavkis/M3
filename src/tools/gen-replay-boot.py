#!/usr/bin/env python3
"""gen-replay-boot.py <trace-name> <ranks> <native|ironbus|host> [out.xml] [instances] [groups]

Writes a boot script that runs the trace replay for <trace-name> on <ranks> scratchpad tiles
(plus one for the relay in host mode); with <instances> = k, k independent instances run at the
same time, each with its own coordinator (R1.1 tenants). Run with
M3_CORES=<k*members+k+6> M3_GEM5_SPM=<k*members+1> so that enough SPM tiles exist (the
coordinators run on cached tiles)."""
import sys

name, ranks, mode = sys.argv[1], int(sys.argv[2]), sys.argv[3]
members = ranks + (1 if mode == "host" else 0)
out = sys.argv[4] if len(sys.argv) > 4 else "boot/replay-%s-%d-%s.xml" % (name, ranks, mode)
instances = int(sys.argv[5]) if len(sys.argv) > 5 else 1
# groups > 1: each coordinator runs that many independent groups of <ranks> ranks; in host mode
# they share one relay (a single host for all tenants)
groups = int(sys.argv[6]) if len(sys.argv) > 6 else 1
members = ranks * groups + (1 if mode == "host" else 0)
coords = "".join("""                    <dom>
                        <app args="/bin/tracereplay coord %s %d %s %d %d">
                            <mount fs="m3fs" path="/" />
                            <tiles type="imem+riscv" count="%d" />
                        </app>
                    </dom>
""" % (name, ranks, mode, i, groups, members) for i in range(instances))
xml = """<config>
    <kernel args="kernel -f $fs.path" />
    <dom>
        <app args="root">
            <dom>
                <app args="m3fs mem $fs.size" daemon="1">
                    <serv name="m3fs" />
                    <physmem addr="0" size="$fs.size" />
                </app>
            </dom>
            <dom>
                <app args="pager $fs.size">
                    <sess name="m3fs" />
                    <physmem addr="0" size="$fs.size" perm="r" />
                    <tiles type="core" count="%(instances)d" />
                    <tiles type="imem+riscv" count="%(spm)d" />
%(coords)s                </app>
            </dom>
        </app>
    </dom>
</config>
""" % {"instances": instances, "spm": members * instances, "coords": coords}
open(out, "w").write(xml)
print(out)
