# Joint limits and drives

A movable joint has not only freedom but also its bounds.

![The same mechanism driven to 0°, 40° and 80°: the arm moves by the rule, not by hand.](img/assembly-drive/)

## Limits

A limit restricts motion: a hinge opens to 110°, not to 360°; a slider travels from zero to its
stroke. The solver honours a limit that is set — the part will not go past it, neither by dragging
nor on a recompute.

Why: the assembly stops showing impossible positions. A lid that opens to 200° in the model will hit
the housing in real life, and it is better to see that now.

## Drives

A drive is a set value of the freedom: “this hinge is turned by 30°”. It sets the position rather
than bounding it.

The value takes a **formula** with global parameters. Hence a simple technique: tie several drives to
one variable and watch the mechanism go through its whole travel by changing a single number.
