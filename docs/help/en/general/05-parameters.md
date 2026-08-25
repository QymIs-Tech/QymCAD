# Parameters and formulas

Every numeric field in the program takes an **expression**, not just a number: `40/2`, `w*2`,
`len+5`, `sin(30)*10`, `pi*d`.

![Project parameters: `h` is computed from `w`, and `d` from `w` and `wall`. Change one number and all of them follow.](img/params.png)

## Global parameters

The “ƒx Parameters” window in the top bar holds the named values of the project: a name, an
expression, the computed value. A parameter can refer to another one — `d = w/2`.

That name then works in any field: in a sketch dimension, in an extrusion height, in a fillet radius,
in an array spacing, in a joint angle.

## What it is for

One number the whole part depends on must live **in one place**. Sheet thickness, fastener pitch,
shaft diameter — if they are retyped into twenty fields, one day you will fix nineteen of them.

A practical habit: create the parameters before drawing and type their names straight away. That is
cheaper than coming back and replacing numbers later.

## Named sketch dimensions

A sketch dimension can be **named** and then becomes available as a parameter. That is how a skeleton
is built: the main sketch sets the overall sizes, and the parts take their dimensions from its named
ones.

## What can go wrong

A mistake in a formula does not break the document: the field keeps the previous value and says what
is wrong. Circular references (`a = b`, `b = a`) are not computed — the program says that too.

## See also

- [Sketch dimensions](sketch/11-dimensions) — where named dimensions come from.
- [Documents and templates](general/08-documents) — how to carry a set of parameters into a new project.
