{
  description = "QymCAD development and package environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      workspace = (fromTOML (builtins.readFile ./Cargo.toml)).workspace.package;
      application = (fromTOML (builtins.readFile ./crates/qymcad/Cargo.toml)).package;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;
          occt = pkgs.opencascade-occt;
          runtimeLibraries = with pkgs; [
            libGL
            libxkbcommon
            vulkan-loader
            wayland
            libx11
            libxcursor
            libxi
            libxrandr
          ];
          qymcad = pkgs.rustPlatform.buildRustPackage {
            pname = application.name;
            inherit (workspace) version;

            src = lib.cleanSourceWith {
              src = ./.;
              filter =
                path: type:
                let
                  name = baseNameOf path;
                in
                lib.cleanSourceFilter path type && name != "flake.nix" && name != "flake.lock";
            };
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [
              "--bin"
              application.name
            ];

            nativeBuildInputs = with pkgs; [
              makeWrapper
              pkg-config
            ];
            buildInputs = runtimeLibraries ++ [
              occt
              pkgs.fontconfig
            ];

            OCCT_INCLUDE_DIR = "${lib.getDev occt}/include/opencascade";
            OCCT_LIB_DIR = "${lib.getLib occt}/lib";

            # The OCCT test suite uses the resource controls and scheduling in tools/gate.py.
            doCheck = false;

            postFixup = ''
              wrapProgram "$out/bin/${application.name}" \
                --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath runtimeLibraries}"
            '';

            meta = {
              inherit (application) description;
              inherit (workspace) homepage;
              changelog = "${workspace.repository}/releases";
              license = lib.licenses.agpl3Plus;
              mainProgram = application.name;
              maintainers = [ ];
              platforms = systems;
              sourceProvenance = [ lib.sourceTypes.fromSource ];
            };
          };
        in
        {
          inherit qymcad;
          default = qymcad;
        }
      );

      apps = forAllSystems (system: {
        qymcad = {
          type = "app";
          program = "${nixpkgs.lib.getExe self.packages.${system}.qymcad}";
          meta.description = application.description;
        };
        default = self.apps.${system}.qymcad;
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;
          occt = pkgs.opencascade-occt;
          runtimeLibraries = with pkgs; [
            libGL
            libxkbcommon
            vulkan-loader
            wayland
            libx11
            libxcursor
            libxi
            libxrandr
          ];
          rustTools = with pkgs; [
            cargo
            clippy
            rust-analyzer
            rustc
            rustfmt
          ];
          nixTools = with pkgs; [
            deadnix
            nixfmt
            statix
          ];
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.qymcad ];
            packages = rustTools ++ nixTools;

            OCCT_INCLUDE_DIR = "${lib.getDev occt}/include/opencascade";
            OCCT_LIB_DIR = "${lib.getLib occt}/lib";
            LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibraries;
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        }
      );

      formatter = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        pkgs.writeShellApplication {
          name = "qymcad-nixfmt";
          runtimeInputs = [ pkgs.nixfmt ];
          text = ''
            flake_dir="$PWD"
            while [[ "$flake_dir" != / && ! -f "$flake_dir/flake.nix" ]]; do
              flake_dir="$(dirname "$flake_dir")"
            done
            if [[ ! -f "$flake_dir/flake.nix" ]]; then
              echo "flake.nix was not found" >&2
              exit 1
            fi
            exec nixfmt "$flake_dir/flake.nix"
          '';
        }
      );

      checks = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          inherit (self.packages.${system}) qymcad;
          nix =
            pkgs.runCommand "qymcad-nix-checks"
              {
                nativeBuildInputs = with pkgs; [
                  deadnix
                  nixfmt
                  statix
                ];
              }
              ''
                nixfmt --check ${./flake.nix}
                statix check ${./flake.nix}
                deadnix --fail ${./flake.nix}
                touch "$out"
              '';
        }
      );
    };
}
