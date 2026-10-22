# Project Preview Thumbnails & Shell Integration

QymCAD project files (`.qcad`) are standard ZIP bundles that optionally embed a raster preview image (`thumb.png`) at the archive root. This document explains the thumbnail specification and how desktop shell thumbnail providers (Linux, Windows, macOS) and third-party tools can extract and display it.

## 1. Specification

* **Archive path**: `thumb.png` (directly at the ZIP archive root).
* **Image format**: Standard PNG (Portable Network Graphics), RGBA8.
* **Dimensions**: 256 x 256 pixels.
* **Background**: Fully transparent (alpha = 0) so the preview renders cleanly against any file manager backdrop (light mode, dark mode, or desktop wallpaper).
* **Geometry**: Orthographic projection from a fixed isometric view (`Cam3::default()`), scaled to fit 95% of the frame (`FitScreen2D(0.95)`), centered at the projected center of the 3D meshes.
* **Colours**: The default (dark) colour scheme, whatever scheme the person saving uses, so that one file has one preview.
* **Bodies**: What the root assembly shows: a body consumed by an operation (the tool of a cut or a boolean), a body or component whose visibility tick is off, and the body of a failed operation are not drawn.
* **Badge**: A 64 x 64 application icon stamped with full opacity in the bottom-right corner (8 px margin).
* **Storage**: Stored in the ZIP archive using `CompressionMethod::Stored` (Method 0, no compression). Because PNG streams are already compressed (DEFLATE), storing without recompression speeds up saves and allows direct file seek/read by external thumbnailers.

## 2. Presence Rules

* **Saved projects with visible bodies**: `thumb.png` is generated and saved upon manual Save (`Ctrl+S`) or Save As (`Ctrl+Shift+S`).
* **Nothing to draw**: When no body is shown (an empty document, sketches only, every body hidden), `thumb.png` is **omitted**. When absent, the host operating system should display the standard QymCAD file type icon.
* **Not carried over**: A save writes the preview of the document being saved or none; the preview of the previous version of the file is never kept. If the bodies are deleted and sketches remain, the next save removes `thumb.png`.
* **Refused save**: A document with no content at all is not written over a file that has content (the save is refused). The file, and its preview, stay as they were.
* **Autosave**: Periodic autosave (`.autosave.qcad`) skips the preview: it is not the file a person opens from a file manager.

## 3. Reading the Thumbnail

### In Rust (`qymcad-io`)

`qymcad-io` provides two dedicated helper functions that read the preview without parsing `document.ron` or loading mesh geometry. Both refuse an entry larger than `MAX_THUMB_SIZE` (2 MiB) or one that does not start with the PNG signature, so they are safe to call on arbitrary files:

```rust
// From a file path:
let thumb: Option<Vec<u8>> = qymcad_io::load_project_thumb("path/to/model.qcad");

// From a byte slice in memory:
let thumb: Option<Vec<u8>> = qymcad_io::load_project_thumb_bytes(&bytes);
```

### In Python

A reader of arbitrary files bounds the entry itself, as `qymcad-io` does:

```python
import zipfile

MAX_THUMB_SIZE = 2 * 1024 * 1024
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"

def get_qcad_thumbnail(path: str) -> bytes | None:
    try:
        with zipfile.ZipFile(path, "r") as z:
            if z.getinfo("thumb.png").file_size > MAX_THUMB_SIZE:
                return None
            with z.open("thumb.png") as f:
                data = f.read(MAX_THUMB_SIZE + 1)
    except (KeyError, zipfile.BadZipFile, OSError):
        return None
    if len(data) > MAX_THUMB_SIZE or not data.startswith(PNG_MAGIC):
        return None
    return data
```

### In Shell / Command Line

```bash
# Extract thumbnail directly to stdout or file
unzip -p model.qcad thumb.png > thumbnail.png
```

## 4. Operating System Shell Integration

The packages of QymCAD do not install a thumbnailer yet: the recipes below are what such an integration needs. The Flatpak desktop entry (`packaging/flatpak/tech.qymis.cad.desktop`) already declares the MIME type `application/x-qymcad`; no package ships the MIME definition for `*.qcad`.

