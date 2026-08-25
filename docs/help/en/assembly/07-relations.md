# Relations

A relation ties **two degrees of freedom together with a constant factor**. It holds nothing and
places nothing: the parts move exactly as before, but now they move together, in a fixed proportion.

![The leading arm turns 20°, the driven one 40°: a gear relation with a ratio of 2 sits between their mates.](img/assembly-relation/)

That is how drives are assembled: turn the pinion and the rack travels; turn the screw and the nut
runs along it; drive one axis and the second one covers half the distance.

## How to add one

Press **Relation** in the assembly bar, click the joints in the mate list and press **Enter**.

You pick finished joints, not geometry: a relation works on top of what is already there. If the
joint you picked has no suitable degree, the program says so at once — “this joint has no free
rotation” or “no free travel”.

## Kinds

| Kind | What it ties | Joints needed |
|---|---|---|
| **Gear** | rotation to rotation | two |
| **Rack and pinion** | rotation to travel | two |
| **Screw** | rotation to travel of the same joint | one |
| **Linear** | travel to travel | two |

The screw relation stands apart: it lives inside a single cylindrical joint and turns it into a
thread — rotation and travel stop being independent.

For rack and pinion the order of the sides is fixed: rotation first, travel second. That is why the
value is called **“travel per turn, mm”** — how many millimetres the rack covers per full turn of
the pinion.

## Value and direction

- **Ratio** — the factor for gear and linear: 2 means “the second one runs twice as fast”.
- **Travel per turn, mm** — for rack and pinion and for screw: the thread pitch or the gear module.
- **Reverse** — flips the direction of the second degree: gears in external mesh turn towards each
  other, in internal mesh the same way.

## When a relation does not act

The relation row in the mate list shows its state:

- **“Joint lost — the relation does not act”** — one of the tied joints was deleted. The relation
  has nothing to hold on to; add the joint back or delete the relation.
- **“Degree of the wrong kind: this relation expects another”** — a gear was handed a slider that
  has no rotation at all, for instance. Change the kind of the relation or pick another joint.

## See also

- [Joints](assembly/02-joints) — what relations work on top of.
- [Joint limits and drives](assembly/03-limits-drives) — how to drive motion and keep it in bounds.
