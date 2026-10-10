# Offset

A contour or a chain standing off the selected one at a given distance.

![A contour and its offset 6 mm inwards.](img/sketch-offset/)

## How to do it

Choose a contour, a chain of lines and arcs or a circle (by clicks, with **Shift** for several) and press **Offset**;
or the other way round - the button first, then clicks on the curves. Type the **Distance** in the bar above. Point to
the side the copy should go to - the preview is drawn there. **Enter** makes it, **Esc** cancels.

A negative distance lays the copy on the side away from the pointer.

## An open chain

A polyline or a chain of lines and arcs is offset to one side: at a convex joint the copy is rounded by an arc about
the corner, at a concave one it is cut, and the ends of the copy stand opposite the ends of the chain.

## The copy holds to its source

The copy is tied to the source contour: its segments run parallel to their own at the given distance, its arcs from
the same centres. Change a size of the source — the copy follows. The offset of a circle is a concentric circle with a
radius larger or smaller by the distance.

## Where it is used

- **A wall**: the outer contour exists, the inner one is an offset inwards by the thickness.
- **An allowance**: the outline of the part and the outline of the blank around it.
- **A slot along a path**: a centreline and two offsets at half the width.

For an even wall over a whole body, **Shell** in the part is better: it works on the body, not on a flat contour.

## If it did not work

- Inwards does not work — the distance is more than the contour holds: going inwards the sharp corners close in.
  Make the distance smaller.
- The copy did not follow the source — where the offset dropped or merged segments (a narrow ledge inside), the copy
  lies free. Make the distance smaller or split the contour into parts.
- The status says "Offset takes lines, arcs and circles" — nothing chosen is a curve (points only, or construction
  lines). Choose a contour, a chain or a circle.
- The copy went to the wrong side — point nearer the side you want before **Enter**.
