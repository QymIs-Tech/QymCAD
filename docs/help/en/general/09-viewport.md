# The viewport: looking and selecting

## How to look

![The viewport: the view cube in the corner, axes and grid, the part in isometric view.](img/viewport.png)

The **view cube** in the top right corner: click a face for the front, top or side view; a corner for
an isometric one; an edge for a 45° view. The “home” button returns the isometric view. The move is
smooth — a jump is disorienting, especially in an assembly.

The size of the cube is set in the settings: a small one is unreadable on 4K, a large one gets in the
way on a small screen.

## Moving the view with the mouse

In the 3D viewport:

* **Drag** turns the model around what you are looking at.
* **Shift** and drag moves the view sideways and up or down.
* Both of those are the factory layout, **QymCAD**. In the settings, under **Mouse navigation**, you can
  pick the layout of the CAD you are used to instead - CAD, Blender, Gesture, Maya, OpenCascade,
  OpenInventor, OpenSCAD, Revit, TinkerCAD or Touchpad. Each is the one that CAD uses, so the habit you
  came with keeps working; the line under each name says what its buttons do.
* **The wheel** zooms in and out, towards wherever the cursor is: the point under it stays put. In the
  settings, under **Zooming with the wheel**, you can choose "from the middle of the view" instead - then
  the middle of the viewport is what holds still, wherever you aim.
* While a command is open the view zooms **from the middle of the part**: the command's fields stand at the
  geometry, and zooming towards the cursor drags the part, and them with it, out from under your hand. The
  line **While a command is open** in the settings turns that off.
* **A click** selects a body, a face, an edge or an outline; a click on empty space clears the selection.
* **The right button** on what is under the cursor offers "expand the selection" while a command that
  takes such a description is open. A right-button DRAG is still a turn, so the menu does not get in the
  way of looking around.

In a sketch the view is flat and the buttons differ:

* **The middle button** and drag moves the sheet. The left button is busy there: it draws and it grabs.
* **The wheel** zooms in and out, by the same rule as in the 3D view.
* **Drag with the left button** on a point, a line or a dimension moves it.
* **Drag with the left button** on empty space draws a selection box.
* **Ctrl** and drag draws a selection box even when the drag begins on top of geometry, so you can box in
  what lies among other things.

If the model has gone out of sight, the "home" button on the view cube brings the isometric view back.

## Section view

Click a plane or a face and half the model is hidden so you can look inside. This is only a **view**:
the model is not cut and nothing goes into the export. The plane's shift and tilt are in the top bar.

## What can be selected

A body, a face, an edge, a sketch outline, a component. Pointing precision is set in the settings —
from “precise” to “coarse”: a touch screen and a 4K display need different hit radii.

## Ghosts

A part outside the current context is shown semi-transparent — so that you can refer to it without it
getting in the way. The transparency is set in the settings: some find it distracting, others cannot
see it at all.
