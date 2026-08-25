# Push face

Click a **planar** face and set the offset at the geometry: plus is outwards, minus is inwards. Enter.

![Offsets of 0, 6 and 12 mm: the face moves and the neighbouring ones stretch after it.](img/part-push-face/)

## Direct editing without a sketch

This is the shortest way to change a finished body: instead of finding the sketch and editing a
dimension, take the face and move it. The neighbouring faces extend after it by themselves.

## The price

Such an edit is not tied to a sketch, and so not to the intent: the offset lives as its own number.
If the size of the part must be parametric, edit the sketch, not the face.

A good use is editing imported geometry, which has no sketches at all.

## When this is the right tool

- Editing **imported** geometry: a STEP file has no sketches at all, and there is simply no other way
  to move a wall.
- A quick try-out: push, look, undo. The offset lives as its own feature and is removed by deleting it.

## When it is the wrong one

When a dimension of the part is supposed to be **parametric**. A face offset lives as its own number
and is not tied to a sketch: editing the sketch will not undo it, and a formula will not reach it. A
part whose thickness is set by a “pushed face” resists editing a month later — the dimension is not
where anyone looks for it.

## What happens to the neighbours

The faces adjoining the moved one stretch after it. If they cannot be stretched, the command refuses
and says why, and the part stays as it was.

That usually happens when the face **borders a fillet or a chamfer**: stretching it would mean
stretching those too, and they are already built. The way out is to put the push earlier in the
timeline, **before** the fillet: drag it up the tree, and the fillet will land on the moved face.
