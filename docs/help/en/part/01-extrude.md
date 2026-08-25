# Extrude

Key **E**; straight to a cut — **Q**.

![The outline rises to the given height — that is extrusion.](img/part-extrude/)

Select one or more sketch outlines, set the **operation** and the height above, press Enter.

## The operation matters more than it looks

- **Add** — material appears.
- **Cut** — material is removed.
- **Intersect** — only the common part stays.

This is one command with three modes, not three different tools. That is why “Extrude” and “Cut” are
a single button: they share the fields, the preview and the timeline node.

## Several outlines — one node

Select three outlines and you get **one** feature with three profiles, not three features. That
matters: editing reopens all three at once, and deleting removes the whole operation without
breaking the part.

Nested outlines (an outline inside an outline) are handled by themselves: the inner one becomes a
hole.

## Direction and depth

The height is a number or a formula. A negative height extrudes the other way.

## See also

- [Revolve](part/02-revolve) — when the shape goes around an axis.
- [Hole](part/08-hole) — instead of a circular cut.
- [Fillet](part/05-fillet) — what to do once the shape is done.
