# Circle

Key **C**.

![A circle with its diameter labelled automatically.](img/sketch-circle.png)

## How to draw it

Press **Circle** and choose the way in the bar above:

- **centre + radius** — a click at the centre, a second click sets the radius;
- **by 2 points** — two clicks set the ends of a diameter;
- **tangent** — a circle of a given radius touching two curves: type the radius in the field of the bar, click the
  first curve, then the second (a line, an arc, a circle, an ellipse, a spline), lead the pointer to where the circle
  should stand — it follows the pointer — and click;
- **by 3 points** — three clicks on the points the circle passes through.

As soon as the circle lies down it gets a **Diameter Ø** dimension with its field open: type a number or a formula
and press **Enter** — or just keep drawing. The tool stays in hand; **Esc** puts it down.

## A circle holds by constraints

Tangency, equal radii, a point on the circle are full constraints: move the neighbour and the circle rebuilds after
it.

A closed circle is a ready profile: extrude it into a cylinder straight away. For a hole in a body, [Hole](part/08-hole)
is handier: it stands on a face and follows it.

## If it did not work

**tangent** draws nothing — there is nothing to touch: draw the two curves the circle should touch first. If no
circle follows the pointer once two curves are picked, no circle of that radius touches both — change the radius in
the bar.

**by 3 points** draws nothing when the three points lie on one straight line: no circle passes through them.
