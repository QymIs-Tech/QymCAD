# Part: an overview

The Part workbench turns sketches into a body and refines that body further. The work goes through a
**history timeline**: every command adds a node to it, and the whole order stays alive — go back to
any step, change a number, and everything below rebuilds.

![A part: shape, holes, fillets — in that order.](img/part-fillet/)

## A part is ONE body

The first material feature creates a body, each next one carries it further: an extrusion adds
material, a cut removes it, a fillet reshapes it. Intermediate states do not pile up as separate
bodies — you always see the result of the chain.

## Two kinds of commands

- **From a sketch**: extrude, revolve, sweep, loft. They need an outline.
- **On the body**: fillet, chamfer, shell, hole, draft, thread, arrays, mirror. They need edges,
  faces or the body itself.

Plus **primitives** — box, cylinder, sphere, cone, torus, prism: a body without a sketch, from sizes
alone.

## The command contract

The same for all of them: pick what it stands on (an outline, edges, a face), set the values in the
top bar or right at the geometry, watch the **preview**: **Enter** applies, **Esc** cancels. Until
Enter the document is unchanged.

Every numeric field takes a formula: `40/2`, `len*2`. Names come from the global parameters.

## Editing what is built

A double click on a feature in the tree reopens the same command with the same fields. Change and
apply — it rebuilds, and so does everything that depends on it.
