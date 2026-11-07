#!/usr/bin/env python3
"""DOES THIS FOLDER LOAD ON WINDOWS 7? Answered from the files, without Windows.

    check-win7.py <folder>               refuse a package a Windows 7 loader would refuse
    check-win7.py --refresh <System32>   rewrite win7-exports.txt from the folder of a real system

The loader stops a program before its first line when one imported name is missing, and the message names
one function of one library. A run on anything but Windows 7 proves nothing here: a system that has the
function loads the program whatever version it reports.

Measured on the Windows zip of 14.10.2026: msvcp140.dll 14.51 imports CopyFile2, CreateFile2 and
GetSystemTimePreciseAsFileTime from kernel32.dll, none of which Windows 7 has; 14.36 to 14.44 import none of them.

Three things are checked, starting at qymcad.exe and every TK*.dll (OCCT opens some of its own by name):
the imported libraries, the imported names, plain and delayed, followed through forwarded exports, and the
system version written in each file's header. What a program asks for at run time through GetProcAddress is
not in any table and only a run shows it.

win7-exports.txt comes from Windows 7 x64 SP1 build 7601.24576 with its updates, the Universal CRT among them, and
holds only names: the libraries themselves cannot be distributed.
"""
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REFERENCE = os.path.join(HERE, "win7-exports.txt")
# Windows 7 is 6.1. A file asking for more is refused by the loader as "not a valid Win32 application".
NEWEST_SYSTEM = (6, 1)
DIR_EXPORT, DIR_IMPORT, DIR_DELAY = 0, 1, 13
# Refreshed along with every api-ms-win-*.dll of the system; a library not listed reads as absent, which fails towards
# a person looking at it.
SYSTEM_LIBRARIES = """advapi32 bcrypt comctl32 comdlg32 crypt32 d3d11 d3d9 dbghelp dwmapi dxgi gdi32 imm32 iphlpapi
kernel32 kernelbase netapi32 ntdll ole32 oleacc oleaut32 opengl32 powrprof propsys psapi rpcrt4 secur32 setupapi
shell32 shlwapi ucrtbase uiautomationcore user32 userenv uxtheme version winmm ws2_32 wsock32""".split()


class Pe:
    """The tables of one PE file."""

    def __init__(self, path):
        with open(path, "rb") as f:
            self.data = f.read()
        head = struct.unpack_from("<I", self.data, 0x3C)[0]
        sections, = struct.unpack_from("<H", self.data, head + 6)
        optional_size, = struct.unpack_from("<H", self.data, head + 20)
        optional = head + 24
        self.wide = struct.unpack_from("<H", self.data, optional)[0] == 0x20B
        self.system = struct.unpack_from("<HH", self.data, optional + 40)
        self.subsystem = struct.unpack_from("<HH", self.data, optional + 48)
        directories = optional + (112 if self.wide else 96)
        self.directories = [struct.unpack_from("<II", self.data, directories + 8 * i) for i in range(16)]
        self.sections = []
        for i in range(sections):
            virtual_size, address, raw_size, raw = struct.unpack_from("<IIII", self.data, optional + optional_size + 40 * i + 8)
            self.sections.append((address, max(virtual_size, raw_size), raw))

    def at(self, rva):
        for address, size, raw in self.sections:
            if address <= rva < address + size:
                return rva - address + raw
        raise ValueError(f"address {rva:#x} lies in no section")

    def text(self, rva):
        start = self.at(rva)
        return self.data[start : self.data.index(0, start)].decode("latin-1")

    def exports(self):
        """name -> forward target or None, and ordinal -> the same."""
        table, size = self.directories[DIR_EXPORT]
        names, ordinals = {}, {}
        if not table:
            return names, ordinals
        base, functions, named, function_table, name_table, ordinal_table = struct.unpack_from("<IIIIII", self.data, self.at(table) + 16)
        name_of = {}
        for i in range(named):
            index, = struct.unpack_from("<H", self.data, self.at(ordinal_table) + 2 * i)
            name_of[index] = self.text(struct.unpack_from("<I", self.data, self.at(name_table) + 4 * i)[0])
        for i in range(functions):
            target, = struct.unpack_from("<I", self.data, self.at(function_table) + 4 * i)
            if not target:
                continue
            forward = self.text(target) if table <= target < table + size else None
            ordinals[base + i] = forward
            if i in name_of:
                names[name_of[i]] = forward
        return names, ordinals

    def imports(self, directory):
        """[(library, [name or ordinal])] of the plain or the delayed table."""
        out = []
        table, _ = self.directories[directory]
        if not table:
            return out
        entry = self.at(table)
        while True:
            if directory == DIR_IMPORT:
                lookup, _, _, name, first_thunk = struct.unpack_from("<IIIII", self.data, entry)
                lookup = lookup or first_thunk
                entry_size = 20
            else:
                _, name, _, _, lookup = struct.unpack_from("<IIIII", self.data, entry)
                entry_size = 32
            if not name:
                return out
            out.append((self.text(name), self.names_at(lookup)))
            entry += entry_size

    def names_at(self, lookup):
        form, width, top = ("<Q", 8, 1 << 63) if self.wide else ("<I", 4, 1 << 31)
        names = []
        place = self.at(lookup)
        while True:
            value, = struct.unpack_from(form, self.data, place)
            if not value:
                return names
            names.append(value & 0xFFFF if value & top else self.text((value & 0x7FFFFFFF) + 2))
            place += width


