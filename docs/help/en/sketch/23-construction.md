# Construction geometry

The "Construction" button or the **X** key. It turns the selected lines, arcs and circles into construction geometry:
drawn dashed, and left out of the contour a body is made from.

## How to use it

Select elements with the arrow and press the button (or X). Pressed again on elements already construction, it turns
them back into ordinary geometry. With nothing selected the button asks what to turn.

## Drawing it as construction

On the top bar of a drawing tool (the line, the rectangle, the circle and the others) tick **Constr.** - whatever is
drawn is laid dashed at once. It is tied as ordinary geometry is: the pointer snaps to points and lines, a line from a
corner starts on that corner, level and upright are laid by themselves.

Beside it, **No ties** puts down what is drawn exactly where you click - the pointer snaps to nothing while drawing,
and nothing is tied to anything. It works for ordinary and construction geometry alike.

## What it is for

Construction geometry is layout: axes of symmetry, the circle the holes stand on, the diagonal that finds the centre of
a rectangle. It takes constraints and dimensions as ordinary geometry does, but it never reaches the body: an extrusion
sees only the solid contours.

## If it did not work

- **The contour no longer closes.** A side of the contour was turned into construction - turn it back.
- **Nothing changed.** Check that the elements are selected: what is selected is lit.
