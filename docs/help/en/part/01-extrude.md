# Extrude

Key **E**; straight as a cut — **Q**.

![A contour rises to a given height — that is an extrusion.](img/part-extrude/)

## How to do it

1. Select a sketch or its contours — or press **Extrude** with nothing selected and click contours: they add up, and
   the bar above shows **Profiles: N**. A sketch you have just finished is already selected — the tool takes it at
   once.
2. Type the **Length** into the field at the geometry or drag the arrow. The preview shows the body before it is
   applied.
3. **Enter** applies, **Esc** cancels.

**Pick contours (U)** takes you back to picking contours after you have gone on to the size.

A region is picked by a click anywhere inside it, not only on its line: a click between a rectangle and a circle in it
takes the ring, a click in the circle takes the disk. The region under the pointer is lit before the click, and a
picked one is filled - its holes stay open, so you see exactly what will be extruded. A second click on a region lets it
go.

## Operation and direction

In the bar above:

- **Add** — material appears; **Cut** — it is removed; **Intersect** — only what is common with the body stays.
- **To a length** — one way; **Symmetric** — equally both ways from the sketch; **Two sides** — a length of its own
  each way, the second in the **Second side** field; for a cut through a body also **Through all**.
- **Flip** — the other way from the sketch. The length is always positive: this button changes the side, not a sign.

A cut that splits the body in two leaves both pieces bodies of the same part. To make a piece a part of its own —
the right button on it -> **Make a part**.

## Several contours — one operation

Pick three contours, and the timeline gets **one** row with three profiles. An edit opens all three at once, a delete
removes the whole operation. A contour inside a contour becomes a hole; pick the inner one too, and it is extruded as an
island inside the hole.

## If it did not work

- The field is red and **Apply** is grey — the value is not allowed (zero, negative, not a number); the reason is
  written at the field.
- **Cut** or **Intersect** on the first sketch of a part refuses in words — there is nothing to cut yet: a body comes
  first.
- A contour cannot be picked — it is not closed. Open the sketch and join the ends.

## See also

- [Revolve](part/02-revolve) — when the shape goes round an axis.
- [Hole](part/08-hole) — instead of a round cut.
- [Fillet](part/05-fillet) — what to do once the shape is ready.
