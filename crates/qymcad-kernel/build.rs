//! Building the C-ABI bridge to a system OpenCASCADE, without vendoring it from source.
//!
//! An installed OCCT is required, with its headers and libraries in place. Other systems and paths are handled
//! through the `OCCT_INCLUDE_DIR` and `OCCT_LIB_DIR` environment variables.

use std::env;

// The C++ runtime is named differently on every toolchain, and the choice is checked by unit tests in the
// crate itself - so it lives in one file that both sides read rather than being written out twice.
include!("src/cxx_runtime.rs");

fn main() {
    let inc = env::var("OCCT_INCLUDE_DIR").unwrap_or_else(|_| "/usr/include/opencascade".into());
    let libdir = env::var("OCCT_LIB_DIR").unwrap_or_else(|_| "/usr/lib".into());

    // WHICH TOOLCHAIN IS ON THE OTHER SIDE. The C++ ABI does not mix: the bridge must be compiled by the
    // same compiler that built OCCT. On Windows that is MSVC - the mainstream target for Rust there, and
    // the one every desktop CAD is built with.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let msvc = target_env == "msvc";

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        // `M_PI` and its relatives are hidden under strict C++17 on some toolchains, so they are enabled
        // explicitly; where they are defined anyway the define is harmless. Without it the OCCT headers do not
        // compile.
        .define("_USE_MATH_DEFINES", None)
        .include(&inc)
        .file("src/occt_bridge.cpp")
        .file("src/occt_helical.cpp")
        .file("src/occt_io.cpp")
        .warnings(false);
    if msvc {
        // `windows.h` declares `min` and `max` as MACROS, and the OCCT headers use `std::min`/`std::max` -
        // they do not compile together until the macros are switched off. `WIN32_LEAN_AND_MEAN` keeps the
        // rest of that header out of the way and shortens the compile besides.
        build.define("NOMINMAX", None).define("WIN32_LEAN_AND_MEAN", None);
    }
    build.compile("qym_occt_bridge");

    println!("cargo:rustc-link-search=native={libdir}");
    // OCCT 7.8 and later use consolidated modules.
    for lib in [
        "TKDESTEP", "TKXSBase", "TKDE", "TKMesh", "TKShHealing", "TKFillet", "TKOffset", "TKBool", "TKPrim", "TKBO",
        // TKFeat holds `BRepFeat_SplitShape`, which splits faces without cutting the body
        "TKFeat", "TKGeomAlgo", "TKTopAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
    // THE C++ RUNTIME, AND ONLY WHERE IT HAS A NAME - see `cxx_runtime` for which name and why.
    if let Some(lib) = cxx_runtime(&target_os, &target_env) {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
    // `src/cxx_runtime.rs` is on this list because it is `include!`d rather than imported: cargo does not
    // see through the macro, and without the line an edit there would not rebuild the script.
    for f in ["src/occt_bridge.cpp", "src/occt_helical.cpp", "src/occt_io.cpp", "src/occt_common.hpp", "src/cxx_runtime.rs"] {
        println!("cargo:rerun-if-changed={f}");
    }
    println!("cargo:rerun-if-env-changed=OCCT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=OCCT_LIB_DIR");
}
