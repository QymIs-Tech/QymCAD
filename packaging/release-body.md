<!-- The page for the NEXT release is written here, before the tag is made: what was added, what was
     fixed, what changed. Short lines, one thought each. The changelog link below is generated. -->

## What changed

**Added**

* **Builds for macOS** — Apple Silicon. The app carries no Apple signature, so the first launch needs one
  command; `README.txt` inside the archive walks through it, in both languages.
* The window carries its own name and its own icon.

**Fixed**

* A machine with no graphics driver showed a window for a moment and closed without a word. A failed start
  is now reported, and an adapter backed by the processor is used rather than refused.
* The file chooser held the window: nothing was drawn while it stood open, and the desktop called the
  program "not responding".
* The interface behind an open system dialog could still be clicked, so an answer could land in a document
  other than the one it was asked from.
* Two saves of an untouched document produced different files — poison for a format kept in version control.
* The rebuild card cut off its own text.

**Changed**

* eframe and egui 0.29 -> 0.35, wgpu 22 -> 30, ron 0.8 -> 0.12, zip 2 -> 8, and the rest with them.

## Known limits

The document format changes with no backward compatibility; `convert_qcad.py` brings older files forward.
The CNC (CAM) module is groundwork and does not work yet. There is no Intel macOS build.
