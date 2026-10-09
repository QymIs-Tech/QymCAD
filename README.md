<div align="center">

<img src="assets/logo.png" width="112" alt="QymCAD">

# QymCAD

**A parametric associative CAD on a B-rep kernel**

Sketch → part → assembly. One program, one project file, no cloud and no subscription.

[cad.qymis.tech](https://cad.qymis.tech)

[![CI](https://github.com/QymIs-Tech/QymCAD/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/QymIs-Tech/QymCAD/actions/workflows/checks.yml)
[![Release](https://img.shields.io/github/v/release/QymIs-Tech/QymCAD?logo=github)](https://github.com/QymIs-Tech/QymCAD/releases)
[![License: AGPL 3.0](https://img.shields.io/badge/License-AGPL_3.0-blue.svg)](LICENSE)
<br>
[![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011%20x64-0078D6?logo=windows&logoColor=white)](https://github.com/QymIs-Tech/QymCAD/releases)
[![Linux](https://img.shields.io/badge/Linux-x86__64-FCC624?logo=linux&logoColor=black)](https://github.com/QymIs-Tech/QymCAD/releases)
[![macOS](https://img.shields.io/badge/macOS-Apple%20Silicon-000000?logo=apple&logoColor=white)](https://github.com/QymIs-Tech/QymCAD/releases)

**English** | [Русский](README.ru.md) | [Українська](README.uk.md) | [Қазақша](README.kk.md)

<img src="docs/screenshots/01-assembly.png" width="900" alt="A CNC machine assembled in QymCAD: components, joints and their degrees of freedom">

</div>

## What it is

QymCAD is a desktop CAD for mechanical parts and assemblies: housings, brackets, mechanisms, printed
and milled work.

A sketch becomes a solid by extruding, revolving or sweeping; the solid takes holes, fillets, a shell,
patterns. Parts come together into an assembly through joints — revolute, slider, rigid. The model is
parametric: change a dimension in any operation and everything below it in the timeline rebuilds, the
assembly included.

The geometry is exact and solid (B-rep), computed by the [OpenCASCADE](https://dev.opencascade.org/)
kernel — the same one FreeCAD runs on. Hence precise surfaces instead of meshes, correct fillets and
exchange through STEP with other CAD systems.

The interface is available in English, Kazakh, Ukrainian and Russian.

## Status

A development build. The program works and is fit for real parts, but it is updated daily.

- **The document format changes with no backward compatibility.** A file saved by an earlier version
  may not open. For such files there is the `convert_qcad.py` script (see below).
- **The CNC (CAM) module does not work.** The settings do carry a “Machining” tab (CAM) checkbox, but
  what is behind it is groundwork: part of the code came from an earlier version and is not
  maintained. The module returns for the stable alpha.
- **The macOS build carries no Apple signature.** macOS marks a downloaded application as quarantined
  and refuses to open it, saying it is damaged - it is not. The mark is cleared once, with one command,
  written out step by step in the notes inside the archive. Apple Silicon only; there is no Intel build.

## Features

**Sketching.** Lines, arcs, circles, ellipses, splines, polygons, slots, text. Constraints and
dimensions are solved together; dimensions are parametric (`w/2`, `sin(a)`). There is construction
geometry and the projection of a body's edges into the sketch.

**Parts.** Extrude, revolve, sweep, loft, fillet (including a variable radius set at vertices),
chamfer, shell, draft, holes (plain, counterbored, countersunk), thicken, patch, stitch, trim, split
of faces and of a body, face copy and face push, linear and circular patterns, mirror, booleans, and
the primitives: box, cylinder, sphere, cone, torus, prism. Threads are built from a real helical
profile with run-outs.

**Assemblies.** Parts and sub-assemblies, joints (rigid, revolute, slider, cylindrical, planar, ball,
pin-slot, parallel), limits and drives, degrees of freedom, an interference check. A sketch can be
placed on the face of a neighbouring part as an associative reference: change the neighbour and the
dependent part rebuilds.

**Exchange.** STEP (import and export), STL (import and export), DXF and SVG (import and export of
sketches). Either the whole project or a single part or sub-assembly picked in the tree can be written
out.

## Installation

The builds are in [Releases](https://github.com/QymIs-Tech/QymCAD/releases). No extra libraries are needed: OpenCASCADE and the
dependencies are inside the package.

<details>
<summary><b>Windows (10 / 11 x64)</b></summary>

- **Portable ZIP**: download `qymcad-win64.zip`, unpack anywhere and run `qymcad.exe`.
- **MSI Installer**: download and run `qymcad-*-x64.msi` for a standard system installation.
- **winget**:
  ```powershell
  winget install QymIsTech.QymCAD
  ```
- **Windows 7 / 8.x (Legacy build)**: download `qymcad-*-win7-x64.zip`, unpack anywhere and run `qymcad.exe`.

> [!NOTE]
> Official support is only for Windows 10+. A separate legacy build (`qymcad-*-win7-x64.zip`) is available for Windows 7 SP1 and 8.x: it works, but stability is not guaranteed.

</details>

<details>
<summary><b>Linux (x86_64)</b></summary>

Requires glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+, Arch Linux).

- **AppImage** (standalone):
  ```bash
  chmod +x qymcad-*.AppImage
  ./qymcad-*.AppImage
  ```
- **Arch Linux (AUR)**:
  ```bash
  yay -S qymcad-bin
  ```

</details>

<details>
<summary><b>macOS 12+ (Apple Silicon)</b></summary>

Download `qymcad-*-macos-arm64.zip` and unpack it. The build carries no Apple signature, so clear the quarantine attribute once before the first launch:

```bash
xattr -cr QymCAD.app
```

Then open `QymCAD.app` with a normal double-click.

*Note: Apple Silicon only (M1/M2/M3/M4); there is no Intel build.*

</details>

## Help

The built-in help opens with **F1**. The articles, with illustrations, live in [`docs/help`](docs/help).

## Project files

A document is saved as `.qcad`. The format changes directly, with no compatibility layer; documents
from earlier versions are brought forward by a separate script:

```bash
python3 convert_qcad.py part.qcad    # converts in place, keeping part.qcad.bak beside it
```

The script brings a document forward from any earlier version in one pass and is idempotent.

## Building from source

Linux — `just pkg-linux`, Docker required. Windows — MSVC with the kernel built from source. macOS — the
kernel from source as well, then `packaging/macos/bundle.sh`. The details are in
[`packaging/README.md`](packaging/README.md).

## Contributing

We welcome contributions of all kinds — code, documentation, translations, and bug reports. Check the [Contributing Guide](CONTRIBUTING.md) and pick a [good first issue](https://github.com/QymIs-Tech/QymCAD/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22) to get started.

## Contributors

<a href="https://github.com/QymIs-Tech/QymCAD/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=QymIs-Tech/QymCAD" alt="Contributors" />
</a>

## Star History

<a href="https://star-history.com/#QymIs-Tech/QymCAD&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=QymIs-Tech/QymCAD&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=QymIs-Tech/QymCAD&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=QymIs-Tech/QymCAD&type=Date" />
 </picture>
</a>

## Licence

The code is distributed under [AGPL-3.0-or-later](LICENSE): a fork and any derived build stay under
the same licence with open sources.

The OpenCASCADE kernel comes under LGPL-2.1 with an exception and is linked dynamically; the fonts
and the icons come under licences of their own. All of it is listed in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), and that file is placed beside the program in every
package.

---

**QymIs Tech** — [qymis.tech](https://qymis.tech). Author: Denis Kazachenkov.
