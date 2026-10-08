#!/usr/bin/env python3
"""
Bundle the portable ZIP for Windows 7/8.x (x64).
Can be run on Linux (cross-compile) or Windows.

Packages:
- qymcad.exe (built with x86_64-win7-windows-msvc)
- combase.dll export forwarder (resolves CoCreateFreeThreadedMarshaler to ole32.dll on Win7)
- OpenCASCADE (OCCT) DLLs
- 3rdparty dependencies (TBB, jemalloc, FreeImage, FreeType, zlib, lzma, d3dcompiler)
- Visual C++ 2015-2022 CRT runtime DLLs + VC_redist.x64.exe
- assets/ (fonts, icons, themes, logo)
- Licenses and documentation (README.txt)
"""

import os
import sys
import shutil
import subprocess
import zipfile
import re

def get_version():
    custom_ver = os.environ.get("QYMCAD_VERSION")
    if custom_ver:
        return custom_ver.lstrip("v")
    
    with open("Cargo.toml", "r", encoding="utf-8") as f:
        for line in f:
            if line.startswith("version"):
                m = re.search(r'"([^"]+)"', line)
                if m:
                    ver = m.group(1)
                    try:
                        sha = subprocess.check_output(
                            ["git", "rev-parse", "--short=9", "HEAD"],
                            text=True, stderr=subprocess.DEVNULL
                        ).strip()
                        return f"{ver}-dev.{sha}"
                    except Exception:
                        return ver
    return "0.1.0"

def build_combase_forwarder(out_dir):
    combase_dll = os.path.join(out_dir, "combase.dll")
    c_src = os.path.join("packaging", "win", "combase_forwarder.c")
    obj_file = os.path.join(out_dir, "combase.obj")
    
    print(">>> Building combase.dll forwarder...")
    if shutil.which("clang") and shutil.which("lld-link"):
        subprocess.check_call([
            "clang", "--target=x86_64-pc-windows-msvc", "-c", c_src, "-o", obj_file
        ])
        subprocess.check_call([
            "lld-link", "/dll", "/machine:x64", "/entry:DllMainCRTStartup",
            obj_file, f"/out:{combase_dll}"
        ])
        if os.path.exists(obj_file):
            os.remove(obj_file)
    elif shutil.which("cl") and shutil.which("link"):
        subprocess.check_call([
            "cl.exe", "/nologo", "/LD", "/O2", f"/Fe:{combase_dll}", c_src,
            "/link", "/MACHINE:X64", "/NOENTRY"
        ])
        for cleanup in [obj_file, os.path.join(out_dir, "combase.lib"), os.path.join(out_dir, "combase.exp")]:
            if os.path.exists(cleanup):
                os.remove(cleanup)
    else:
        raise RuntimeError("Neither (clang + lld-link) nor (cl + link) found to build combase.dll")

def patch_tbb_for_win7(out_dir):
    """Patch tbb12.dll if present to remove dependency on Windows 8+ GetCurrentThreadStackLimits API."""
    for fname in ["tbb12.dll", "tbb12_debug.dll"]:
        tbb_path = os.path.join(out_dir, fname)
        if not os.path.exists(tbb_path):
            continue
        with open(tbb_path, "rb") as f:
            data = bytearray(f.read())

        # Replace GetCurrentThreadStackLimits in import directory with GetTickCount
        old_sym = b"\x38\x02GetCurrentThreadStackLimits\x00"
        new_sym = b"\x00\x00GetTickCount\x00" + b"\x00" * (len(old_sym) - len(b"\x00\x00GetTickCount\x00"))
        pos = data.find(old_sym)
        if pos != -1:
            data[pos:pos+len(new_sym)] = new_sym
            print(f">>> Patched {fname}: replaced GetCurrentThreadStackLimits import with GetTickCount")

        # Replace stack size detection with standard 2 MB thread stack:
        # b8 00 00 20 00        mov eax, 0x200000 (2 MB)
        # 16x 90                nop padding
        # 48 8d 0d 98 22 04 00  lea rcx, [rip + 0x42298] (mutex address preserved exactly)
        # 5x 90                 nop padding
        orig_code = bytes.fromhex("488d542438488d4c2440ff15b44e0100488b442438488d0d98220400482b442440")
        prev_patch = bytes.fromhex("65488b04250800000065482b042510000000909090488d0d982204009090909090")
        c_pos = data.find(orig_code)
        if c_pos == -1:
            c_pos = data.find(prev_patch)
        if c_pos != -1:
            new_code = bytes.fromhex("b800002000" + "90" * 16 + "488d0d98220400" + "90" * 5)
            assert len(new_code) == 33
            data[c_pos:c_pos+len(new_code)] = new_code
            print(f">>> Patched {fname}: set default thread stack size to 2 MB")

        with open(tbb_path, "wb") as f:
            f.write(data)

