Supported Platforms:
--------------------

- gem5

Getting Started:
----------------

### 1. Initial setup

If you setup the project on a new (ubuntu) machine make sure to have at least the following packages installed

    $ sudo apt update
    $ sudo apt install git build-essential scons zlib1g-dev \
        m4 libboost-all-dev libssl-dev libgmp3-dev libmpfr-dev \
        libmpc-dev libncurses5-dev texinfo ninja-build libxml2-utils

We also support building the system using a nix-shell, with dependencies loaded as defined in the default.nix file. Additionally, these packages can be loaded automagically using direnv.


### 2. Preparations for gem5

The submodule in `platform/gem5` needs to be pulled in and built:

    $ git submodule update --init platform/gem5
    $ cd platform/gem5
    $ scons -j$(nproc) build/RISCV/gem5.opt

Note that you can specify the number of threads to use for building in the last command via, for example, `-j8`.

### 3. Cross compiler for gem5 and the hardware platform

For gem5 and the hardware platform, you need to build a cross-compiler for the desired ISA. Note that only gem5 supports all three ISAs; the hardware platform only supports RISC-V. You can build the cross compiler as follows:

    $ cd cross
    $ ./build.sh (x86_64|arm|riscv)

The cross compiler will be installed to ``<m3-root>/build/cross-<ISA>``.

### 4. Rust

M³ is primarily written in Rust and requires some nightly features of Rust. The nightly toolchain will be installed automatically, but you need to install `rustup` manually first. Visit [rustup.rs](https://rustup.rs/) for further information.

### 6. Building

Before you build M³, you should choose your target platform, the build mode, and the ISA by exporting the corresponding environment variables. For example:

    $ export M3_BUILD=release M3_TARGET=gem5 M3_ISA=riscv LD_LIBRARY_PATH=build/cross-riscv/lib/ M3_FS=bench.img

Now, M³ can be built by using the script `b`:

    $ ./b

### 5. Running

On all platforms, scenarios can be run by starting the desired boot script in the directory `boot`, e.g.:

    $ ./b run boot/hello.xml

Note that this command ensures that everything is up to date as well. For more information, run

    $ ./b -h
