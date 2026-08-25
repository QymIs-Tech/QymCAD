# Interference check

Shows where parts **run into each other** — that is, occupy the same volume.

![The tree reports the check: an interference inside the parts is invisible, but the line about it is red.](img/interference.png)

It is switched on by a checkbox; the check runs on the assembly in 3D and computes the volume of the
common part rather than “do the bounding boxes touch”.

## What counts as interference

Not every intersection is a mistake. A threaded joint, an interference fit and a press fit look like
intersections in the model and indeed are. What matters is the **amount**: hundredths of a millimetre
are intent, a five-millimetre overlap is an assembly error.

## When to check

Before release and after every large change. Especially after editing joints: a part that moved a
couple of millimetres is invisible to the eye, but the volume of the common part gives it away.
