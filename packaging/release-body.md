<!-- The page for the NEXT release is written here, before the tag is made: what was added, what was
     fixed, what changed. Short lines, one thought each. The changelog link below is generated. -->

## What changed

**Added**

* **Mesh to solid.** The **Recognise** tool turns an imported mesh into a body: exact surfaces (planes,
  cylinders, cones, spheres, tori, thread and spring faces, smooth free-form walls) or the mesh as it is,
  a polyhedron.
* **Simplify a mesh.** A heavy mesh is made lighter within a set deviation before it becomes a body; the tool
  warns when a mesh is heavy and suggests the value.
* **New file formats.** IGES, OBJ, PLY, glTF, 3MF and AMF, read and written. One **Import…** command for all of
  them, one **Export** submenu.
* **Imports keep their structure.** STEP, IGES, glTF, 3MF, AMF and STL come in as an assembly tree with part
  names, positions and the author's colours, down to single faces; repeated parts come in as one part used
  several times.
* **Units and scale on import**, changeable later from the imported file's node.
* **Export keeps the structure** too: STEP, glTF and 3MF are written as an assembly tree with names, positions
  and colours.
* **Assembly:** a clone of a part (the same part again, not a copy), a component pattern (linear in up to
  three directions, circular about a picked axis), a mirrored copy of a part.
* **Part:** the pieces a cut leaves are bodies of one part, and **Make Part** moves one into a part of its own;
  **Offset Surface**; a face replaced by a sheet stretches its neighbours to it.
* **Sketch:** text in any installed font, with a font window (search and preview); dimension text placement
  and style in **Settings**.
* **View:** mouse layouts that follow the programs they are named after (3D and sketch), a layout for a pen,
  box selection in 3D, highlight under the cursor, "look at point".
* **Faster rebuilds:** a rebuild computes independent nodes on several cores (how many is a setting); the scene
  draws large models with less memory.
* **First start** opens a sample; afterwards the last project opens.
* **A macOS build for Intel** beside the one for Apple Silicon: `macos-x86_64.zip` and `macos-arm64.zip`.

**Fixed**

* More than 150 faults found by working through the program by hand: every tool takes what was selected
  before it, lets a pick go on a second click, and says in words why a value is refused instead of clamping it.
* Previews for revolve, fillet, chamfer, hole and draft show what Enter will build.
* One action is one undo step, named after the tool.
* A node that fails keeps its reason, also after saving and opening again.
* **Cancel** stops a long rebuild at once and takes back the change it was computing.
* A chamfer on a body made from a mesh no longer closes the program.
* Opening STEP and IGES at the same time no longer closes the program; a large IGES opens in about a minute.

**Changed**

* All settings and data are kept in one folder.
* **File -> New Project** starts an empty assembly.

## Known limits

The document format changes with no backward compatibility; `convert_qcad.py` brings older files forward.
Recognising a mesh: smooth rounded walls come out in several faces with visible seams between them.
The CNC (CAM) module is groundwork and does not work yet.
