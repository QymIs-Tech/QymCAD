# Building the QymCAD packages

The dev workflow is untouched: `cargo run` / `cargo test` as always. Packages are a separate `--release` path.

The one thing that governs all of this: the application links against **OCCT 7.9.3**, so a package carries
OCCT inside itself rather than "depending on the system libocct".

## The kernel

Every build we make compiles OCCT 7.9.3 from source, the same way on every system:

```bash
curl -sSL -o occt.tar.gz https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V7_9_3.tar.gz
tar xf occt.tar.gz
cmake -S OCCT-7_9_3 -B occt-build \
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DCMAKE_BUILD_TYPE=Release -DBUILD_LIBRARY_TYPE=Shared \
  -DINSTALL_DIR=/opt/occt -DINSTALL_DIR_LAYOUT=Unix \
  -DBUILD_RELEASE_DISABLE_EXCEPTIONS=OFF \
  -DBUILD_MODULE_Draw=OFF -DBUILD_MODULE_Visualization=OFF -DBUILD_DOC_Overview=OFF \
  -DUSE_FREETYPE=OFF -DUSE_TK=OFF -DUSE_FREEIMAGE=OFF \
  -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_OPENGL=OFF -DUSE_GLES2=OFF
cmake --build occt-build --target install -j"$(nproc)"
```

Then `OCCT_INCLUDE_DIR=/opt/occt/include/opencascade OCCT_LIB_DIR=/opt/occt/lib cargo build --release`.

Three things in it are not a matter of taste. A package of OCCT from a system may differ in any of them, and the
program built against it is then not the one we test:

- **The version, 7.9.3.** The bridge to the kernel (`crates/qymcad-kernel/src/occt_*.cpp`) stands on how this version
  behaves; OCCT has changed behaviour between versions under the same calls. A build against another version prints a
  warning (`crates/qymcad-kernel/build.rs`) and goes on, at the builder's own risk.
- **Exceptions on** (`BUILD_RELEASE_DISABLE_EXCEPTIONS` set to `OFF`). With OCCT's default (`ON`) a Release build
  compiles its own checks out: a degenerate input is not refused but runs on into the algorithm, and the bridge has
  nothing to catch.
- **Signals converted** (`OCC_CONVERT_SIGNALS`, OCCT's default everywhere but MSVC; leave it as it is). The bridge turns
  a fault inside OCCT - a segmentation fault, a division by zero - into a refusal, which works only when OCCT itself is
  built with it. Without it such a fault closes the program. On MSVC the bridge is compiled with `/EHa` instead.

## Linux - AppImage (locally, through Docker)

One self-contained file that runs on Ubuntu 22.04+, Debian 12+, Fedora 36+ and Arch **with no OCCT
installed**.

```bash
just pkg-linux            # builds the image (slow the first time: OCCT is compiled) -> dist/qymcad-*.AppImage
# sending the file to someone:  chmod +x qymcad-*.AppImage && ./qymcad-*.AppImage
```

Why Ubuntu 22.04 (glibc 2.35) as the base: an AppImage runs on a glibc **no older** than the one it was built
against. Building on a fresh distribution is therefore not an option - the binary would not start on someone
else's 24.04 LTS. OCCT 7.9.3 is compiled from source in the same image (apt on 22.04 carries only 7.5). All of
it lives in `packaging/linux/Dockerfile`.

Requirements on the host: `docker`. Rust and OCCT need not be installed - they are in the image.

## Windows - portable ZIP (GitHub Actions)

The `.exe` with every DLL it needs beside it; unpack and run (Win10/11 x64).

- **Push a tag**: `git tag v0.1.0 && git push origin v0.1.0` -> CI builds it and attaches `qymcad-win64.zip`
  to the Release.
- **By hand**: GitHub -> Actions -> `release` -> Run workflow -> download the `qymcad-win64` artifact.

The build runs under MSVC - the mainstream Rust target on Windows, and the compiler the kernel is built
with. The C++ ABI does not mix, so the bridge and OCCT must come from the same toolchain.

OCCT is built from source, the same 7.9.3 with the same flags as on Linux (see "The kernel"). In a
Developer PowerShell for VS 2022:

```powershell
curl.exe -sSL -o occt.tar.gz https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V7_9_3.tar.gz
tar xf occt.tar.gz
cmake -S OCCT-7_9_3 -B occt-build -A x64 `
  "-DCMAKE_POLICY_VERSION_MINIMUM=3.5" `
  -DBUILD_LIBRARY_TYPE=Shared -DINSTALL_DIR=C:/occt -DINSTALL_DIR_LAYOUT=Unix `
  -DBUILD_RELEASE_DISABLE_EXCEPTIONS=OFF `
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

## macOS (Apple Silicon and Intel)

Two builds, `qymcad-*-macos-arm64` and `qymcad-*-macos-x86_64`, each built natively on a runner
of its own kind (`macos-15` and `macos-15-intel`). The Intel runner is the last one GitHub offers and is
supported until August 2027; after that the Intel build has to become a cross-build on Apple Silicon.

The script names the packages after the processor it runs on (`uname -m`); `MACOS_ARCH=arm64|x86_64`
overrides that.

The kernel is built from source exactly as on the other two systems, then:

```bash
export OCCT_INCLUDE_DIR=/Users/you/occt/include/opencascade
export OCCT_LIB_DIR=/Users/you/occt/lib
export OCCT_ROOT=/Users/you/occt
cargo build --release --bin qymcad
bash packaging/macos/bundle.sh
```

It leaves two packages in `dist/` holding the same signed bundle and the same notes: `*-macos-<arch>.zip`, and
`*-macos-<arch>.dmg`, a disk image that shows the program beside a link to Applications.

**What makes a mac bundle different.** A dylib carries the path it was BUILT at, baked into whatever
loads it - so a copied library is looked for where it used to live, and the program dies on start. The
script rewrites those paths to `@rpath` with `install_name_tool` and points `@rpath` at the bundle's own
`Frameworks`. It then checks that nothing still names the build machine: one path left over means the
program starts here and nowhere else, and it says so only on somebody else's computer.

**The build is not signed by Apple.** `bundle.sh` signs it - with the project's self-signed certificate in
CI, ad hoc otherwise - so the bundle verifies, but no certificate Apple issued vouches for it. On the first
launch macOS refuses it: "Apple could not verify QymCAD is free of malware". System Settings -> Privacy &
Security then lists it with an "Open Anyway" button; one press and the password, and it opens from then on.
The notes beside the app walk a person through that in both languages, step by step, because whoever is not
told deletes the program instead. The Terminal steps (`xattr -cr`) were the way round a broken signature,
which macOS called damaged and offered no button for; with a signature that verifies they are not needed.

Nothing free removes the step itself: only notarisation does, and notarisation needs the paid Apple
Developer Program.

**The script runs on any machine, not only on a mac.** `crates/qymcad/src/packaging_macos.rs` puts stub
`otool` and `install_name_tool` on PATH and runs this very script over a sandbox tree, in three shapes:
paths already `@rpath`, paths naming the build machine and rewritten, and a path that refuses to be
rewritten - where the sentinel must refuse. Every mistake found in this script so far would have been
caught there in a fifth of a second instead of six minutes of the most expensive runner there is.

**What the stubs do not prove:** that `install_name_tool` rewrites a real Mach-O, that `@rpath` resolves
in a live dyld, and that the `.app` starts at all. Those need a mac.
