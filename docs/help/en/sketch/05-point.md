# Point

Key **P**. Places a node — a point that draws nothing but takes part in constraints.

![Four points hold the centres of the circles: they never become material, but they say where it goes.](img/sketch-point.png)

## What it is for

A point is an **anchor**. It never becomes material, but things refer to it:

- the centre of a future hole: dimensions go to the point, and the hole is made from it (the Hole
  command can lay out holes straight from sketch points);
- the centre of a circular array or an axis of symmetry;
- where a vertex has to land in an assembly joint;
- a node several dimensions hold on to at once — then one number is edited instead of five.

## A point takes constraints too

It can be made coincident with another point, put on a line (**point on line**) or on a circle, or
made the midpoint of a segment. Each of these removes degrees of freedom, so points are a convenient
way to pin a sketch to geometry that already exists.

## A point has degrees of freedom too

A free point adds two: it can move in X and Y. Until it is pinned by dimensions or constraints,
everything resting on it moves with it. That is a common cause of “the part fell apart after an
edit”: an undefined point beneath the feature.

## Construction point

The **X** key turns a point into construction geometry: it stays visible in the sketch but takes no
part in the outlines and never affects the body profile.