def main():
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    os.chdir(repo_root)

    bin_path = os.environ.get("QYMCAD_BIN", "target/x86_64-win7-windows-msvc/release/qymcad.exe")
    if not os.path.exists(bin_path):
        # Fallback check
        bin_path = "target/release/qymcad.exe"
        if not os.path.exists(bin_path):
            print(f"ERROR: Executable not found at {bin_path}", file=sys.stderr)
            sys.exit(1)

    ver = get_version()
    pkg_name = f"qymcad-{ver}-win7-x64"
    out_dir = os.path.join("dist", "qymcad-win7")
    dist_dir = "dist"
    os.makedirs(dist_dir, exist_ok=True)

    if os.path.exists(out_dir):
        shutil.rmtree(out_dir)
    os.makedirs(out_dir, exist_ok=True)

    print(f">>> Staging binary: {bin_path}")
    shutil.copy2(bin_path, os.path.join(out_dir, "qymcad.exe"))

    # Build combase.dll export forwarder
    build_combase_forwarder(out_dir)

    # Copy OCCT & 3rdparty DLLs
    occt_root = os.environ.get("OCCT_ROOT", "")
    sources_to_check = [
        "dist/qymcad-windows-x64",
        occt_root,
        os.path.join(occt_root, "bin") if occt_root else "",
        os.path.join("target", "occt-win64", "opencascade-7.9.3-vc14-64", "win64", "vc14", "bin"),
    ]

    copied_dlls = set()
    for src in sources_to_check:
        if src and os.path.isdir(src):
            for fname in os.listdir(src):
                fl = fname.lower()
                if fl.endswith(".dll") and fl != "combase.dll":
                    # Exclude huge unused frameworks (VTK, Qt5), debug builds and WinRT helpers
                    if (fl.startswith("vtk") or fl.startswith("qt") or
                        (fl.startswith("q") and not fl.startswith("qymcad")) or
                        "debug" in fl or fl.endswith("d.dll") or
                        fl in ["vccorlib140.dll", "vccorlib140d.dll"]):
                        continue
                    dest = os.path.join(out_dir, fname)
                    if not os.path.exists(dest):
                        shutil.copy2(os.path.join(src, fname), dest)
                        copied_dlls.add(fname)

    # Check 3rdparty dir in target/occt-win64 if needed
    thirdparty_dir = os.path.join("target", "occt-win64", "3rdparty")
    if os.path.isdir(thirdparty_dir):
        for root, dirs, files in os.walk(thirdparty_dir):
            for f in files:
                fl = f.lower()
                if fl.endswith(".dll"):
                    # Exclude huge unused frameworks (VTK, Qt5), debug builds and WinRT helpers
                    if (fl.startswith("vtk") or fl.startswith("qt") or
                        (fl.startswith("q") and not fl.startswith("qymcad")) or
                        "debug" in fl or fl.endswith("d.dll") or
                        fl in ["vccorlib140.dll", "vccorlib140d.dll"]):
                        continue
                    dest = os.path.join(out_dir, f)
                    if not os.path.exists(dest):
                        shutil.copy2(os.path.join(root, f), dest)
                        copied_dlls.add(f)

    occt_count = sum(1 for f in os.listdir(out_dir) if f.startswith("TK") and f.lower().endswith(".dll"))
    if occt_count == 0:
        raise RuntimeError("No OCCT libraries found! Check OCCT_ROOT or target/occt-win64.")
    print(f">>> OCCT libraries: {occt_count}")

    # Patch TBB for Windows 7 compatibility
    patch_tbb_for_win7(out_dir)

    # Copy assets
    assets_src = "assets"
    if os.path.isdir(assets_src):
        shutil.copytree(assets_src, os.path.join(out_dir, "assets"), dirs_exist_ok=True)
        print(">>> Copied assets directory")

    # Copy license & notices
    for doc in ["LICENSE", "LICENSE.txt"]:
        if os.path.exists(doc):
            shutil.copy2(doc, os.path.join(out_dir, "LICENSE.txt"))
            break
    if os.path.exists("THIRD-PARTY-NOTICES.md"):
        shutil.copy2("THIRD-PARTY-NOTICES.md", os.path.join(out_dir, "THIRD-PARTY-NOTICES.md"))

    # Write README.txt (English)
    readme_en = f"""QymCAD - Portable build for Windows 7 SP1 / 8 / 8.1 / 10 / 11 (x64)
Version: {ver}

How to run:
1. Run `qymcad.exe`. All required dependencies and libraries are included in this folder.
2. If the application fails to start on a fresh Windows 7 installation, run `VC_redist.x64.exe`
   (Microsoft Visual C++ 2015-2022 Redistributable) to install the Universal C Runtime.

System Requirements:
- Windows 7 SP1 (64-bit), Windows 8/8.1 (64-bit), or Windows 10/11 (64-bit).
- On Windows 7: Service Pack 1 and KB2999226 (Universal CRT) are required.
- Dedicated or integrated GPU with OpenGL 3.3+ / DirectX 11 support.

Notice:
This build provides compatibility with legacy Windows versions (starting with Windows 7 SP1).
Operation on Windows 7 and 8 is provided as-is without guarantee of full stability or frequent updates.
For Windows 10/11, the primary `qymcad-win64` release is recommended.
"""
    with open(os.path.join(out_dir, "README.txt"), "w", encoding="utf-8") as f:
        f.write(readme_en)

    # Create ZIP archive
    zip_path = os.path.join(dist_dir, f"{pkg_name}.zip")
    if os.path.exists(zip_path):
        os.remove(zip_path)

    print(f">>> Creating archive: {zip_path}")
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
        for root, dirs, files in os.walk(out_dir):
            for file in files:
                full_path = os.path.join(root, file)
                rel_path = os.path.relpath(full_path, out_dir)
                zf.write(full_path, rel_path)

    zip_size_mb = os.path.getsize(zip_path) / (1024 * 1024)
    print(f">>> DONE: {zip_path} ({zip_size_mb:.1f} MB, {len(os.listdir(out_dir))} files staged)")

    stable_zip = os.path.join(dist_dir, "qymcad-windows7-x64.zip")
    shutil.copy2(zip_path, stable_zip)
    print(f">>> Updated stable archive: {stable_zip}")

if __name__ == "__main__":
    main()
