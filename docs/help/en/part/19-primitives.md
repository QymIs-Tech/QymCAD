# Primitives

A body without a sketch, from sizes alone. The buttons are in the “Primitives” group; the sizes are
set at the geometry, Enter applies.

![A box and a cylinder — ready-made bodies, no sketch needed.](img/part-primitives.png)

- **Box** (key **B**) — length, width, height.
- **Cylinder** (key **Y**) — diameter and height.
- **Sphere** — diameter.
- **Cone** — bottom diameter, top diameter, height. A zero top diameter gives a sharp cone, a
  non-zero one a truncated cone.
- **Torus** — ring diameter and tube diameter.
- **Prism** — the number of sides is set above, then the diameter and the height.

## Why bother when there is a sketch

Speed. For a roller, a boss, a pin or a blank a sketch is an extra step: the shape has nothing but
two or three numbers, and those numbers are just as parametric as sketch dimensions.

## How they behave afterwards

A primitive is a material feature like an extrusion: it carries the single body of the part. A second
primitive will not create a second body, it merges with the first.

## When a sketch is better

As soon as the shape stops being described by those numbers — a flange, a flat, a groove is needed —
move to a sketch. Assembling a complex part out of primitives costs more than drawing an outline.
