# Trim

Cuts the excess off a surface along a neighbouring body. Click **the part that stays**, then the body
you are cutting with — **Enter**.

## Why

The other half of surface work. A skin is stretched deliberately LARGER than needed and then trimmed
in place: easier than fitting the sections beforehand, which means guessing.

## How

1. Press **Trim**.
2. Click the surface — exactly the part of it that should stay. The same click also picks the sheet:
   asking for the side as a separate step would split one gesture in two for data already given.
3. Click the tool body.
4. **Enter** to apply, **Esc** to cancel.

## Worth knowing

- **The side is remembered as a POINT, not a piece number.** A number is a property of today's
  traversal order: after the base changes it would point somewhere else. A point survives both a
  shift and a stretch.
- **The tool is not consumed.** Keep cutting with it — every sheet in turn, if you like.
- **The tool must reach past the edge.** Once it stops cutting all the way through, the piece beyond
  the end of the cut is connected to the rest and stays with it. That is not a bug, but it looks like
  "trim stopped working": check that the tool is still wider than the surface.
- **Nothing cut off is an honest refusal.** A feature that "worked" and changed nothing is worse than
  a red node: you never find out about it.
