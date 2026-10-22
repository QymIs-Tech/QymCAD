# Break

Click a curve — a line, an arc, a circle, an ellipse or a spline — it splits at the point of the click; a closed one
splits into two halves, at the click and opposite it.

![A whole segment and the same one broken at the intersection: from here each half is its own line.](img/sketch-break/)

## When you need it

- To give one half **its own dimension**: while the segment is one, a dimension sets its whole length.
- To **delete the middle** of a segment without touching its ends.

## What happens to the constraints

The halves meet in one shared point, so the outline stays closed. A horizontal or vertical constraint
of the original line goes to each half. Dimensions and other constraints stay on the old ends.

## How it differs from trim

[Trim](sketch/12-trim) **removes** a piece. Break removes nothing — it splits one entity into two,
leaving both where they were. Visually nothing changes after a break: the line looks exactly as
before.

It is easy to check — click one half: only that half gets selected.
