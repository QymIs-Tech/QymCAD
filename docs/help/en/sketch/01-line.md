# Line

Key **L**. Draws a segment or a polyline.

![A three-segment polyline: the end of one is the start of the next.](img/sketch-line.png)

## How

Click the start, click the end — the segment is there, and the next one already follows from its
end. A **double click** ends the polyline, **Esc** leaves the tool.

## What is added for you

With auto constraints on, a nearly horizontal segment gets a **horizontal** constraint, a nearly
vertical one gets **vertical**, and an endpoint snapped to someone else's point gets **coincident**.
That is why a freehand outline often comes out almost fully defined.

If the wrong constraint appears, remove it from the constraint list: the geometry itself does not
change, it only becomes freer.

## A hint

It is easier to close a polyline back into the point you started from: a closed outline is what a
body is made of. An open one is useful for sweeping along a path and for construction geometry.
