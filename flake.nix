{
  description = "QymCAD - parametric CAD on a B-rep kernel (OpenCASCADE), built entirely from source";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;

        # -------------------------------------------------------------
        # OCCT 7.8.1, built from source with exactly the module set that
        # crates/qymcad-kernel/build.rs and packaging/linux/Dockerfile
        # use: Draw and Visualization off (QymCAD does its own drawing
        # in egui/glow), plus FreeType/TK/FreeImage/RapidJSON/Draco/
        # OpenGL/GLES2 all off. Ubuntu 22.04's apt only carries OCCT 7.5,
        # and build.rs needs the merged 7.8 modules (TKDESTEP and
        # friends), so it must be built from source rather than taken
        # from nixpkgs' own `opencascade-occt`.
        # -------------------------------------------------------------
        occt = pkgs.stdenv.mkDerivation {
          pname = "occt-qymcad";
          version = "7.8.1";

          src = pkgs.fetchFromGitHub {
            owner = "Open-Cascade-SAS";
            repo = "OCCT";
            rev = "V7_8_1";
            hash = "sha256-tg71cFx9HZ471T/3No9CeEHi8VSo0ZITIuNfTSNB2qU=";
          };

          nativeBuildInputs = [ pkgs.cmake pkgs.ninja pkgs.pkg-config ];
          # OCCT's CMake unconditionally links GL/X11/Xmu/Xi/Xext into several
          # base (non-Visualization) TK modules such as TKV3d and TKService,
          # regardless of USE_OPENGL/BUILD_MODULE_Visualization — so these are
          # needed purely to satisfy the linker, even though QymCAD itself
          # never calls into them (its own drawing is done in egui/glow).
          buildInputs = [
            pkgs.libx11
            pkgs.libxext
            pkgs.libxmu
            pkgs.libxi
            pkgs.libGL
            pkgs.libglvnd
            pkgs.freetype
            pkgs.fontconfig
          ];

          cmakeFlags = [
            "-DCMAKE_POLICY_VERSION_MINIMUM=3.5"
            "-DBUILD_LIBRARY_TYPE=Shared"
            "-DBUILD_MODULE_Draw=OFF"
            "-DBUILD_MODULE_Visualization=OFF"
            "-DBUILD_DOC_Overview=OFF"
            "-DUSE_FREETYPE=OFF"
            "-DUSE_TK=OFF"
            "-DUSE_FREEIMAGE=OFF"
            "-DUSE_RAPIDJSON=OFF"
            "-DUSE_DRACO=OFF"
            "-DUSE_OPENGL=OFF"
            "-DUSE_GLES2=OFF"
          ];

          preConfigure = ''
            cmakeFlagsArray+=("-DINSTALL_DIR=$out")
          '';

          enableParallelBuilding = true;
        };

        # Linked at runtime by egui/glow (OpenGL) + winit (both X11 and
        # Wayland backends are on in qymcad-render's Cargo.toml), plus dbus
        # for rfd's xdg-desktop-portal file-chooser backend (rfd 0.12+ talks
        # to the portal via zbus, no GTK needed).
        runtimeLibs = with pkgs; [
          libGL
          libxkbcommon
          wayland
          libx11
          libxcursor
          libxi
          libxrandr
          libxext
          dbus
          vulkan-loader
        ];

        qymcad = pkgs.rustPlatform.buildRustPackage {
          pname = "qymcad";
          version = "0.1.0";

          # The flake lives inside the repo, next to Cargo.toml.
          src = lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let base = baseNameOf path; in
              base != "target" && base != ".git" && base != "dist";
          };

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.autoPatchelfHook
          ];

          buildInputs = [ occt ] ++ runtimeLibs;

          # Read directly by crates/qymcad-kernel/build.rs.
          OCCT_INCLUDE_DIR = "${occt}/include/opencascade";
          OCCT_LIB_DIR = "${occt}/lib";

          # Mirrors `cargo build --release --bin qymcad`.
          cargoBuildFlags = [ "--bin" "qymcad" ];

          # No sandboxed display/GPU/dbus session is available in the build
          # sandbox, so tests that need a window or a portal can't run here;
          # use `nix develop` + `cargo test` (== `just test`) for those.
          doCheck = false;

          # autoPatchelfHook fixes up the ELF rpath against buildInputs
          # automatically; OCCT's lib dir is added explicitly since it's
          # not a "well-known" nixpkgs output path.
          # winit's wayland/x11 backends and rfd's portal backend load several
          # of these via dlopen at runtime rather than linking them normally,
          # so autoPatchelfHook can't discover them from the ELF alone — they
          # have to be added to the rpath by hand.
          appendRunpaths = [
            "${occt}/lib"
            "${pkgs.wayland}/lib"
            "${pkgs.libxkbcommon}/lib"
            "${pkgs.libGL}/lib"
            "${pkgs.libglvnd}/lib"
            "${pkgs.dbus}/lib"
            "${pkgs.libx11}/lib"
            "${pkgs.libxcursor}/lib"
            "${pkgs.libxi}/lib"
            "${pkgs.libxrandr}/lib"
            "${pkgs.vulkan-loader}/lib"
          ];

          meta = {
            description = "Parametric 3D CAD with a real B-rep kernel (OpenCASCADE)";
            homepage = "https://github.com/QymIs-Tech/QymCAD";
            license = lib.licenses.agpl3Plus;
            mainProgram = "qymcad";
            platforms = lib.platforms.linux;
          };
        };
      in
      {
        packages = {
          default = qymcad;
          inherit qymcad occt;
        };

        apps.default = {
          type = "app";
          program = "${qymcad}/bin/qymcad";
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.clippy
            pkgs.rustfmt
            pkgs.pkg-config
          ] ++ runtimeLibs;

          buildInputs = [ occt ];

          OCCT_INCLUDE_DIR = "${occt}/include/opencascade";
          OCCT_LIB_DIR = "${occt}/lib";
          LD_LIBRARY_PATH = lib.makeLibraryPath ([ occt ] ++ runtimeLibs);

          shellHook = ''
            echo "QymCAD dev shell — OCCT ${occt.version} at ${occt}"
            echo "cargo run --bin qymcad   /   cargo test   (as in the justfile: 'just dev' / 'just test')"
          '';
        };
      });
}
