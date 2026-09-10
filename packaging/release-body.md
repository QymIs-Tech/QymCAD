<!-- The page for the NEXT release is written here, before the tag is made: what was added, what was
     fixed, what changed. Short lines, one thought each. The changelog link below is generated. -->

## What changed

**Added**

* **A check for new versions.** The program says when a newer one is out, and opens the release page.
  How often to ask is in **Settings -> General**; "never" is one of the choices.
* **A Windows installer (MSI)** beside the portable zip: the program appears in "Apps and features",
  updates in place, and leaves a shortcut on the desktop and in the Start menu.

**Fixed**

* The AppImage opened no window on a Wayland desktop.
* Settings and crash reports were kept in the wrong folder.
* "The last run ended in an error" came back at every start until every report had been closed one by one.
* A dimension could not be measured to the X or Y axis — a crooked one was placed instead.
* A circle drawn at the origin could not be moved away from it.
* A circle drawn by hand showed a diameter that was not a dimension: the sketch stayed under-defined
  until that number was edited by hand.
* The move tool let a shape held by its constraints appear to move, and it jumped back at the next edit.
* The command bar ran its messages together on one line.

**Changed**

* The About window names the release, not the number in the manifest.

## Known limits

The document format changes with no backward compatibility; `convert_qcad.py` brings older files forward.
The CNC (CAM) module is groundwork and does not work yet. There is no Intel macOS build.
