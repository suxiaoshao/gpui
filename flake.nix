{
  description = "gpui development environment";
  inputs = {
    nixpkgs.url = "git+https://github.com/NixOS/nixpkgs?ref=nixos-26.05&shallow=1";
    rust-overlay = {
      url = "git+https://github.com/oxalica/rust-overlay?ref=master&shallow=1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
    in
    {
      formatter = forAllSystems (system: (pkgsFor system).nixfmt);
      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          inherit (pkgs) lib;
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          # Metal comes from the selected host Xcode, not Nix's SDK-only xcrun.
          appleTools = pkgs.writeShellScriptBin "xcrun" ''
            exec /usr/bin/env -u SDKROOT -u DEVELOPER_DIR /usr/bin/xcrun "$@"
          '';
          graphics = with pkgs; [
            wayland
            libxkbcommon
            libGL
            vulkan-loader
            libx11
            libxcursor
            libxi
            libxrandr
          ];
          linuxLibraries =
            with pkgs;
            graphics
            ++ [
              fontconfig
              freetype
              openssl
              zstd
              libxcb
              # Jaco's get-selected-text -> enigo xdo backend links libxdo.
              xdotool
              alsa-lib
              pipewire
              libgbm
            ];
        in
        {
          default = pkgs.mkShell {
            packages = [
              rust
              pkgs.pkg-config
              pkgs.cmake
              pkgs.nodejs_24
              pkgs.python3
            ];
            nativeBuildInputs = [ pkgs.rustPlatform.bindgenHook ];
            shellHook = lib.optionalString pkgs.stdenv.isDarwin ''
              export PATH="${appleTools}/bin:$PATH"
            '';
            buildInputs = [ pkgs.sqlite ] ++ lib.optionals pkgs.stdenv.isLinux linuxLibraries;
            LD_LIBRARY_PATH = lib.optionalString pkgs.stdenv.isLinux (lib.makeLibraryPath linuxLibraries);
          };
        }
      );
    };
}
