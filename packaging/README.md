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

The build runs under MSVC - the mainstream Rust target on Windows, and the compiler the kernel is built
with. The C++ ABI does not mix, so the bridge and OCCT must come from the same toolchain.

OCCT is built from source, the same 7.8.1 with the same modules switched off as on Linux. In a
Developer PowerShell for VS 2022:

```powershell
curl.exe -sSL -o occt.tar.gz https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V7_8_1.tar.gz
tar xf occt.tar.gz
cmake -S OCCT-V7_8_1 -B occt-build -A x64 `
  -DBUILD_LIBRARY_TYPE=Shared -DINSTALL_DIR=C:/occt -DINSTALL_DIR_LAYOUT=Unix `
  -DBUILD_MODULE_Draw=OFF -DBUILD_MODULE_Visualization=OFF -DBUILD_DOC_Overview=OFF `
  -DUSE_FREETYPE=OFF -DUSE_TK=OFF -DUSE_FREEIMAGE=OFF `
  -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_OPENGL=OFF -DUSE_GLES2=OFF
cmake --build occt-build --config Release --target install -- /m
```

`INSTALL_DIR_LAYOUT=Unix` is not cosmetic: by default OCCT installs into `inc` and `win64/vc14/lib` on
Windows, and then every place that names a path would need two answers. With Unix the layout is the one
Linux already has.

Then the program itself:

```powershell
$env:OCCT_INCLUDE_DIR = "C:\occt\include\opencascade"
$env:OCCT_LIB_DIR     = "C:\occt\lib"
$env:OCCT_ROOT        = "C:\occt"
cargo build --release --bin qymcad
pwsh -File packaging/win/bundle.ps1
```

- Windows: the Rust toolchain (1.88 or newer is needed), and whether the list of DLLs from
  `ldd` is complete.

## Later (closer to an alpha)

`.deb` and Flatpak on Linux, an NSIS or MSI installer on Windows - the same artifacts wrapped into one CI run
on a tag.
