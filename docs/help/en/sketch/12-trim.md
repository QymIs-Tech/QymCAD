# Trim

Key **K**. Removes the extra piece of a line — up to the nearest intersections.

![Before and after: a click on the tail right of the crossing removes exactly that tail.](img/sketch-trim/)

Click a segment and exactly the part between the intersections with its neighbours disappears.
**Drag the cursor** and everything it passes through is trimmed: that is the fastest way to clean up
a mesh of lines.

## What it is for

This is how outlines are built “from layout”: first draw crossing lines and circles without caring
about the boundaries, then trim away the extra. It is faster than aiming each segment exactly from
corner to corner.

## About constraints

Trimming removes geometry and with it the constraints that were held on it. So after a trim the
degrees of freedom go up: the outline became freer and needs defining again.
