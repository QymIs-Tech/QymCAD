# The history timeline and rollback

Every operation is a node of the timeline. A model is not a picture but a **recipe**: the program
walks the timeline top down and builds the result again.

![The history timeline: every operation on its own line, in execution order. The orange bar at the bottom is the rollback.](img/tree.png)

## Everything else follows from that

- Go back to any step, change a number, and it rebuilds along with everything below.
- An edit to a sketch at the bottom of the timeline reaches the very last feature.
- A broken step is visible on its own line, not “somewhere in the model”.

## Rollback

The rollback bar temporarily stops the build at a chosen step: nothing below is built. That is how
you look at what the body was halfway through, and how a new operation is inserted into the middle.

## Suppression

A suppressed feature is skipped but stays in the timeline. A modifier does **not break the chain**
that way: suppress a fillet and the features below build on the body as it was before it. That is
handy for variants and for finding the culprit when something fails.

## When a feature fails

Its line is marked and the reason appears in the status — as a code, not a guess: “the source body
was not built”, “the cutting plane was deleted”, “the radius is too large for these edges”. Fix the
named reason instead of rebuilding everything.

## See also

- [The model tree](general/02-tree) — where that timeline lives.
- [Parameters and formulas](general/05-parameters) — what to change so everything rebuilds at once.
