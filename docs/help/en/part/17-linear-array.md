# Linear array

Select the body, set the count and the direction above, the spacing at the geometry, watch the
preview, press Enter.

![A 3×2 array: the copies are laid out along two directions.](img/part-array-linear.png)

## An array is one feature

All copies live in a single timeline node. So the count and the spacing are edited in one place, and
deleting the array removes every copy at once, leaving no litter.

## What is set

- **Count** — how many in total, including the original.
- **Direction** — along which axis or which edge.
- **Spacing** — the distance between neighbouring copies; it takes a formula.

## A hint

It is convenient to tie the spacing to a global parameter: `pitch`. Then rearranging the holes on a
plate is a change to one number rather than a rebuild of the array.