class Exports:
    """What one library offers: named exports and ordinals, each with where it forwards to."""

    def __init__(self, names, ordinals):
        self.names = names
        self.ordinals = ordinals

    def forward_of(self, wanted):
        """False when the library lacks the name; otherwise its forward target or None."""
        table = self.ordinals if isinstance(wanted, int) else self.names
        return table[wanted] if wanted in table else False


def read_reference():
    libraries = {}
    current = None
    with open(REFERENCE, encoding="ascii") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("["):
                current = Exports({}, {})
                libraries[line.strip("[]")] = current
                continue
            name, ordinal = line.split(" ")
            current.ordinals[int(ordinal)] = None
            if name != "-":
                current.names[name] = None
    return libraries


def library_file(name):
    name = name.lower()
    return name if name.endswith((".dll", ".exe")) else name + ".dll"


def lacks(libraries, library, wanted, depth=0):
    """Why `library!wanted` cannot be bound, or None when it can. `libraries` maps a lower-case file name to Exports."""
    exports = libraries.get(library_file(library))
    if exports is None:
        return f"{library} is not there"
    forward = exports.forward_of(wanted)
    if forward is False:
        return f"{library} has no {wanted}"
    if not forward or depth > 8:
        return None
    target, name = forward.split(".", 1)
    by_ordinal = name.startswith("#") and name[1:].isdigit()
    return lacks(libraries, target, int(name[1:]) if by_ordinal else name, depth + 1)


def check(folder):
    """Every reason the folder would not load, as lines of text, and how many of its files a loader would open."""
    files = {f.lower(): os.path.join(folder, f) for f in os.listdir(folder) if f.lower().endswith((".dll", ".exe"))}
    if "qymcad.exe" not in files:
        return [f"{folder} holds no qymcad.exe"], 0
    libraries = read_reference()
    opened = {}
    for name, path in files.items():
        pe = Pe(path)
        opened[name] = pe
        libraries[name] = Exports(*pe.exports())
    pending = ["qymcad.exe"] + [f for f in files if f.startswith("tk")]
    seen = set()
    faults = set()
    while pending:
        name = pending.pop()
        if name in seen or name not in opened:
            continue
        seen.add(name)
        pe = opened[name]
        for version in (pe.system, pe.subsystem):
            if version > NEWEST_SYSTEM:
                faults.add(f"{name}: asks for system {version[0]}.{version[1]}, Windows 7 is 6.1")
        for directory in (DIR_IMPORT, DIR_DELAY):
            for library, wanted in pe.imports(directory):
                pending.append(library_file(library))
                for one in wanted:
                    reason = lacks(libraries, library, one)
                    if reason:
                        faults.add(f"{name}: {reason}")
    return sorted(faults), len(seen)


def refresh(system32):
    present = {f.lower(): os.path.join(system32, f) for f in os.listdir(system32) if f.lower().endswith(".dll")}
    wanted = sorted(n + ".dll" for n in SYSTEM_LIBRARIES if n + ".dll" in present)
    wanted += sorted(n for n in present if n.startswith("api-ms-win-"))
    every = {}

    def exports_of(name):
        if name not in every and name in present:
            try:
                every[name] = Exports(*Pe(present[name]).exports())
            except (ValueError, struct.error):
                every[name] = Exports({}, {})
        return every.get(name)

    # A forward can lead anywhere in the system, so the whole folder is at hand while the listed libraries are written.
    for name in present:
        exports_of(name)
    lines = ["# Exports of Windows 7 x64 SP1. Written by `check-win7.py --refresh <System32>`, not by hand.", "# name ordinal, or - ordinal for an export without a name."]
    for name in wanted:
        exports = every[name]
        lines.append(f"[{name}]")
        ordinal_of = names_with_ordinals(Pe(present[name]))
        for export in sorted(exports.names):
            if lacks(every, name, export) is None:
                lines.append(f"{export} {ordinal_of[export]}")
        for ordinal in sorted(set(exports.ordinals) - set(ordinal_of.values())):
            if lacks(every, name, ordinal) is None:
                lines.append(f"- {ordinal}")
    with open(REFERENCE, "w", encoding="ascii", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{len(wanted)} libraries -> {REFERENCE}")


def names_with_ordinals(pe):
    table, _ = pe.directories[DIR_EXPORT]
    if not table:
        return {}
    base, _, named, _, name_table, ordinal_table = struct.unpack_from("<IIIIII", pe.data, pe.at(table) + 16)
    out = {}
    for i in range(named):
        index, = struct.unpack_from("<H", pe.data, pe.at(ordinal_table) + 2 * i)
        out[pe.text(struct.unpack_from("<I", pe.data, pe.at(name_table) + 4 * i)[0])] = base + index
    return out


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--refresh":
        refresh(sys.argv[2])
        return 0
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    faults, opened = check(sys.argv[1])
    if faults:
        print(f"!!! the package does not load on Windows 7 ({len(faults)}):")
        for fault in faults:
            print(f"!!!   {fault}")
        return 1
    print(f">>> loads on Windows 7: {opened} files, every import found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
