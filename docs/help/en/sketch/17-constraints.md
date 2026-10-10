# Sketch constraints

The constraint buttons stand on the left, in the "Constraints" group. A constraint is a rule between elements of the
sketch: a line is horizontal, two lines are equal, a circle touches a line. A dimension sets a number, a constraint a
position; together they define the sketch.

## How to add a constraint

Two ways, one result:

- **Selection first.** Select what you tie with the arrow (Shift adds to the selection), press the constraint's
  button. The constraint is added at once.
- **Button first.** Press the button with nothing selected - the status line asks for the elements. Click them on the
  canvas; the constraint is added as soon as it has what it needs. Esc cancels.

If the selection is not what the constraint takes (two points for parallel, two lines for a midpoint), the program
says what it needs and starts picking afresh.

## What there is

| Constraint | What to point at | What comes of it |
|---|---|---|
| Horizontal | a line or two points | the line (or the segment between the points) becomes horizontal |
| Vertical | a line or two points | it becomes vertical |
| Parallel | two lines | the lines become parallel |
| Perpendicular | two lines | they meet at a right angle |
| Equal length | two lines | the lengths become equal; for circles, the radii |
| Collinear | two lines | they lie on one straight line |
| Concentric | two circles or arcs | the centres coincide |
| Tangent | a line and a circle | the line touches the circle; short of it, the nearer end runs on to the touch point; picked at its middle, it touches by the middle |
| Midpoint | a point and a line | the point moves to the middle of the line |
| Coincident | two points, or a point and a line | the points meet; the point lies on the line |
| Fix | points | the points stay where they are through any edit |
| Symmetric | two points and a line as the axis | the points mirror each other about the axis |

## Where they show

Every constraint is drawn as a mark beside its geometry and as a row in the list of constraints on the right. A click
on the mark selects the constraint, Delete removes it. The degrees of freedom at the bottom of the window say how much
is left to define.

## If a constraint is not added

- **It argues with others.** The program lights the conflict red: remove the extra constraint or dimension.
- **It is redundant.** If the same is already set by other constraints, the new one changes nothing - the sketch is
  defined already.
- **The wrong things are selected.** Read the status line: it names what this constraint takes.
