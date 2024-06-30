{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/refs/tags/23.05.tar.gz") {} }:

let
  buildInputs = with pkgs; [
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
          channel = pkgs.lib.head (pkgs.lib.splitString "-" rustToolchain);
          date = pkgs.lib.concatStringsSep "-" (pkgs.lib.tail (pkgs.lib.splitString "-" rustToolchain));
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

  byaccBuild = pkgs.stdenv.mkDerivation {
    pname = "byacc";
    version = "20210808";

    src = pkgs.fetchurl {
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

  ld = pkgs.writeShellScriptBin "ld" ''
     exec ${pkgs.gcc7Stdenv.cc}/bin/ld ${pkgs.lib.concatMapStringsSep " " (l: "-L${pkgs.lib.getLib l}/lib -rpath ${pkgs.lib.getLib l}/lib" ) buildInputs} "$@"
  '';
  args = pkgs.lib.concatMapStringsSep " " (l: "-I${pkgs.lib.getDev l}/include -L${pkgs.lib.getLib l}/lib -Wl,-rpath,${pkgs.lib.getLib l}/lib" ) buildInputs;
  cc = pkgs.writeShellScriptBin "cc" ''
    exec ${pkgs.gcc7Stdenv.cc}/bin/cc ${args} "$@"
  '';
  cxx = pkgs.writeShellScriptBin "c++" ''
    exec ${pkgs.gcc7Stdenv.cc}/bin/c++ ${args} "$@"
  '';
in pkgs.gcc7Stdenv.mkDerivation rec {
  name = "env";
  EDITOR = "vim";
  M4 = "m4";
  inherit buildInputs;

  shellHook = ''
    export CC=cc CXX=c++ LD=ld
    export PATH=${ld}/bin:${cxx}/bin:${cc}/bin:$PATH
  '';

  hardeningDisable = [ "format" ];

  nativeBuildInputs = with pkgs; [
    python3.pkgs.pyyaml
    python3.pkgs.pandas
    python3.pkgs.seaborn
    python3.pkgs.matplotlib
    python3.pkgs.scipy
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
