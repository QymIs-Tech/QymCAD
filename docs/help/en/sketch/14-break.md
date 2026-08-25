# Break

Clicking a line splits it in two at the point of the click.

![A whole segment and the same one broken at the intersection: from here the halves live apart.](img/sketch-break/)

## When you need it

- To give one half **its own dimension**: while the segment is one, a dimension sets its whole length.
- To **delete the middle** of a segment without touching its ends.
- To free a stretch for a corner fillet or a chamfer.
- To insert an arc between the halves without redrawing the outline.

## What happens to the constraints

Both halves inherit the constraints of the original line, and a coincidence of the ends appears at
the break point. It keeps the outline closed: to pull the halves apart, remove that coincidence —
otherwise they move together.

## How it differs from trim

[Trim](sketch/12-trim) **removes** a piece. Break removes nothing — it splits one entity into two,
leaving both where they were. Visually nothing changes after a break: the line looks exactly as
before.

It is easy to check — click one half: only that half gets selected.
