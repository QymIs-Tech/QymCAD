# Boolean of bodies

Select body A, press the button, click body B. The action — **Cut**, **Union** or **Intersect** — is
chosen in the top bar.

![A plate and a cylinder overlapped, then the cut: the hole came from a BODY, not from a sketch.](img/part-boolean/)

## Why it exists separately

The ordinary commands (extrude, cut, hole) work from a sketch: an outline gives the shape. A boolean
works with **two finished bodies** — where the tool is already built as a body rather than drawn as an
outline.

That is how you clear a pocket to the shape of a neighbouring part, subtract imported geometry from a
blank, or keep the volume two bodies share.

## Three actions

- **Cut** — the volume of body B is removed from body A. Order matters: A minus B and B minus A are
  different results.
- **Union** — the two bodies become one.
- **Intersect** — only the shared volume is left. Often the shortest way to a complicated shape: two
  simple bodies overlapped, and you keep the middle.

## What happens to body B

It is **consumed**: after the operation the timeline holds the result, and the original body B is no
longer a body of its own. You can see it in the body list — where there were two, one is left. Delete
the boolean node and both bodies come back.

## When it is the right tool

- The cutting tool is itself a body: an import, a primitive, a separately built shape.
- The shape is easier to get by intersecting two simple bodies than from a single outline.

## When it is not

When the shape comes from a sketch. A sketch cut stays tied to its outline: edit the outline and the
cut follows. A boolean ties two BODIES instead, so an edit of the original sketch reaches the result by
a longer road — through rebuilding both bodies.
