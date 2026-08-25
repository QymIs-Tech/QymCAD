# Move, copy, rotate

Three buttons of one kind: select the entities, then say where.

![Copying: the original rectangle stays where it was, the copy lands where you pointed.](img/sketch-copy/)

- **Move** — click the base point, click the target. The selection travels there.
- **Copy** — the same, but the original stays.
- **Rotate** — click the centre, then type the angle.

## This edits the sketch, it does not build

None of the three creates constraints or leaves a node in the timeline: they move what is already
drawn. The result therefore behaves like ordinary geometry — you can pin it down with dimensions and
constraints, or leave it free.

## Base and target snap to existing points

A click snaps to ends and centres, so “move it by the corner of the rectangle onto the centre of the
circle” takes two clicks and lands exactly, with no numbers.

## A copy is not an array

A copy lives its own life: an edit of the original never reaches it. If you need several copies that
change together with the original, use an **Array** (linear or circular), not copying.

## Mind the constraints

The selection may be tied to what stayed behind. Moving an entity that dimensions and constraints hold
will be undone by the solver — it puts the entity where the constraints say. Remove what is in the way
first, then move; or set the position with a dimension rather than with the mouse.
