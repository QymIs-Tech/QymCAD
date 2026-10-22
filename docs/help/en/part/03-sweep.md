# Sweep

![A round section run along a broken-line path.](img/part-sweep.png)

The profile travels along the path and sweeps a body: a pipe, a handrail, a cable duct, a seal along an outline.

## How to do it

1. Select the **profile** sketch — a closed contour of the section — and press **Sweep**.
2. Pick the **path**: click its sketch in the tree - an open path or a closed one - or click edges of a body in the
   view, one after another, so that each edge starts where the one before it ends. The bar above shows **Path:** with
   the sketch's name or the number of edges.
3. **Enter** applies, **Esc** cancels.

The profile and the path are separate sketches. Start the path on the profile's plane, at its contour: the body runs
from the profile where it is drawn, from the end of the path that touches it. A profile drawn away from the path is
carried to its start, across it, by the origin of its sketch.

A path along edges of a body follows those edges when the body changes, and the body swept along them is joined to
it, cut from it or met with it. A click on a face takes all of its edges; a sketch picked in the tree after the edges
takes the place of the edges, and edges clicked after a sketch take the place of the sketch.

## If it did not work

- The body is not built — the path turns more sharply than the profile allows: with a bend radius smaller than the
  profile, the body would cross itself. Make the bend radius larger or the profile smaller.
- The body stands somewhere else — the path does not start on the profile's plane, so the profile was carried to its
  start. Start the path at a point of the profile's contour.
- The bar reads **Path?** — no path is picked yet: click its sketch in the tree or an edge of the body.
- The edges of the path are refused — they do not join end to end into one chain: pick edges that follow one another.
- The path lost edges after an edit above it — the body was rebuilt without them: open the sweep and pick the edges
  again.
- **Cut** or **Intersect** is refused while a sweep made with **Add** along a sketch is edited — delete the sweep and
  make it again with that operation.
- A closed path gives a ring, an open one a body with two ends; both are allowed.
