# Offset

Builds an outline at a given distance from the selected one. The distance is in the top bar.

![An outline and its 6 mm inward offset.](img/sketch-offset/)

## Where it is needed

- **A wall**: you have the outer outline, the inner one is an offset inwards by the thickness.
- **Stock allowance**: the part outline and the blank outline around it.
- **A groove along a path**: a centre line and two offsets by half the width.

## About inner corners

Offsetting inwards either rounds a corner or makes it self-intersect — that is a property of the
operation itself, not of the program. If the outline looks tangled after an offset, reduce the
distance or split the outline into stretches.

## A hint

For an even wall over the whole body the **Shell** command in the Part workbench is usually better:
it works with the body rather than a flat outline and handles all faces at once.
