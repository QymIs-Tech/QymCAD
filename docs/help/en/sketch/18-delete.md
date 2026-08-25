# Delete the selection

Key **Del**. Removes the selected sketch entities.

![Before and after: the circle takes its constraints with it, and the degrees of freedom grow.](img/sketch-delete/)

## What goes with them

The constraints and dimensions that were held on the deleted geometry: a constraint missing one of
its sides cannot work.

So after a delete the **degrees of freedom usually go up** — you have removed what held the shape.
The counter at the bottom shows how many were freed. If the sketch stopped being defined, define it
again before building a body on it.

## What stays

Points where other entities meet: they belong to the junction, not to one line. Only what has become
nobody's is removed.

## Careful with what already carries a body

If a feature stands on the entity you delete, the feature goes with it. The program asks first and
lists by name what else will disappear — read that list, it is the price of the action.

## A common beginner's mistake

Deleting a “spare” line in a defined sketch and finding the part has drifted. The cause is not the
deletion but the constraints of neighbouring entities that sat on that line. The degrees-of-freedom
counter shows how many were freed.
