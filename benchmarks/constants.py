#!/usr/bin/env python3

CPU_FREQ = "2GHz"
MEM_FREQ = "2GHz"
PAR_PIPE = "1"

# Interconnect/AIU model of the evaluation platform (see platform/gem5 tcu params):
# DMA commands keep up to 16 packets in flight, in-flight packets share one AES-GCM engine,
# every encrypted packet carries 32 B of IV+MAC on the wire.
PLATFORM_ENV = {
    "M3_GEM5_INFLIGHT": "16",
    "M3_GEM5_BUFCOUNT": "16",
    "M3_GEM5_CRYPTO_ENGINES": "1",
    "M3_GEM5_CRYPTO_WIRE": "32B",
}
