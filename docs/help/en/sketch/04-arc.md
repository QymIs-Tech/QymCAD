# Arc

Key **A**. Three ways, the switch is in the top bar.

![An arc by its centre and two ends.](img/sketch-arc.png)

- **Centre — start — end**: the first click sets the centre, the second the start, the third the end.
- **By three points**: start, end and a point on the arc.
- **Tangent**: an arc continuing the chosen segment without a kink.

## An arc is held by constraints

Tangency between an arc and a neighbouring segment is a real constraint: move the segment and the arc
stays tangent instead of splitting at the joint. The radius takes part in the solve as well, so equal
radii and point-on-arc work just as they do for a circle.

## A hint

Rounding a corner in a sketch is easier with the **Corner fillet** tool (click the corner of two
lines): it places the arc and both tangencies for you.
