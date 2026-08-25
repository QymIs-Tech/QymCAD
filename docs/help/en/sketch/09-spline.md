# Spline

Key **N**. A smooth curve through points: click the knots, a **double click** finishes it.

![A spline through four points: a smooth curve passing through them.](img/sketch-spline.png)

## Careful with definition

A spline is the freest entity in a sketch: every knot has two degrees of freedom, and defining it
completely with dimensions is next to impossible. That is fine for a shaped surface and bad for a
critical outline.

The rule is simple: where the shape is set by a **function** (a fairing, aerodynamics, ergonomics) a
spline belongs; where it is set by a size (a fit, a hole, a joint) lines and arcs are better.

## A hint

Pin the spline ends to their neighbours with coincidence, otherwise the outline will come apart at
the joint on the first change.
