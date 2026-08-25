# Copy face

Takes a face off a body as a **separate surface**. The body itself stays put: a copy is a copy, not
"cut a piece off the part".

![The part, a copy of its face taken off (moved aside so you can see it) and the same face handed back.](img/part-face-copy/)

## Why

This is the bridge from the parametric part into the design layer. The surface can be reshaped on its
own and then handed back to the body with "replace face". While it lives apart, everything further
down the timeline keeps rebuilding as usual.

## How

1. Press **Copy face**.
2. Click faces of the body — you can pick several.
3. **Enter** — the surface appears in the tree as its own body; **Esc** — cancel.

## Worth knowing

- **Faces are stored as a description.** If you said "all faces of this feature" or "all parallel to
  this one" beforehand (right-click on a face), the copy remembers exactly that and follows the base
  when it changes. Otherwise the picked list is stored.
- **A surface has no volume.** It takes no part in mass properties and never goes to machining: it is
  a sheet, not a solid. The document knows the difference.
- **The source body does not disappear.** If the part vanished after a copy, that is not this tool.
