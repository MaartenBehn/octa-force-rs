{ pkgs ? ( import <nixpkgs> {}), ... }:

pkgs.mkShell {

  name = "octa-force";

  packages = with pkgs; [
    cmake

    # For performance profile
    perf
    hotspot

    # For dependency graph
    graphviz

    # Shader debug
    spirv-tools
    renderdoc

    # for vulkaninfo
    vulkan-tools
  ];

  # Use faster linker for local build 
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.clang}/bin/clang";

  RUSTFLAGS = 
    # Use faster linker     
    [''-C link-arg=-fuse-ld=${pkgs.mold-wrapped}/bin/mold''] ++

    (builtins.map (a: '' -L ${a}/lib'') [
      pkgs.vulkan-loader
    ]);

  LD_LIBRARY_PATH =with pkgs; lib.makeLibraryPath [
    # load external libraries that you need in your rust project
    libxkbcommon
    #wayland-scanner.out
    #libGL
    wayland
    #vulkan-headers 
    #vulkan-loader
    #vulkan-validation-layers

    # For renderdoc x11 fallback
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
  ];

  VULKAN_SDK = "${pkgs.vulkan-headers}";
  VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d";
}
