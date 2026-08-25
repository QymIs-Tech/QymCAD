# Chamfer

Key **C**. Like the fillet: click edges or a whole face, the leg is set at the geometry.

![A chamfer cuts the edge with a plane — unlike a fillet, the surface stays flat.](img/part-chamfer.png)

## How it differs from a fillet

A chamfer cuts the corner with a plane, a fillet with an arc. For assembly these are different
things: a chamfer on an edge eases the entry of a part and removes the burr, a fillet reduces stress
concentration.

## In practice

- A chamfer for welding or for entry is usually put on all outer edges at once — click the face and
  all of its edges are taken in one go.
- On a hole a chamfer is a countersink; if the hole was made by the Hole command, it is simpler to
  set the countersink there than to add a separate chamfer feature.

## If it fails

The reason is the same as with a fillet: the leg is larger than the neighbouring face allows. The
per-edge breakdown shows exactly which edge is in the way.

## See also

- [Fillet](part/05-fillet) — the same edge, but rounded.
