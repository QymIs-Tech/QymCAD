# Extend

Click the end of a line and it reaches the nearest intersection with another entity.

![Before and after: the line is extended to its neighbour, not dragged by hand.](img/sketch-extend/)

## The pair to trim

Trim and extend are one technique from two sides: draw roughly, cut away the extra, stretch out what
is missing. Both rely on intersections, so it is easier to work when the lines deliberately overlap.

## If nothing happens

There is nowhere to extend to when there is no intersection: the line is parallel to its neighbour
or the neighbour is too short. Extend the one you are reaching for first, or use the **point on
line** constraint — unlike a plain extend, it holds the connection through later changes.
