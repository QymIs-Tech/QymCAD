# The model tree

The tree on the left is the document. Everything is there: assembly components, sketches, datums,
bodies and the **history timeline** — every operation on its own line, in the order it is executed.

![The tree inside a part: sketch, extrude, fillet, hole — and the orange rollback bar at the bottom.](img/tree.png)

## What you can do right in the tree

- **Select** — a click; the selection is shown in the properties on the right.
- **Go inside** a component — a double click.
- **Rename** — from the right-click menu. Meaningful names pay off on the first complex part.
- **Hide** — the checkbox on the line. Turns a body or a sketch off without deleting it.
- **Open for editing** — a double click on a feature reopens the same command with the same fields.
- **Delete** — Del or the menu. The program asks and **lists by name** what will go with it.

## Search

The search box above the tree filters it by name. On a part with fifty features that is faster than
looking.

## The order is not decoration

The order of the lines in the timeline is the order of construction. A fillet before a cut and the
same fillet after it are different shapes. So lines can be moved, but only to where the things they
stand on already exist.
