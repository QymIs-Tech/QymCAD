# Fillet

Key **F**. Click **edges** — or a whole face, then all of its edges are taken. The radius is set at
the geometry, Enter applies.

![As the radius grows the fillet eats material while the overall size stays put.](img/part-fillet/)

## A fillet holds on to the edge, not to a number

Change the extrusion height or move a wall — the fillet stays on the same edge. A change higher up the
history does not knock it off.

## If it fails

The usual reason is **a radius larger than the geometry allows**: the neighbouring face is shorter
than the radius, or two fillets met and ate each other. In that case the program does not say “it
failed” — it breaks the answer down per edge: which one takes no radius at all, which one takes no
more than a given value.

Read that breakdown — it is the instruction on what to fix: reduce the radius, drop one of the edges,
or move the fillet earlier in the timeline.

## Order matters

A fillet placed before a cut and the same fillet after it are different shapes. Usually fillets go
last, once the main shape is there.

## See also

- [Chamfer](part/06-chamfer) — the same edge, cut flat.
- [The history timeline and rollback](general/03-timeline) — why fillets come last.
