# Settings

The settings window is split into sections; above them is a search that finds a setting by its label.

![The settings window: sections on the left, search above them, “Reset this section” at the bottom.](img/settings.png)

- **General** — language of the program and of the help, how to open the help, the start screen and
  the last project, units on import, autosave, undo depth, how many processors to give to computing, update checks, recent
  files, the settings profile.
- **Appearance** — colour scheme, interface scale.
- **Viewport** — engine, projection, shading, view cube, mouse navigation, wheel zoom, pointing
  precision, ghost transparency, field of view, antialiasing.
- **Sketch** — snapping, grid step, rotation step, auto constraints; what a dimension label says (name, formula), how its text lies
  and its size — with the Smaller and Larger buttons.
- **Part** — the default extrusion height and offset.
- **Assembly** — showing sketch outlines, joint glyphs and the interference check.
- **Layout** — where the panels stand: the tree, properties, the tool bars, the status line.
- **For a developer** — the list of the model's elements in the tree, and the record of mouse clicks in a file.

Every section has **“Reset section”**: it restores the factory values in that section only, leaving
the rest alone.

## Colour schemes

There are four: **Dark**, **Light**, **Dracula** and **Alucard** (the light twin of Dracula). The
first two paint the canvas only — their panels and buttons keep the stock look; Dracula and Alucard
paint the whole program.

![Dracula: the scheme paints not only the scene but panels, buttons and fields.](img/scheme-dracula.png)

![Alucard — the same canon on a light background.](img/scheme-alucard.png)

**“Make my own copy”** creates your scheme next to the selected one and opens the editor. Edits show
up live, right on the screen, so a shade can be picked without closing the window. Built-in schemes
cannot be edited: you can always come back to them.

In the editor:

- **the name** and “Rename” — the scheme file moves along with the name;
- **is it light** — this picks the stock look your colours are laid on top of;
- **paint the interface too** — while it is off, panels and buttons keep the stock look;
- **shading** — how dark the darkest face gets, how much a body colour is lifted;
- **colours by section** — window, grid, sketch, dimensions, selection, bodies, planes, previews,
  constraints, assembly, outlines, states, view cube, interface.

Your scheme is a **file** in the settings folder (“Open the folder”). It can be sent as a single
file: the file is named after the scheme, so it is recognisable in the folder at a glance.

## Where the settings live

The path is shown in the window itself, with an “Open the folder” button next to it. Your colour
schemes and document templates live there too — as files you can share.

## The settings profile

The whole set is written to a file and read back: move it to another machine, give it to a colleague,
attach it to a bug report. A profile from another version of the program still loads — what is
missing is taken from the factory values.

## What applies at once and what does not

Almost everything applies at once. The single exception is named in the window itself: **GPU
antialiasing** takes effect the next time the program starts.

## The panel layout

In the **Layout** section every panel picks its place: menu, top, left, right, bottom or the middle.
Click the one you want and the panel moves on the next frame.

Only what you moved is remembered; everything else is taken as it comes, so a panel added in a later
version turns up in its own place instead of going missing.

If it comes out wrong, press **Restore the standard layout** and everything goes back.

## For a developer

The **For a developer** section is for whoever works on the program rather than only using it.

**Show the elements of the model** adds to the tree a list of what the model really holds: faces, edges
and corners. The list is arranged by kind, and inside a kind by body — the heading of a body says how
many elements it has, and its own elements are under that heading. An element carries the same name as
the one the cursor took, so a row and the cursor can be put side by side. Click a row and the element is
lit in the viewport; the arrow keys step from row to row; the field above the list leaves only what you
are looking for. The right button on a row, or Ctrl+C, puts the name of the element into the clipboard,
so a number read off the screen reaches a report without a mistype. The list selects nothing: it is for
reading, and the command you are holding stays as it was.

**Record left-button clicks** writes the path of every click into `clicks.log` in the settings folder; the
path of the file is shown right under the switch. The last line of the file is the place where the
program stopped. If the file is not written, check that the settings folder can be written to and start
the program again: the file is opened on the first click.

Both switches are off in the factory settings. **Reset this section** in it returns both to the factory
values and touches nothing in the other sections.
