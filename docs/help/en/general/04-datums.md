# Datums: planes, axes, points

A datum is auxiliary geometry to build from: a plane for a sketch, an axis for a revolve or an array,
a point as a reference.

![A datum plane next to the part: you can sketch on it where there is no face.](img/datum-plane.png)

- **Datum plane** — a sketch does not have to lie on a face of the part. A plane offset from a base
  plane or from a face gives you a place where no face exists yet.
- **Datum axis** — an axis of revolution, of a circular array, of symmetry.
- **Datum point** — a reference for dimensions, the centre of an array, an anchor for a hole.

## Datums are parametric

A datum is given not by coordinates but by a **definition**: “20 mm off this face”, “an axis through
these two points”. So it follows the geometry: change the part and the datum rebuilds, and with it
everything that stood on it.

The offset takes a formula, like any numeric field.

## Why bother when there is a face

A sketch on a face is tied to that face. If the face disappears — eaten by a fillet, say — the sketch
is left without support. A datum lives its own life and has no such dependency.

Second: a datum exists before any body is built. You start from one when there is nothing to build on
yet.
