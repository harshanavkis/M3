with import <nixpkgs> {};

let
  buildInputs = [
    python3
    protobuf
    zlib
    boost
    gperftools
    ncurses
    libpng
    hdf5
    mpfr
    gmp
    libmpc
    isl
    zstd
    ninja
    libxml2
  ];

  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  rustpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };

  rustBuild = (
    rustpkgs.rustChannelOf (
      let
        # Read the ./rust-toolchain (and trim whitespace) so we can extrapolate
        # the channel and date information. This makes it more convenient to
        # update the Rust toolchain used.
        rustToolchain = builtins.replaceStrings ["\n" "\r" " " "\t"] ["" "" "" ""] (
          builtins.readFile ./rust-toolchain
        );
      in
        {
          channel = lib.head (lib.splitString "-" rustToolchain);
          date = lib.concatStringsSep "-" (lib.tail (lib.splitString "-" rustToolchain));
        }
    )
  ).rust.override {
    extensions = [
      "rust-src" # required to compile the core library
      "llvm-tools-preview"
      "rust-analyzer-preview"
      "rustfmt-preview"
    ];
  };

  byaccBuild = stdenv.mkDerivation {
    pname = "byacc";
    version = "20210808";

    src = fetchurl {
      urls = [
        "ftp://ftp.invisible-island.net/byacc/byacc-20210808.tgz"
        "https://invisible-mirror.net/archives/byacc/byacc-20210808.tgz"
      ];
      sha256 = "sha256-8VhSm+nQWUJjx/Eah2FqSeoj5VrGNpElKiME+7x9OoM=";
    };

    configureFlags = [
      "--program-transform-name='s,^,b,'"
      "--enable-btyacc"
    ];

    doCheck = true;

    postInstall = ''
      ln -s $out/bin/byacc $out/bin/yacc
    '';
  };

  ld = writeShellScriptBin "ld" ''
     exec ${gcc7Stdenv.cc}/bin/ld ${lib.concatMapStringsSep " " (l: "-L${lib.getLib l}/lib -rpath ${lib.getLib l}/lib" ) buildInputs} "$@"
  '';
  args = lib.concatMapStringsSep " " (l: "-I${lib.getDev l}/include -L${lib.getLib l}/lib -Wl,-rpath,${lib.getLib l}/lib" ) buildInputs;
  cc = writeShellScriptBin "cc" ''
    exec ${gcc7Stdenv.cc}/bin/cc ${args} "$@"
  '';
  cxx = writeShellScriptBin "c++" ''
    exec ${gcc7Stdenv.cc}/bin/c++ ${args} "$@"
  '';
in gcc7Stdenv.mkDerivation rec {
  name = "env";
  EDITOR = "vim";
  M4 = "m4";
  inherit buildInputs;

  shellHook = ''
    export CC=cc CXX=c++ LD=ld
    export PATH=${ld}/bin:${cxx}/bin:${cc}/bin:$PATH
  '';

  hardeningDisable = [ "format" ];

  nativeBuildInputs = [
    python3.pkgs.pyyaml
    python3.pkgs.pandas
    git
    gdb
    scons
    swig
    m4
    pkg-config
    tree
    rustBuild
    byaccBuild
  ];
}
