# Circle

Key **C**. The ways to define it are switched in the top bar.

![A circle with the automatic diameter label.](img/sketch-circle.png)

- **Centre and radius** — the usual way.
- **By two points** — they set the diameter.
- **By three points** — a circle through three given points.
- **Tangent** — a circle touching the chosen line.

## The radius is held by constraints

Tangency, equal radii and point-on-circle are full constraints: move a neighbour and the circle
rebuilds after it. A circle boxed in by tangencies can be fully defined without a dimension at all.

## What next

A closed circle is a ready profile: extrude it into a cylinder or cut a hole with it. For holes in a
body the **Hole** command is usually better: it keeps a reference to the face and survives a rebuild.
