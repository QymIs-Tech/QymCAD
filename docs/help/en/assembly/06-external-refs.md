# External references and top-down design

One part's sketch can stand on another part's geometry — on its face or edge. An explicit **external
reference** then appears between the parts.

## What it is for

That is how top-down work goes: the housing is defined first, and the bracket, the lid and the gasket
take their dimensions from it. Retyping those dimensions by hand means keeping a second copy of one
value and one day fixing only one of them.

## The reference is live and explicit

Projected geometry **follows** its source: move the source part and the outline goes with it, and so
does everything built on it.

And the link is not silent: it is visible in the tree and can be **broken** with one command. After
breaking, the geometry stays where it is but stops following — which is sometimes exactly what is
needed, when a part goes into production and must no longer change after the housing.

## Careful with loops

If A takes its dimensions from B and B from A, the rebuild becomes undefined. The program does not
let such links close, but the intent is better kept one-way: from the main to the subordinate.
