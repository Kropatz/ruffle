{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell rec {
  buildInputs = with pkgs; [
    cargo
    rustc
    rustfmt
    jdk21
    libgcc
    cmake
    gtk3
    pkg-config
    alsa-lib
    udev
    wayland
    libxkbcommon
    libGL
  ];

  shellHook = ''
    export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath buildInputs}"
  '';
}
