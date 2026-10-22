# Import and export

## How to open someone else's file

**File -> Import…** is one item for every format. The chooser shows every file the program reads;
the file's extension decides what it becomes: a solid, a mesh or a sketch. If the program does not
read the format, the status line names the file and lists what can be opened.

**Insert component** in an assembly opens the same chooser, but only with what can become a part:
STEP, IGES and STL. A flat drawing cannot become a part - open it from the File menu instead.

## What can be read

- **STEP** — exact geometry with faces and edges. The best way to get someone else's part: you can
  work with it as with your own, except for the history — it is not in the file. An assembly in the file comes
  in as a tree of sub-assemblies and parts in their places, with names and face colours.
- **IGES** — exact geometry too, an older format: many archives of machine-shop and design programs are
  kept in it. The file usually holds surfaces rather than solids; the ones that close are sewn into
  solids, and an open surface comes in as a shell. Surfaces the author gathered into a group are sewn into
  one part under the group's name, in the file's colours; a group of groups comes in as a subassembly. An
  IGES with no surface at all is a drawing: its curves go into a sketch, as with DXF.
- **STL** — a triangle mesh only. There is no shape as such in it, so precise operations are
  impossible until it is recognised into a solid (below); it is good for overall size and for printing.
- **OBJ** — a mesh too, the format of rendering and sculpting. Every object in the file becomes a part of
  its own; OBJ carries no unit, so the program asks what it is drawn in.
- **PLY** — the mesh of 3D scanners, as text or binary. The geometry is taken; the colours and normals of
  the points are not needed and are passed over.
- **glTF** (`.glb`, `.gltf`) — the format of browsers and rendering: a scene of nodes, each in its own
  place. A node that holds others becomes a subassembly, and a node that carries a mesh a part in its place, under
  its name. glTF keeps metres and takes Y as up - this is turned into millimetres and Z on the way in. A node's
  scale goes into its mesh: a part's place is a turn and a shift only.
- **3MF** — a mesh for printing, with the unit inside the file: a model drawn in inches arrives in
  millimetres at the right size. Every build item of a 3MF becomes a part in its place, under its name and in
  its colour; an object of named parts comes in as a group of those parts, to any depth.
- **AMF** — the predecessor of 3MF, a mesh for printing with the unit inside too; it is sometimes zipped,
  and that is read as well. A constellation becomes a subassembly, and its objects parts in their places.
- **DXF** and **SVG** — flat outlines, they land in a sketch.

## Units and scale

STL, OBJ and PLY carry no unit, and a file that names one can name it wrong. So once a file is read, a question
comes up at the bottom of the window, **Units and scale**: what the file is drawn in (for a file without units) and
by what factor to take it. The model in the view changes with the answer at once. **Enter** or **Import** keeps it
so as one step of undo; **Esc** cancels the import altogether. A file with units is asked about only when the model
comes out under 1 mm or over 10 m - or always, when the settings say "Ask for units and scale on every import".

To change the scale of a file already in the document, double-click its node in the tree - **Import: file name**
or **Mesh from a file: file name** - choose **Edit** in the node's menu, or press **Edit** in its properties. The
node lies in the part the file came into: if it does not show, step into the part with a double click first. The
same window opens at the factor the file stands at now, and everything that came from the file changes together.
**Enter** or **Apply** keeps the new scale as one step of undo; **Esc** leaves it as it was. If the status line
says the bodies are still loading from the file, try again when loading is done.

## What can be written

Everything goes through **File -> Export project**: a submenu of every format, the exact ones on top, the
meshes below the line. On a component in the tree the same submenu is called **Export** and writes only the
visible bodies of that component.

- **Export to STEP** — exact geometry as the assembly tree, with names and colours. This is what goes to a
  customer and to a machine.
- **Export to IGES** — exact geometry written as faces. For programs that do not read STEP - most
  often older machine-shop ones.
- **Export to STL** — triangles, for printing. Here the document's **geometry accuracy** matters: it
  decides how finely the surface is divided. It travels with the file, so the same project gives the
  same STL to two different people.
- **Export to OBJ** — the same mesh at the same choice of quality, one object per body: for rendering
  programs.
- **Export to PLY** — the same mesh as a binary file with exact coordinates: for programs that work with
  scans.
- **Export to glTF** — the same mesh as one `.glb` file, one node per body, in metres with Y up, as
  browsers and rendering programs expect.
- **Export to 3MF** — the same mesh for printing, with the millimetres stated in the file itself: at the
  printer it will not arrive in inches.
- **Export to AMF** — the same mesh for printing programs that expect AMF, in millimetres.

## Things to remember

- A mesh is turned into a solid by the Part tool [Recognise](part/26-recognise): exact surfaces (a hole becomes
  a cylinder) or a polyhedron as it is. The solid can be cut, drilled and sketched on. A large mesh takes long
  to recognise - tens of seconds for a hundred thousand triangles.

- A mesh lands where its file puts it - as the exact formats do. If a model comes in far from the origin,
  move it like any other body.

- Only **visible, not consumed** bodies are exported: intermediate states of a chain are not.
- An imported body lives in the timeline as its own feature: you can move, cut and drill it, but you
  cannot “go back to its sketch” — there never was one.
- A sketch goes out as a flat drawing: the right button on the sketch in the tree -> **Export to SVG…**
  or **Export to DXF…**. An ellipse and a spline go out as the curves they are — as ELLIPSE and SPLINE in DXF, as
  elliptical arcs and Bezier curves in SVG — and come back from DXF as an ellipse and a spline.
- To simplify an import there is the **Remove face** command: it takes away a hole or a pad that
  should not be in someone else's model.
