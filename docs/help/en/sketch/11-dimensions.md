# Dimensions

Key **D** — a linear dimension. The angular and the radius/diameter ones are next to it on the panel.

![The same rectangle before and after dimensioning: the labels and the anchor glyph in the corner appear.](img/sketch-dimensions/)

- **Linear**: clicking a line gives its length, or pick two points. While placing it the cursor
  chooses the orientation — aligned, horizontal or vertical.
- **Angular**: two lines (the angle between them) or three points (A — vertex — C).
- **Radius/diameter**: click a circle or an arc.

## A dimension is a constraint, not a label

A dimension **holds** the geometry: change the number and the shape changes. That is why dimensions
remove degrees of freedom, and why they are not added just to look nice.

## A formula instead of a number

A dimension field takes an expression: `40/2`, `w*2`, `len+5`, `sin(30)*10`. Names come from the
global parameters (“ƒx Parameters” in the top bar). This is how dimensions that must change together
are tied.

## Driven dimensions

If a new dimension would be redundant (the same thing is already set by other constraints), the
program makes it **driven**: it shows the measured value but holds nothing.

But if the dimension **conflicts** with the existing ones, it will not be made driven: the conflict
is flagged so you can see it and decide which dimension is the odd one out.

## See also

- [Parameters and formulas](general/05-parameters) — how to name a dimension and use it across the part.
