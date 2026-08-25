# Component arrays and mirrors

A row of identical parts is placed by an array, not by copying.

![Four posts at a 22 mm pitch — one feature, not four manual inserts.](img/assembly-array.png)

- **Linear array**: select the part, set the count and the direction above, the spacing at the
  geometry.
- **Circular array**: the count and the axis above, the angle at the geometry.
- **Component mirror**: select the part and point at a plane.

## One feature instead of a handful of copies

The copies live in **one node**: the count and the spacing are edited in one place, and deleting the
array removes every copy at once. Copy a part by hand twelve times and one edit becomes twelve, and
you will forget one of them.

That is also why a copy of an array is not deleted on its own: the array drives its position, and it
would come back on the next rebuild anyway. The whole array is deleted instead.

## A mirror is not a copy

A mirrored part stays tied to its source: a change to the original carries over to the reflection.
For a left-and-right pair that is exactly what is wanted.
