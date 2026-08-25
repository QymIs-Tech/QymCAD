# Project edges

Click the edges of a body and they appear in the sketch as entities. The “Face outline” switch above
takes the whole outline of the picked face at once instead of edge by edge.

![The outline and the hole circle are taken from the body: real sketch entities, not a backdrop.](img/sketch-project.png)

## What it is for

So that a new sketch stands on geometry that already exists: a hole coaxial with a boss, a cut-out
along the edge of a flange, a groove along a rib. Tracing that by hand means typing the same numbers
a second time and losing the link to the original.

## A projection is driven

A projection is not redrawn: it **follows** the body. Change the part higher up the history and the
outline in the sketch moves with it, and so does everything built on it.

That is why a projection cannot be dragged by hand: it has no degrees of freedom of its own, the
source sets them.
