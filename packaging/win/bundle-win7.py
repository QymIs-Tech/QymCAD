#!/usr/bin/env python3
"""Build the portable ZIP for Windows 7 SP1 and later (x64), after the build for x86_64-win7-windows-msvc.

The one way this package is made, in CI and by hand, so what is tested is what ships. It reads OCCT_ROOT (the kernel
built from source, packaging/README.md) and QYMCAD_VERSION; the Visual C++ runtime comes from QYMCAD_VC_REDIST or
from the Visual Studio of this machine. Anything missing stops the build, and so does a package check-win7.py
refuses: an archive that does not start is worse than no archive.
"""
import glob
import hashlib
import os
import re
import shutil
import subprocess
import sys
import urllib.request
import zipfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BINARY = os.path.join("target", "x86_64-win7-windows-msvc", "release", "qymcad.exe")
STAGE = os.path.join("dist", "qymcad-win7")
# Nothing here imports it, and it imports WinRT libraries Windows 7 does not have.
NOT_CARRIED = {"vccorlib140.dll"}
# DRAWING ON THE PROCESSOR (crates/qymcad/src/software_gl.rs): Mesa's llvmpipe, one fixed build checked by its hash.
# 25.3.6 is the newest the loader of Windows 7 takes: 26.0.8, 26.1.1, 26.1.8 and 26.2.6 import
# GetSystemTimePreciseAsFileTime from kernel32.dll. The MinGW build, because the MSVC one of 26.2.6 carries no OpenGL.
# Of the archive only the two files the loader opens go in: opengl32.dll loads libgallium_wgl.dll and nothing else.
MESA_VERSION = "25.3.6"
MESA_ARCHIVE = f"mesa3d-{MESA_VERSION}-release-mingw.7z"
MESA_URL = f"https://github.com/pal1000/mesa-dist-win/releases/download/{MESA_VERSION}/{MESA_ARCHIVE}"
MESA_SHA256 = "95f8404e4f916bac4640ac4454bd5038bc1f7ceb6878354207911b1ef6715f94"
MESA_FILES = ["opengl32.dll", "libgallium_wgl.dll"]
RENDERER = os.path.join(STAGE, "software-gl")

NOTE_EN = """QymCAD - portable build for Windows 7 SP1 / 8 / 8.1 / 10 / 11 (x64).
To run: qymcad.exe (every DLL of the program is right here).
Windows 7 also needs update KB2999226 (Universal C Runtime) installed.
If the message names api-ms-win-crt-*.dll, that update is missing.
With no graphics driver the program draws on the processor (software-gl folder):
it works, slowly. Installing the driver of the card makes it fast.
On Windows 10/11 the qymcad-win64 package is the better choice.
"""
NOTE_RU = """QymCAD - переносимая сборка для Windows 7 SP1 / 8 / 8.1 / 10 / 11 (x64).
Запуск: qymcad.exe (все DLL программы лежат рядом).
Для Windows 7 также нужно обновление KB2999226 (Universal C Runtime).
Если в сообщении названа api-ms-win-crt-*.dll, этого обновления нет.
Без драйвера видеокарты программа рисует процессором (папка software-gl):
работает, но медленно. С драйвером видеокарты - быстро.
На Windows 10/11 лучше взять пакет qymcad-win64.
"""


def package_name():
    """A tag names the package; anything else is named by the manifest version plus the commit."""
    tag = os.environ.get("QYMCAD_VERSION")
    if tag:
        return "qymcad-" + tag.removeprefix("v")
    with open("Cargo.toml", encoding="utf-8") as f:
        version = re.search(r'^version\s*=\s*"([^"]+)"', f.read(), re.M).group(1)
    commit = subprocess.run(["git", "rev-parse", "--short=9", "HEAD"], capture_output=True, text=True).stdout.strip()
    return f"qymcad-{version}-dev.{commit}" if commit else f"qymcad-{version}"


def build_forwarder():
    """combase.dll, which Windows 7 lacks: its exports forward to ole32.dll (combase_forwarder.c)."""
    source = os.path.join("packaging", "win", "combase_forwarder.c")
    built = os.path.join(STAGE, "combase.dll")
    if not (shutil.which("clang") and shutil.which("lld-link")):
        sys.exit("clang and lld-link are needed to build combase.dll")
    object_file = os.path.join(STAGE, "combase.obj")
    subprocess.check_call(["clang", "--target=x86_64-pc-windows-msvc", "-c", source, "-o", object_file])
    subprocess.check_call(["lld-link", "/dll", "/machine:x64", "/entry:DllMainCRTStartup", object_file, f"/out:{built}"])
    for leftover in (object_file, os.path.join(STAGE, "combase.lib")):
        if os.path.exists(leftover):
            os.remove(leftover)


