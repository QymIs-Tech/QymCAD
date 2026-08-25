# The application icons

Generated from `assets/logo.png` (4168x4168 RGBA). To regenerate them: `assets/icons/generate.sh`.

## Windows (`.exe`)
- `windows/qymcad.ico` - multi-size (16, 24, 32, 48, 64, 128, 256).
- Embedded into the exe (through `winres`/`embed-resource` in `build.rs`, pointing at this `.ico`).

## Linux (AppImage)
- `linux/<N>x<N>.png` - the hicolor set (16 to 512) for `usr/share/icons/hicolor/<N>x<N>/apps/qymcad.png`.

## macOS (`.icns`)
- `macos/qymcad.icns` - laid in for the future; there is no macOS build yet.
- The PNG chunks: icp4/5/6 (16/32/64), ic07-ic10 (128/256/512/1024), ic11-ic14 (retina @2x).