### Linux (FreeDesktop Thumbnailer)

Desktop environments implementing the FreeDesktop Thumbnail Management Standard (GNOME, KDE, XFCE, Cinnamon) discover thumbnail providers through `.thumbnailer` files in `/usr/share/thumbnailers/` or `~/.local/share/thumbnailers/`.

A thumbnailer is run on any file the file manager comes across, so it bounds the entry as the readers above do: a damaged or forged `.qcad` may declare a `thumb.png` of any size. Create `/usr/bin/qymcad-thumbnailer` (executable):

```sh
#!/bin/sh
# qymcad-thumbnailer INPUT OUTPUT: thumb.png of a .qcad, at most 2 MiB and a PNG, or no output at all
unzip -p "$1" thumb.png | head -c 2097153 > "$2"
n=$(wc -c < "$2")
magic=$(head -c 8 "$2" | od -An -tx1 | tr -d ' \n')
[ "$n" -gt 0 ] && [ "$n" -le 2097152 ] && [ "$magic" = 89504e470d0a1a0a ] && exit 0
rm -f "$2"
exit 1
```

and `/usr/share/thumbnailers/qymcad.thumbnailer`:

```ini
[Thumbnailer Entry]
TryExec=qymcad-thumbnailer
Exec=qymcad-thumbnailer %i %o
MimeType=application/x-qymcad;
```

Ensure the MIME type `application/x-qymcad` is registered for the `.qcad` extension in `/usr/share/mime/packages/qymcad.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-qymcad">
    <comment>QymCAD Project</comment>
    <glob pattern="*.qcad"/>
    <icon name="qymcad"/>
  </mime-type>
</mime-info>
```

Update the system databases after installing:

```bash
update-mime-database /usr/share/mime
```

### Windows Explorer (Shell Thumbnail Provider)

Windows Explorer displays thumbnails via an in-process COM server implementing `IThumbnailProvider` and `IInitializeWithStream` (or `IInitializeWithFile`).

1. **COM Implementation**:
   * Implement `IThumbnailProvider::GetThumbnail(UINT cx, HBITMAP *phbmp, WTS_ALPHATYPE *pdwAlpha)`.
   * Open the stream as a ZIP archive, locate `thumb.png`.
   * Decode the PNG stream into an `HBITMAP` (32-bit PARGB32) using Windows Imaging Component (`IWICImagingFactory`).
   * Set `*pdwAlpha = WTSAT_ARGB` so Windows Explorer honors the alpha channel.
2. **Registry Registration**:
   Register the CLSID under `.qcad`:

```reg
Windows Registry Editor Version 5.00

[HKEY_CLASSES_ROOT\.qcad\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}]
@="{YOUR-THUMBNAIL-PROVIDER-GUID}"
```

### macOS (QuickLook Thumbnail Provider)

macOS Finder and QuickLook query thumbnail app extensions bundled inside the application:

1. Add a **Quick Look Thumbnail Extension** (`QLThumbnailProvider`) to `QymCAD.app/Contents/PlugIns/`.
2. In `provideThumbnail(for:request:)`:
   * Unzip `thumb.png` from the file at `request.fileURL`.
   * If found, draw the `CGImage` into `handler.provideThumbnail(forImage: ...)` or use `QLThumbnailReply(contextSize: ...)`:

```swift
class ThumbnailProvider: QLThumbnailProvider {
    override func provideThumbnail(for request: QLFileThumbnailRequest, _ handler: @escaping (QLThumbnailReply?, Error?) -> Void) {
        // Read thumb.png from request.fileURL ZIP archive
        if let pngData = extractThumbPng(from: request.fileURL),
           let image = NSImage(data: pngData) {
            let reply = QLThumbnailReply(contextSize: request.maximumSize) { () -> Bool in
                image.draw(in: CGRect(origin: .zero, size: request.maximumSize))
                return true
            }
            handler(reply, nil)
        } else {
            handler(nil, nil)
        }
    }
}
```
3. Register the document UTType in `Info.plist` with `public.filename-extension: ["qcad"]`.
