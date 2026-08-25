# Stitch

Joins several surfaces into one. Click the sheets, set the tolerance if needed, press **Enter**.

![Two separate surfaces, one stitched out of them, and the same one thickened — that is what stitching is for.](img/part-stitch/)

## Why

A surface is rarely born whole: a patch here, a copied face there, a third piece in between. While
they are separate bodies you can neither work with them as one surface nor thicken them — thickening
would take each piece on its own and give a stack of plates instead of a lid.

## How

1. Press **Stitch**.
2. Click the sheets that will become one surface. Clicking again removes a sheet from the set.
3. **Enter** to apply, **Esc** to cancel.

## Worth knowing

- **Closed means a solid.** If the stitched surfaces surround a volume on all sides, the result is an
  ordinary solid rather than a shell: asking for another step would mean asking you to confirm what
  the program already sees.
- **Stitching joins surfaces, not solids.** Clicking a part selects nothing and says so right away,
  not after Enter.
- **The pieces are absorbed.** One surface is left on screen, not the surface plus its parts.
- **Nothing joined is an honest refusal.** Sheets that do not touch would give the same two islands
  under one name; further down the timeline such a surface behaves like garbage.
- **Tolerance** is how far apart edges may lie and still count as shared. Start from the default and
  raise it only when you know the seam is imprecise.
