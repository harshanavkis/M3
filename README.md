M³
==

This is the official repository of M³: **m**icrokernel-based syste**m** for heterogeneous **m**anycores [1, 2, 3, 4]. M³ is the operating system for a new system architecture that considers heterogeneous compute units (general-purpose cores with different instruction sets, DSPs, FPGAs, fixed-function accelerators, etc.) from the beginning instead of as an afterthought. The goal is to integrate all compute units (CUs) as *first-class citizens*, enabling 1) isolation and secure communication between all types of CUs, 2) direct interactions of all CUs to remove the conventional CPU from the critical path, 3) access to OS services such as file systems and network stacks for all CUs, and 4) context switching support on all CUs.

The system architecture is based on a hardware/operating system co-design with two key ideas:

1) introduce a new hardware component next to each CU used by the OS as the CUs' common interface and
2) let the OS kernel control applications remotely from a different CU.

The new hardware component is called trusted communication unit (TCU). Since not all CUs can be expected to offer the architectural features that are required to run an OS kernel, M³ runs the kernel on a dedicated CU and the  applications on the remaining CUs. To control an application, a kernel controls its TCU remotely, because CU-external resources (other CUs, memories, etc.) can only be accessed via the TCU.

Supported Platforms:
--------------------

Currently, M³ runs on the following platforms:

- gem5, by adding a TCU model to gem5.
- hw, a FPGA-based hardware platform.
- host, on Linux by using Linux' primitives to simulate the behavior of the TCU and the envisioned system architecture.

Getting Started:
----------------

### 1. Initial setup

If you setup the project on a new (ubuntu) machine make sure to have at least the following packages installed

    $ sudo apt update
    $ sudo apt install git build-essential scons zlib1g-dev \
        m4 libboost-all-dev libssl-dev libgmp3-dev libmpfr-dev \
        libmpc-dev libncurses5-dev texinfo ninja-build libxml2-utils

We also support building the system using a nix-shell, with dependencies loaded as defined in the default.nix file. Additionally, these packages can be loaded automagically using direnv.

Afterwards, pull in the submodules:

    $ git submodule update --init src/libs/musl src/libs/flac src/libs/leveldb

### 2. Preparations for gem5

The submodule in `platform/gem5` needs to be pulled in and built:

    $ git submodule update --init platform/gem5
    $ cd platform/gem5
    $ scons -j$(nproc) build/RISCV/gem5.opt

Note that you can specify the number of threads to use for building in the last command via, for example, `-j8`.

### 3. Preparation for the hardware platform

The current workflow assumes that the FPGA is connected to a machine `M_fpga` that is reachable via SSH from the machine `M_m3` that hosts M³. On `M_fpga`, the `platform/hw` submodule of the M³ repository needs to be available at `$HOME/tcu`:

    $ git clone https://gitlab.com/Nils-TUD/tcu.git

The bitfiles for the hardware platform can be found in `tcu/fpga_tools/bitfiles`. The bitfiles are built for the Xilinx VCU118 FPGA. Provided that Vivado Labs is installed, the following command can be used to load the latest bitfile onto the FPGA:

    $ cd tcu/fpga_tools/testcases/tc_rocket_boot && make program-fpga

Finally, before starting M³ on the FPGA, you have to tell M³ on `M_m3` how `M_fpga` can be reached:

    $ export M3_HW_SSH=ssh-alias-for-M_fpga

Note that the source of the hardware platform is [openly available](https://github.com/Barkhausen-Institut/M3-hardware) as well.

### 4. Cross compiler for gem5 and the hardware platform

For gem5 and the hardware platform, you need to build a cross compiler for the desired ISA. Note that only gem5 supports all three ISAs; the hardware platform only supports RISC-V. You can build the cross compiler as follows:

    $ cd cross
    $ ./build.sh (x86_64|arm|riscv)

The cross compiler will be installed to ``<m3-root>/build/cross-<ISA>``.

### 5. Rust

M³ is primarily written in Rust and requires some nightly features of Rust. The nightly toolchain will be installed automatically, but you need to install `rustup` manually first. Visit [rustup.rs](https://rustup.rs/) for further information.

### 6. Building

Before you build M³, you should choose your target platform, the build mode, and the ISA by exporting the corresponding environment variables. For example:

    $ export M3_BUILD=release M3_TARGET=gem5 M3_ISA=riscv LD_LIBRARY_PATH=build/cross-riscv/lib/

Now, M³ can be built by using the script `b`:

    $ ./b

### 6. Running

On all platforms, scenarios can be run by starting the desired boot script in the directory `boot`, e.g.:

    $ ./b run boot/hello.xml

Note that this command ensures that everything is up to date as well. For more information, run

    $ ./b -h

References:
-----------

**Warning:** Some papers below use the name *data transfer unit (DTU)* instead of TCU and some use the name *controller* instead of kernel.

[1] Nils Asmussen, Sebastian Haas, Carsten Weinhold, Till Miemietz, and Michael Roitzsch. **Efficient and Scalable Core Multiplexing with M³v**. In Proceedings of the Twenty-seventh International Conference on Architectural Support for Programming Languages and Operating Systems (ASPLOS'22), pages 452–466, February 2022.

[2] Nils Asmussen, Michael Roitzsch, and Hermann Härtig. **M³x: Autonomous Accelerators via Context-Enabled Fast-Path Communication**. USENIX Annual Technical Conference (ATC'19), July 2019

[3] Matthias Hille, Nils Asmussen, Pramod Bhatotia, and Hermann Härtig, **SemperOS: A Distributed Capability System**, USENIX Annual Technical Conference (ATC'19), July 2019

[4] Nils Asmussen, Marcus Völp, Benedikt Nöthen, Hermann Härtig, and Gerhard Fettweis. **M³: A Hardware/Operating-System Co-Design to Tame Heterogeneous Manycores**. In Proceedings of the Twenty-first International Conference on Architectural Support for Programming Languages and Operating Systems (ASPLOS'16), pages 189-203, April 2016.
