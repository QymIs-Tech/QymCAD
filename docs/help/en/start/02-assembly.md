# The second lesson: an assembly

The first lesson had one part. Here there will be several, and they will stand against each other
the way they do in a real product — not “placed nearby” but **constrained**.

![An assembly of two parts: each with its own colour and its own place.](img/assembly-components.png)

Read [the first lesson](start/01-first-part) first: this one assumes you have already built a part.

## A component is a part inside an assembly

A new document is already an assembly: the root node of the tree is one. Press **N** (“New part”)
and a component appears inside it; the path on top shows that you are now **inside** it:
`Assembly › Part 1`.

Everything you build goes into that part, not into the assembly. This matters more than it looks: a
body built by accident in the assembly root can afterwards be neither moved nor patterned.

Build something simple — say a 60×40×10 plate with a hole. Click the assembly root in the tree to
step back out.

## The second part, and how to get one

Press **N** again and build a second one — a 20×20×50 post, for instance.

The other way is **I** (“Insert”) — it takes a finished document or a library item. That is how
bought parts arrive: bearings, fasteners, extrusion.

## A joint is a rule, not a move

Press **J**, click a face on one part, then a face on the other. The parts snap together.

And here is the difference from simply dragging: a joint is a **rule** that keeps holding. Change
the thickness of the plate and the post stays standing on it instead of hanging in the air. It works
the other way too: while the rule exists, the part cannot be dragged where the rule would break.

There are seven kinds of joint, and they differ in **what they leave free**:

| Kind | What stays free |
|---|---|
| Rigid | nothing |
| Revolute | rotation about an axis |
| Slider | motion along an axis |
| Cylindrical | rotation and motion along one axis |
| Planar | two motions in a plane and rotation in it |
| Ball | three rotations about a point |
| Pin-slot | rotation about one axis and motion along another |

More in [Joints](assembly/02-joints).

## Degrees of freedom here too

The counter at the bottom shows how many degrees of freedom the assembly has left. Zero means
everything is fixed. More than zero means something can still move, and that is normal: a mechanism
is supposed to move. What is not normal is a part being free when you thought you had pinned it.

The solver can also point out **redundant** constraints: two of them holding the same thing. That is
not an error, but it hides the intent — which of the two is the one that matters?

## Arrays and mirrors at the assembly level

Eight bolts around a circle are not placed one at a time. A [component array](assembly/04-arrays)
patterns a part together with its joints.

## Interference check

The [interference check](assembly/05-interference) finds places where parts have grown into each
other. On screen that is often invisible; on the machine it becomes visible at once.

## What next

- [External references and top-down design](assembly/06-external-refs) — how to build a part **in
  place**, off the geometry of its neighbour.
- [Joint limits and drives](assembly/03-limits-drives) — so a mechanism moves within given bounds.
- [Parameters and formulas](general/05-parameters) — one number for the whole product.
