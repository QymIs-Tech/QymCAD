# Parts library

Ready-made parts — motors, boards, profiles, gearboxes — are inserted from the library rather than
rebuilt in every project.

![The library window: categories on the left, items in a list, search on top.](img/library.png)

## How to insert

The **I** key (“Insert component”) puts a part into the current assembly: either an item from the
library or a STEP or STL file from the disk. It goes **into the active assembly** — if a part is
active there is nowhere to insert, and the program says so.

The library window opens from the panel and lists items by category; search and a refresh button are
there too.

## Your own parts

Any part or sub-assembly you have built can go into the library: **Save as a part** — a name and a
category. From then on it sits next to the built-in ones and is inserted the same way.

Your own items live in the operating system’s user directory, not in the project: they outlive both the
project and a reinstall of the program.

The root of a project is not saved as an item — you save a part or a sub-assembly, not the whole
document.

## An inserted item is an ordinary component

It lives in the assembly like any other part: move it, mate it, put it in an array. A copy is brought
into the document, so a later edit of the library original does not reach projects already assembled —
and that is deliberate: someone else’s part should not change under your hands.

## If an item is not found

The list is read from the disk when the window opens. If an item file was renamed or deleted, the
program says so at the moment of inserting rather than leaving an empty space.
