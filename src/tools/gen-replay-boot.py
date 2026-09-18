#!/usr/bin/env python3
"""gen-replay-boot.py <trace-name> <ranks> <native|ironbus|host> [out.xml]

Writes a boot script that runs the trace replay for <trace-name> on <ranks> scratchpad tiles
(plus one for the relay in host mode). Run with M3_CORES=<ranks+members+6> M3_GEM5_SPM=<members+1>
so that enough SPM tiles exist (the coordinator itself runs on a cached tile)."""
import sys

name, ranks, mode = sys.argv[1], int(sys.argv[2]), sys.argv[3]
members = ranks + (1 if mode == "host" else 0)
out = sys.argv[4] if len(sys.argv) > 4 else "boot/replay-%s-%d-%s.xml" % (name, ranks, mode)
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
                    <tiles type="core" count="1" />
                    <tiles type="imem+riscv" count="%(members)d" />
                    <dom>
                        <app args="/bin/tracereplay coord %(name)s %(ranks)d %(mode)s">
                            <mount fs="m3fs" path="/" />
                            <tiles type="imem+riscv" count="%(members)d" />
                        </app>
                    </dom>
                </app>
            </dom>
        </app>
    </dom>
</config>
""" % {"members": members, "name": name, "ranks": ranks, "mode": mode}
open(out, "w").write(xml)
print(out)
