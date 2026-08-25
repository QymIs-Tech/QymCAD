# The program window

The window has five parts, each with its own job.

![The program window: tools and the tree on the left, the path and parameters on top, properties on the right, the viewport with the view cube in the centre.](img/window.png)

## Left — the tools

Tools of the **current workbench**, grouped by meaning: drawing a sketch, turning a sketch into a
body, refining the body. The set changes with the workbench: Sketch will not offer “Shell”, and
Assembly will not offer “Line”.

## Top — the path and the parameters

The first line is the **path through the document**: `Assembly › Part › Sketch`. It shows where you
are, and it takes you back up in one click.

The second line appears when a command is active and holds its **parameters**: extrusion height,
fillet radius, the number of copies in an array. Every field accepts a formula.

## Centre — the viewport

The model itself. The view cube in the top right corner turns the camera: a face gives the front,
top or side view, a corner gives an isometric one, an edge gives a 45° view.

## Right — the properties

What is selected, what made it and what depends on it. The panel **shows, it does not edit**:
editing goes through a command, which has a preview, formulas and Esc to cancel.

## Bottom — the status

What is happening right now, how many **degrees of freedom** the sketch has left, where the cursor is.
