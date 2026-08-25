# Third-party notices

QymCAD itself is distributed under [AGPL-3.0-or-later](LICENSE). It is built on other people's work,
and this file says whose, on what terms, and where the sources are. It accompanies the binary
packages (the Windows zip and the Linux AppImage) as well as the repository.

## Open CASCADE Technology

The geometric kernel — exact B-rep, the boolean operations, the fillets, STEP.

- Version 7.8.1
- Copyright (c) Open CASCADE SAS
- Licence: **LGPL-2.1 with an additional exception** (the "Open CASCADE Exception"), quoted at
  <https://dev.opencascade.org/resources/licensing>
- Sources: <https://github.com/Open-Cascade-SAS/OCCT>

QymCAD links OCCT **dynamically**: the `TK*` shared libraries lie next to the executable in the
Windows package and inside the AppImage, and are not compiled into it. LGPL-2.1 §6 is thereby
satisfied — the library can be replaced with another build of the same version without touching
QymCAD.

## Liberation Sans

The bold face used for the ViewCube, the axes and the dimension labels.

- Copyright (c) 2012 Red Hat, Inc., with Reserved Font Name Liberation; digitized data
  copyright (c) 2010 Google Corporation
- Licence: **SIL Open Font License 1.1** — the full text ships alongside the font in
  [`assets/fonts/LiberationSans-LICENSE.txt`](assets/fonts/LiberationSans-LICENSE.txt)

## Phosphor Icons

The interface icons, through the `egui-phosphor` crate.

- Copyright (c) 2023 Phosphor Icons
- Licence: **MIT**
- Sources: <https://github.com/phosphor-icons/core>

## The Rust crates

The rest of the dependencies — egui and eframe for the interface, wgpu for the viewport, serde and
ron for the document, and the several hundred crates beneath them — come under permissive licences:
MIT, Apache-2.0, BSD, Zlib, BSL-1.0, Unicode-3.0. None of them conflicts with AGPL-3.0.

The exact list for a given build is produced from the lock file:

```bash
cargo install cargo-license
cargo license --avoid-build-deps
```
