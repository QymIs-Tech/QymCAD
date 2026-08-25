# Settings

The settings window is split into sections; above them is a search that finds a setting by its label.

![The settings window: sections on the left, search above them, “Reset this section” at the bottom.](img/settings.png)

- **General** — language, autosave, undo depth, how many recent files to keep, the settings profile.
- **Appearance** — colour scheme, interface scale.
- **Viewport** — engine, projection, shading, view cube, pointing precision, ghost transparency,
  field of view, antialiasing.
- **Sketch** — snapping, grid step, rotation step, auto constraints.
- **Part**, **Assembly** — default values and what auxiliary geometry to show.
- **Machining** — appears only when the module is on.

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
antialiasing** takes effect the next time the program starts, because it is baked into the drawing
pipelines at startup.
