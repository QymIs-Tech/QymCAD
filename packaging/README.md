# Building the QymCAD packages

The dev workflow is untouched: `cargo run` / `cargo test` as always. Packages are a separate `--release` path.

The one thing that governs all of this: the application links against **OCCT 7.8+** (the merged modules,
`TKDESTEP` and the rest), so a package carries OCCT inside itself rather than "depending on the system
libocct".

## Linux - AppImage (locally, through Docker)

One self-contained file that runs on Ubuntu 22.04+, Debian 12+, Fedora 36+ and Arch **with no OCCT
installed**.

```bash
just pkg-linux            # builds the image (slow the first time: OCCT is compiled) -> dist/qymcad-*.AppImage
# sending the file to someone:  chmod +x qymcad-*.AppImage && ./qymcad-*.AppImage
```

Why Ubuntu 22.04 (glibc 2.35) as the base: an AppImage runs on a glibc **no older** than the one it was built
against. Building on a fresh distribution is therefore not an option - the binary would not start on someone
else's 24.04 LTS. OCCT 7.8.1 is compiled from source in the same image (apt on 22.04 carries only 7.5). All of
it lives in `packaging/linux/Dockerfile`.

Requirements on the host: `docker`. Rust and OCCT need not be installed - they are in the image.

## Windows - portable ZIP (GitHub Actions)

The `.exe` with every DLL it needs beside it; unpack and run (Win10/11 x64).

- **Push a tag**: `git tag v0.1.0 && git push origin v0.1.0` -> CI builds it and attaches `qymcad-win64.zip`
  to the Release.
- **By hand**: GitHub -> Actions -> `release` -> Run workflow -> download the `qymcad-win64` artifact.

The build runs under MSYS2/UCRT64 (Rust and OCCT from one gnu toolchain). The config is
`.github/workflows/release.yml`.

### Locally in a Win10 VM (without CI)

In the MSYS2 (UCRT64) shell:

```bash
pacman -S mingw-w64-ucrt-x86_64-{toolchain,rust,opencascade,pkgconf} zip
export OCCT_INCLUDE_DIR=/ucrt64/include/opencascade OCCT_LIB_DIR=/ucrt64/lib
# Rust carries a libmsvcrt.a of its own, older than the system libmingwex.a it gets linked beside;
# naming the system one by full path is the workaround from rust-lang/rust#60912.
export RUSTFLAGS="-C link-arg=/ucrt64/lib/libmsvcrt.a"
cargo build --release --bin qymcad
bash packaging/win/bundle.sh        # -> dist/qymcad-win64.zip
```

## The first real run - what to watch

These scripts were never exercised in a sandbox (no Docker, no Windows there), so the first genuine run may
call for corrections:

- Linux: the names of the OCCT modules and the cmake flags, the set of dev `.so` files for egui, the version
  of linuxdeploy.
- Windows: the version of `mingw-w64-ucrt-x86_64-rust` (1.88 or newer is needed), and whether the list of DLLs from
  `ldd` is complete.

## Later (closer to an alpha)

`.deb` and Flatpak on Linux, an NSIS or MSI installer on Windows - the same artifacts wrapped into one CI run
on a tag.
