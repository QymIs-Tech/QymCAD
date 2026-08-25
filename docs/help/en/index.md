# Help

This program is a parametric CAD: you draw a **sketch**, turn it into a **body**, and assemble
bodies into an **assembly**. The whole history stays alive: change a dimension in the sketch and the
part rebuilds itself.

![The program window: tools and the tree on the left, the model in the centre, properties on the right.](img/window.png)

## Where to start

Start with the lessons — they are short and run in order, with no steps skipped:

1. [Your first part](start/01-first-part) — sketch, dimensions, extrude, hole, fillet, and the important bit: change
   a dimension and watch the part rebuild itself.
2. [An assembly](start/02-assembly) — components, joints, degrees of freedom.

## What it is all made of

1. **Sketch** — a flat drawing: lines, circles, arcs. All geometry stands on it.
2. **Part** — turning a sketch into a body: extrude, revolve, sweep. The body is then refined with
   fillets, holes and shells.
3. **Assembly** — several parts placed against each other by joints.

## How the window is laid out

Tools of the current workbench are on the left. The document path and the parameter bar of the
active command are on top. Properties of the selection are on the right. The status line at the
bottom says what is happening, how many degrees of freedom the sketch has and where the cursor is.

## Rules shared by every tool

- A tool is a **command**: pick what it stands on, type the values, watch the preview, **Enter**
  applies, **Esc** cancels.
- Every numeric field takes a **formula**: `40/2`, `len*2`, `sin(30)*10`. Names come from the global
  parameters.
- Nothing is applied on the fly: until you press Enter the document is unchanged.

## Where to look next

- [The program window](general/01-window) — what lives where.
- [Parameters and formulas](general/05-parameters) — one number for the whole part.
- [The history timeline and rollback](general/03-timeline) — why a model is a recipe, not a picture.
- [Keyboard shortcuts](general/10-hotkeys) — the full reference and how to reassign them.
- [Report a problem](general/13-report) — something does not work: how to tell it so it gets fixed.

**F1** at any moment opens the article about what you are doing right now, not the contents page.

![The help window: contents by section on the left, the article itself on the right.](img/help-window.png)
