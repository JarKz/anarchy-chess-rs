{
  description = "Noti Application";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane = {
      url = "github:ipetkov/crane";
    };

    flake-utils = {
      url = "github:numtide/flake-utils";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    {
      inherit
        (flake-utils.lib.eachSystem [ "x86_64-linux" ] (
          system:
          let
            pkgs = import nixpkgs {
              inherit system;
              overlays = [
                rust-overlay.overlays.default
              ];
            };

            rustToolchain = pkgs.rust-bin.stable.latest.default.override {
              extensions = [
                "rust-analyzer"
                "rust-src"
                "clippy"
              ];
            };

            rustNightlyToolchain = pkgs.rust-bin.selectLatestNightlyWith (
              toolchain:
              toolchain.default.override {
                extensions = [
                  "rust-analyzer"
                  "rust-src"
                  "rust-std"
                  "clippy"
                ];
              }
            );

            buildInputsWith = (
              additionalPackages:
              with pkgs;
              [
                libxkbcommon
                libudev-zero
                libx11
                libxcursor
                libxi
                libxrandr
                alsa-lib
                wayland
                vulkan-loader
                vulkan-tools
                freetype
                fontconfig
              ]
              ++ additionalPackages
            );

            nativeBuildInputsWith = (
              additionalPackages:
              with pkgs;
              [
                pkg-config

                vulkan-loader
                vulkan-tools
                libxkbcommon
                libudev-zero
                libx11
                libxcursor
                libxi
                libxrandr
                alsa-lib
                libGL
                wayland
                freetype
                fontconfig
              ]
              ++ additionalPackages
            );

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
              with pkgs;
              [
                vulkan-loader
                libx11
                libxcursor
                libxkbcommon
                wayland
              ]
            );

            mkDevShell = (
              rustToolchain:
              pkgs.mkShell {
                packages = with pkgs; [
                  llvmPackages_21.libllvm
                ];

                buildInputsWith = buildInputsWith [ rustToolchain ];
                nativeBuildInputs = nativeBuildInputsWith [ rustToolchain ];

                shellHook = ''
                  export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}"
                  export RUSTFLAGS="-L ${pkgs.libGL}/lib -lGL";
                  zsh
                '';
              }
            );

          in
          {
            devShells = {
              default = mkDevShell rustToolchain;
              nightly = mkDevShell rustNightlyToolchain;
            };

            checks = {
              build = self.packages."${system}".default;
            };
          }
        ))
        devShells
        checks
        ;

    };
}
