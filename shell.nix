{ pkgs ? ( import <nixpkgs> {}), ... }:

pkgs.mkShell {

  name = "octa-force";
  RUSTC_VERSION = "stable";
  shellHook = ''
    export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
    export PATH=$PATH:''${RUSTUP_HOME:-~/.rustup}/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
    export RUSTUP_TOOLCHAIN=$RUSTC_VERSION-x86_64-unknown-linux-gnu
  '';

  packages = with pkgs; [
    rustup
    clang
    pkg-config
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    vulkan-tools
  ];

  LD_LIBRARY_PATH =
    with pkgs;
    lib.makeLibraryPath [
      # load external libraries that you need in your rust project here
      libxkbcommon
      wayland-scanner.out
      libGL
      wayland
    ];

  # Add precompiled library to rustc search path
  RUSTFLAGS = (
    builtins.map (a: ''-L ${a}/lib'') [
      # add libraries here (e.g. pkgs.libvmi)
      pkgs.vulkan-headers
      pkgs.vulkan-loader
      pkgs.vulkan-validation-layers

    ]
  );

  VULKAN_SDK = "${pkgs.vulkan-headers}";
  VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d";
}
