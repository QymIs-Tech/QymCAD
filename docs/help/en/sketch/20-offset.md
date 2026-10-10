# Offset

A contour or a chain standing off the selected one at a given distance.

![A contour and its offset 6 mm inwards.](img/sketch-offset/)

## How to do it

1. Choose a contour, a chain of lines and arcs or a circle (by clicks, with **Shift** for several) and press **Offset**.
   Or the other way round - the button first, then clicks on the curves.
2. Point to the side the copy should go to - the preview is drawn there.
3. Click on that side, off the curves. The side is fixed, the preview changes colour, and the **Distance** box opens
   beside the copy. The pointer can now go to the box - the copy stays where it is.
4. Type the distance - the preview follows the number. **Enter** or the tick makes the copy.

**Enter** without a click makes the copy at once, on the side of the pointer, with the distance of the bar above.

After a copy the tool stays in hand: choose the next curves and go on. **Esc** in the box frees the side - the copy
follows the pointer again; one more **Esc** puts the tool down.

A negative distance lays the copy on the side away from the pointer.

## An open chain

A polyline or a chain of lines and arcs is offset to one side. A corner between two lines stays a sharp corner in the
copy - the lines of the copy run on to where they meet. Where an arc meets a line, a convex joint is rounded. The ends
of the copy stand opposite the ends of the chain. A closed contour of lines keeps sharp corners in its copy too.

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
- The copy went to the wrong side — press **Esc** to free the side, and click nearer the one you want.
- The click added a curve instead of fixing the side — it landed on a line. Click on an empty place beside the copy;
  take the extra curve off with another click on it.
