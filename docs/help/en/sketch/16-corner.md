# Corner fillet and chamfer

Click the corner of two lines. The size is set in the top bar.

![A sharp corner and the same corner rounded: the radius becomes a dimension and the tangencies become constraints.](img/sketch-corner/)

## What happens

A fillet inserts an arc between the lines and adds **two tangencies**, one to each side. A chamfer
inserts a segment at the required angle. Both stay constrained: move a side and the corner rebuilds
itself.

## Why not draw the arc by hand

You can, but then both tangencies are yours to place, and either is easy to forget. A forgotten
tangency is invisible in the picture — it shows up later, when the outline comes apart at the joint
on the first change of a dimension.

## A hint

Corners are usually rounded **last**, once the outline is defined: before that they get in the way
of snapping to corners and confuse the count of degrees of freedom.
