# Extend

![Before and after: the line is stretched to its neighbour, not fitted by eye.](img/sketch-extend/)

## How to do it

Select a line and press **Extend**, or press **Extend** and click the line. Point at what it should reach: the
extension is drawn dashed up to that line, arc or circle, or up to the nearest one on that side. **Enter** or a click
extends it; the tool stays in hand for the next line. **Both sides** in the top bar extends both ends at once, each to
its own nearest geometry. **Esc** lets the line go; a second **Esc** puts the tool down.

Trim and extend are one technique from two sides: draw roughly first, then remove what is extra and stretch what is
short.

## If it did not work

- No dashed line appears — nothing lies ahead of the line on that side: it is parallel to its neighbour or the
  neighbour is too short. Extend the one you are reaching for first, or point at the other end.
- The joint came apart after editing dimensions — extend stretches once and does not hold the joint. To hold it, add
  the **Point on a line** or **Coincident** constraint.
- The line did not extend and the status says its end is joined — that end is shared with another line or an arc, or
  a constraint or a dimension holds it. It is not moved, so as not to pull the neighbours along. Draw a new line from
  that end.