def copy_kernel():
    occt_root = os.environ.get("OCCT_ROOT")
    if not occt_root:
        sys.exit("OCCT_ROOT is not set")
    libraries = glob.glob(os.path.join(occt_root, "bin", "*.dll"))
    if not libraries:
        sys.exit(f"{occt_root}/bin holds no DLLs")
    for library in libraries:
        shutil.copy2(library, STAGE)
    print(f">>> OCCT libraries: {len(libraries)}")


def version_key(path):
    """14.44.35112 -> (14, 44, 35112) out of a runtime path, so 14.100 sorts after 14.44."""
    found = re.search(r"[\\/]MSVC[\\/](\d+(?:\.\d+)+)[\\/]", path)
    return tuple(int(n) for n in found.group(1).split(".")) if found else ()


def runtime_folder():
    given = os.environ.get("QYMCAD_VC_REDIST")
    if given:
        return given
    pattern = "C:/Program Files*/Microsoft Visual Studio/*/*/VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT"
    found = [p for p in glob.glob(pattern) if version_key(p)]
    if not found:
        sys.exit("the Visual C++ runtime was not found; QYMCAD_VC_REDIST names its folder")
    return max(found, key=version_key)


def copy_runtime():
    folder = runtime_folder()
    print(f">>> Visual C++ runtime from {folder}")
    for library in glob.glob(os.path.join(folder, "*.dll")):
        if os.path.basename(library).lower() not in NOT_CARRIED:
            shutil.copy2(library, STAGE)


def mesa_archive():
    """The archive of QYMCAD_MESA_ARCHIVE, or the one downloaded into dist/; refused unless its hash is the pinned one."""
    archive = os.environ.get("QYMCAD_MESA_ARCHIVE") or os.path.join("dist", MESA_ARCHIVE)
    if not os.path.isfile(archive):
        print(f">>> downloading {MESA_URL}")
        urllib.request.urlretrieve(MESA_URL, archive)
    with open(archive, "rb") as f:
        digest = hashlib.sha256(f.read()).hexdigest()
    if digest != MESA_SHA256:
        sys.exit(f"{archive}: SHA-256 {digest}, expected {MESA_SHA256}")
    return archive


def copy_processor_renderer():
    if not shutil.which("7z"):
        sys.exit("7z is needed to unpack Mesa")
    os.makedirs(RENDERER)
    members = [f"x64/{name}" for name in MESA_FILES]
    subprocess.check_call(["7z", "e", "-y", f"-o{RENDERER}", mesa_archive(), *members], stdout=subprocess.DEVNULL)
    missing = [name for name in MESA_FILES if not os.path.isfile(os.path.join(RENDERER, name))]
    if missing:
        sys.exit(f"the Mesa archive holds no {missing}")
    print(f">>> Mesa {MESA_VERSION} into {RENDERER}")


def write_notes():
    shutil.copy2("LICENSE", os.path.join(STAGE, "LICENSE.txt"))
    shutil.copy2("THIRD-PARTY-NOTICES.md", STAGE)
    for name, text in (("README.txt", NOTE_EN), ("ПРОЧТИ.txt", NOTE_RU)):
        with open(os.path.join(STAGE, name), "w", encoding="utf-8-sig", newline="\r\n") as f:
            f.write(text)


def pack(name):
    archive = os.path.join("dist", f"{name}-win7-x64.zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zipped:
        for folder, _, files in os.walk(STAGE):
            for file in files:
                path = os.path.join(folder, file)
                zipped.write(path, os.path.relpath(path, STAGE))
    print(f">>> DONE: {archive} ({os.path.getsize(archive) / 2**20:.1f} MB)")


def main():
    os.chdir(ROOT)
    if not os.path.isfile(BINARY):
        sys.exit(f"no {BINARY}: build for x86_64-win7-windows-msvc first (packaging/README.md)")
    shutil.rmtree(STAGE, ignore_errors=True)
    os.makedirs(STAGE)
    shutil.copy2(BINARY, STAGE)
    build_forwarder()
    copy_kernel()
    copy_runtime()
    copy_processor_renderer()
    write_notes()
    subprocess.check_call([sys.executable, os.path.join("packaging", "win", "check-win7.py"), STAGE])
    pack(package_name())


if __name__ == "__main__":
    main()
