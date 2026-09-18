#!/usr/bin/env python3

CPU_FREQ = "2GHz"
MEM_FREQ = "2GHz"
PAR_PIPE = "1"

# Interconnect/AIU model of the evaluation platform (see platform/gem5 tcu params):
# DMA commands keep up to 16 packets in flight, the AIU has one AES-GCM engine per link
# direction (M3_GEM5_CRYPTO_ENGINES counts engines per direction, as in PCIe IDE
# implementations; M3_GEM5_CRYPTO_SHARED=1 models a single pool for both directions instead,
# which halves the rate of a tile that sends and receives at the same time), every encrypted
# packet carries 32 B of IV+MAC on the wire.
PLATFORM_ENV = {
    "M3_GEM5_INFLIGHT": "16",
    "M3_GEM5_BUFCOUNT": "16",
    "M3_GEM5_CRYPTO_ENGINES": "1",
    "M3_GEM5_CRYPTO_WIRE": "32B",
}
