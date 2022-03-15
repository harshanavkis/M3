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
  ];

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
    git
    gdb
    scons
    swig
    m4
    pkg-config
    tree
  ];
}
