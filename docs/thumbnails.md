# Project Preview Thumbnails & Shell Integration

QymCAD project files (`.qcad`) are standard ZIP bundles that optionally embed a raster preview image (`thumb.png`) at the archive root. This document explains the thumbnail specification and how desktop shell thumbnail providers (Linux, Windows, macOS) and third-party tools can extract and display it.

## 1. Specification

* **Archive path**: `thumb.png` (directly at the ZIP archive root).
* **Image format**: Standard PNG (Portable Network Graphics), RGBA8.
* **Dimensions**: 256 x 256 pixels.
* **Background**: Fully transparent (alpha = 0) so the preview renders cleanly against any file manager backdrop (light mode, dark mode, or desktop wallpaper).
* **Geometry**: Orthographic projection from a fixed isometric view (`Cam3::default()`), scaled to fit 95% of the frame (`FitScreen2D(0.95)`), centered at the projected center of the 3D meshes.
* **Badge**: A 64 x 64 application icon stamped with full opacity in the bottom-right corner (8 px margin).
* **Storage**: Stored in the ZIP archive using `CompressionMethod::Stored` (Method 0, no compression). Because PNG streams are already compressed (DEFLATE), storing without recompression speeds up saves and allows direct file seek/read by external thumbnailers.

## 2. Presence Rules

* **Saved projects with 3D bodies**: `thumb.png` is generated and saved upon manual Save (`Ctrl+S`) or Save As (`Ctrl+Shift+S`).
* **Documents without 3D bodies**: When a project contains no 3D meshes (such as an empty document or only 2D sketch contours), `thumb.png` is **omitted**. When absent, the host operating system should display the standard QymCAD file type icon.
* **Deleted bodies**: If a model previously containing 3D geometry is edited such that all bodies are deleted, subsequent saves remove `thumb.png` from the bundle.
* **Autosave**: Periodic autosave (`.autosave.qcad`) deliberately skips thumbnail generation to avoid background CPU and IO overhead during active user editing.

## 3. Reading the Thumbnail

### In Rust (`qymcad-io`)

`qymcad-io` provides two dedicated helper functions that read the preview without parsing `document.ron` or loading mesh geometry:

```rust
// From a file path:
let thumb: Option<Vec<u8>> = qymcad_io::load_project_thumb("path/to/model.qcad");

// From a byte slice in memory:
let thumb: Option<Vec<u8>> = qymcad_io::load_project_thumb_bytes(&bytes);
```

### In Python

```python
import zipfile

def get_qcad_thumbnail(path: str) -> bytes | None:
    try:
        with zipfile.ZipFile(path, "r") as z:
            return z.read("thumb.png")
    except (KeyError, zipfile.BadZipFile):
        return None
```

### In Shell / Command Line

```bash
# Extract thumbnail directly to stdout or file
unzip -p model.qcad thumb.png > thumbnail.png
```

## 4. Operating System Shell Integration

### Linux (FreeDesktop Thumbnailer)

Desktop environments implementing the FreeDesktop Thumbnail Management Standard (GNOME, KDE, XFCE, Cinnamon) discover thumbnail providers through `.thumbnailer` files in `/usr/share/thumbnailers/` or `~/.local/share/thumbnailers/`.

Create `/usr/share/thumbnailers/qymcad.thumbnailer`:

```ini
[Thumbnailer Entry]
TryExec=unzip
Exec=sh -c "unzip -p %i thumb.png > %o"
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
   * Implement `IThumbnailProvider::GetThumbnail(UINT cx, HBITMAP *phbmp, WFD_CONTAINER_TYPE *pdwAlpha)`.
   * Open the stream as a ZIP archive, locate `thumb.png`.
   * Decode the PNG stream into an `HBITMAP` (32-bit PARGB32) using Windows Imaging Component (`IWICImagingFactory`).
   * Set `*pdwAlpha = WFDCT_CAN_BE_TRANSPARENT` so Windows Explorer honors the alpha channel.
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
