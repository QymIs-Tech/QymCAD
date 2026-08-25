# Where to start: your first part

A ten-minute lesson. At the end you get a plate with a hole and rounded corners, built so that its
dimensions can be changed at any moment and the part rebuilds itself.

If you have used a parametric 3D modeller before, you will recognise nearly everything. If you have
not, read it straight through: no steps are skipped here.

## 1. A sketch — the flat drawing everything starts from

Press **K** or the “Sketch” button and pick the XY plane. The program switches to the Sketch
workbench: the view goes flat and the drawing tools appear on the left.

Press **R** (rectangle) and click twice — one corner and the opposite one. Precision does not matter
yet: dimensions will be given as numbers, not by mouse. That is the main habit of parametric CAD —
**draw roughly, dimension exactly**.

## 2. Dimensions — what holds the shape

Press **D**, click the bottom side of the rectangle and type `40`. Then the right side — `30`.

At the bottom of the window there is a **degrees of freedom** counter. It shows how much in the
sketch is still undecided. While it is above zero the shape can drift. Drive it to zero and the
rectangle turns black and stops moving under the mouse: it is fully defined.

> Do not chase zero at any cost. A sketch with two or three degrees of freedom works fine; it just
> reserves the right to drift when you change something higher up the timeline.

Press **Esc** to leave the sketch.

## 3. Extrude — from flat to solid

Select the sketch outline and press **E**. A bar with the command parameters appears on top:
operation, height, direction. Type a height of `10` and press **Enter**.

![The outline rises to the given height — that is extrusion.](img/part-extrude/)

Note that until you press Enter the document is unchanged — what you see is a **preview**. Every
command works this way, without exception.

## 4. A hole is not a round cut

Select the body, press **O**, click the top face, set the diameter to `10` and the depth to
“through”.

![A through hole in the top face: the command remembers the face, not coordinates.](img/part-hole.png)

You could have made the hole as a circular cut, but that would be a different thing. A hole keeps a
**reference to the face**: change the part higher up the timeline and the hole stays where it was
meant to be, not where the coordinates happened to land.

## 5. Fillets — and why they come last

Press **F**, pick the four vertical edges, set the radius to `5`, Enter.

![As the radius grows the fillet eats material while the overall size stays put.](img/part-fillet/)

Fillets and chamfers go **at the end**. Not out of taste: they breed faces and edges, and anything
built on those breaks at the slightest change of shape. Shape first, then holes, then fillets — that order saves
hours.

## 6. And now the important part: change a dimension

Find the sketch in the tree on the left, open it with a double click, change `40` to `60`, leave it.

The part rebuilt as a whole: the plate got longer, the hole stayed on its face, the fillets on their
edges. You redrew nothing.

That is what a parametric model is for: it stores not a shape but a **recipe** for a shape. The
timeline on the left is that recipe, and it can be edited anywhere.

## What next

- [The second lesson: an assembly](start/02-assembly) — how to put parts against each other.
- The **Sketch** section — every drawing tool and the constraints between them.
- The **Part** section — every construction command.
- **Parameters and formulas** — how to keep one number the whole part depends on.
