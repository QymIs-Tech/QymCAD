# Projection

Click the edges of a body and they appear in the sketch as entities. The “Face outline” switch above
takes the whole outline of the picked face at once instead of edge by edge. The body may be this
part's own or another part of the assembly.

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

Draw from it as from any line: the pointer snaps to its corners, its middles and along it, and the line drawn is tied
there - when the body changes, what is tied to the projection follows it. Dimensions and constraints put on it hold the
same way. A fillet or a chamfer put on the body higher up the history comes into the projection as its arc or its line,
and what was tied to the sides stays on them.

Nothing projected, a line, a rectangle or a circle started on a thin grey edge or corner of the face under the sketch
takes that edge in by itself, as construction, and is tied to it; so does a dimension clicked on a grey edge.

## Making it ordinary or construction

- **Construction**: pick a projected line and press **X**, or right-click it and press **Construction / normal**. It is
  drawn pink, dashed and thin, and still follows the body.
- **Ordinary geometry**: pick it, right-click and press **Make ordinary geometry**. The curves stay where they are, are
  dragged and dimensioned as any line, and the body no longer moves them.
- **Cut**: delete a projected line, trim it or break it - only that line lets go of the body: what is left of it is
  ordinary geometry, held level, upright and square where it stands so. The other lines of the outline still follow
  the body.

## If it did not work

- A line of the projection does not follow the body any more - it was made ordinary geometry, cut, or the edge it came
  from is gone from the part. It is ordinary geometry now: project the edge again if it should follow.
- A projected line does not drag - that is right while it is projected; make it ordinary geometry first.

## Another part's geometry

When one part's sketch stands on another part's geometry, an explicit **external reference** appears
between them. It shows in the properties of the part and can be broken — [more](assembly/06-external-refs).

This is top-down design: the bracket's dimensions come from the housing instead of being retyped
into it. Move the source part in the assembly and the outline follows.
