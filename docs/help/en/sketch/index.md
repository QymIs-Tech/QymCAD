# Sketch: an overview

A sketch is the flat drawing every part stands on. You draw an outline, give it constraints and
dimensions, then turn it into a body: extrude, revolve, sweep.

![A rectangle and a circle with constraint glyphs: the green squares are horizontals and verticals placed automatically.](img/sketch-constraints.png)

## The main rule: a sketch should be fully defined

The status line shows the **degrees of freedom** — how much can still be moved without breaking any
constraint. While that number is above zero the sketch floats: it looks right, but the first change
higher up the history can move it.

A fully defined sketch (zero degrees of freedom) behaves predictably: change a dimension and exactly
what that dimension controls changes.

## What it is made of

- **Entities** — lines, circles, arcs, splines. What you see.
- **Constraints** — rules between them: horizontal, vertical, coincident, parallel, perpendicular,
  equal, tangent, symmetric, point on line, point on circle.
- **Dimensions** — constraints with a number: length, angle, radius. The number can be a **formula**.

## How drawing works

A tool is switched on by a button on the left or by its hotkey and stays on: you can draw several
lines in a row. **Esc** leaves the tool.

**Auto constraints** (the wand in the top bar) add the obvious ones for you: draw a nearly
horizontal line and you get a horizontal constraint. It saves time but sometimes guesses wrong; then
delete the constraint from the list.

## Construction geometry

The **X** key toggles construction mode: such lines are dashed, never become part of a body profile
and exist only as support for constraints — centre lines, diagonals, circles for arrays.
