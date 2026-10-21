# Sweep

![A round section run along a broken-line path.](img/part-sweep.png)

The profile travels along the path and sweeps a body: a pipe, a handrail, a cable duct, a seal along an outline.

## How to do it

1. Select the **profile** sketch — a closed contour of the section — and press **Sweep**.
2. Click the **path** sketch in the tree: an open path or a closed one. The bar above shows **Path:** with its name.
3. **Enter** applies, **Esc** cancels.

The profile and the path are separate sketches. Start the path on the profile's plane, at its contour: the body runs
from the profile where it is drawn, from the end of the path that touches it. A profile drawn away from the path is
carried to its start, across it, by the origin of its sketch.

## If it did not work

- The body is not built — the path turns more sharply than the profile allows: with a bend radius smaller than the
  profile, the body would cross itself. Make the bend radius larger or the profile smaller.
- The body stands somewhere else — the path does not start on the profile's plane, so the profile was carried to its
  start. Start the path at a point of the profile's contour.
- The bar reads **Path?** — no path is picked yet: click its sketch in the tree.
- A closed path gives a ring, an open one a body with two ends; both are allowed.
