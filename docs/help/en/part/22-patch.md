# Patch

Spans a surface across the picked edges. This is the first design-layer shape **that was not on the
body before**: copying a face only repeats what exists, while a patch closes an opening.

![Patch step by step: the opening, the picked boundary edges, the spanned surface.](img/part-patch/)

## How

1. Press **Patch**.
2. Click the edges that define the boundary — at least two. Edges highlight as they do in Fillet;
   clicking a face takes all of its edges at once.
3. **Enter** — the surface appears; **Esc** — cancel.

![What it is for: an open box, a patch, thickness, union — a closed part.](img/part-surface-flow/)

## Worth knowing

- **Smooth or by position.** A switch in the top bar. "By position" simply butts the surface against
  the edges — the seam shows both to the eye and to the hand. "Smooth" brings it in tangent to the
  neighbouring faces, leaving no seam: this is how fairings, housing transitions and rounded blends
  are made. An edge has two neighbouring faces and tangency goes to one of them — the LARGER one is
  taken: what should continue is the surface that defines the shape, not a narrow rim strip.
- **The body stays as it was.** The patch lives beside it as a separate surface. To make it part of
  the part, use **Replace face**.
- **The boundary is stored as a description.** If the edges were chosen through "expand selection"
  (right-click), the patch remembers the rule rather than the numbers, and rebuilds when the base
  changes.
- **Edges need not close into a loop.** An open chain works too: a piece of a dome over three edges is
  an everyday case.
- **If it will not span, you get an honest refusal.** The node turns red and says so; no surface is
  invented.
