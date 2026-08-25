# Ellipse

Key **E**. Centre and angle: the first click sets the centre, then the direction and the semi-axes.

![An ellipse with 26 and 14 mm semi-axes.](img/sketch-ellipse.png)

## When you need it

An ellipse shows up where a round section is cut at an angle: a slanted branch pipe, a hole in a
slanted wall, a decorative cut-out. If what you really need is a slanted cylinder, it is safer to
extrude a circle and tilt the body — an ellipse in the sketch gives the same shape but loses the
link to the original diameter.

## What defines it

A centre and two semi-axes, major and minor. Rotation is a separate parameter, so a tilted ellipse
stays an ellipse instead of turning into a spline.

## Where it is used

Oval windows and hatches, eccentrics, transitions between round and flat. An ellipse is an exact
curve, not an approximation made of arcs: a surface extruded from it stays smooth, and a machine
cuts it without steps.

## Do not confuse it with a slot

A [slot](sketch/07-slot) is two semicircles and two straight lines; its width is constant along its
whole length. An ellipse narrows towards its ends. A screw guide needs a slot; a shape needs an
ellipse.
