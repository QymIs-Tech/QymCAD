# Array in a sketch

Select the entities, press **Linear** or **Circular array**, set the parameters in the top bar. While
you set them the copies are shown as a preview; **Enter** applies, **Esc** cancels.

![Two, four, six copies — one number changes, not the drawing.](img/sketch-array/)

- **Linear** — the count and the step along a direction.
- **Circular** — the count, the angle and the centre of rotation.

## An array, not copies

The copies of an array are **one thing, not a set of separate ones**. An edit of the source entity goes
through all of them: to change the step or the number of copies you redraw nothing. A double click on
any copy opens the array parameters for editing.

Hence the rule for choosing: a one-off repetition is **Copy**; a repetition that is part of the intent
(“six fastener circles around a bolt circle”) is an array.

## A sketch array or a body array

The same grid of holes can be made two ways, and they are not equal:

- **an array in the sketch** multiplies outlines, and the body is then built from all of them at once —
  a single operation;
- **an array of the body** (Part workbench) multiplies a finished feature with all of its fillets and
  threads.

For plain through holes the first is shorter; where a machined feature is being repeated, take the
second.

## The centre of a circular array

The centre is given by a click and snaps to sketch points. It helps to put a construction point or
circle at the centre beforehand: then the centre of the array and the centre of the part coincide
exactly, rather than nearly.
