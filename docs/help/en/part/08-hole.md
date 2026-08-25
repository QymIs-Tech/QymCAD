# Hole

Key **O**. Select the body, click a face, set the diameter and the depth, press Enter.

![A through hole in the top face: the command remembers the face, not coordinates.](img/part-hole.png)

## Why not a circular cut

A hole keeps a **reference to the face**, not coordinates. So it travels with the face: change the
part higher up the history and the hole stays where it was meant to be, not where the coordinates
happened to land.

A circular cut has no such link and can drift after a change.

## Layout by points

If the sketch on the face has points, holes can be placed at them — then their position is parametric
and is edited by the sketch dimensions.

## Depth

A blind hole is given by a number, a through hole by a depth deliberately larger than the wall or by
the matching mode. For a thread use the **Thread** command on the finished hole.

## See also

- [Thread](part/10-thread) — how to cut one in a hole.
- [Linear array](part/17-linear-array) — a row of holes as one feature.
