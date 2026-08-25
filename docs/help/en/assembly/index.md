# Assembly: an overview

An assembly places parts against each other. A part stays itself: it keeps its own history, its own
sketches and its own single body — the assembly only says where it stands and how it moves.

![An assembly: the parts stand against each other the way they do in the product.](img/assembly-components.png)

## Components

The assembly tree is made of components: parts and sub-assemblies. A double click **goes inside** a
component, and from there you work with it like an ordinary part; the path on top shows where you are.

## Joints

Position is set by **joints**, rules between parts, rather than by coordinates:

- **Rigid** — the parts are tied solid, zero degrees of freedom.
- **Revolute** — one axis of rotation: a hinge, a pulley.
- **Slider** — one translation along an axis.
- **Cylindrical** — rotation and translation along the same axis: a shaft in a plain bearing.
- **Planar** — two translations in a plane and a rotation in it: a part lying on a table.
- **Ball** — three rotations about a point.
- **Pin-slot** — rotation about one axis and translation along another.
- **Parallel** — holds the direction only; where the part sits is its own business.

A joint's anchor is **inferred under the cursor**: the middle of a face gives its centre, an edge its
midpoint, a circular rim the hole centre, an edge end a vertex. You do not declare in advance what
you are about to pick.

The solver places the parts so that all joints hold at once, and says honestly how many degrees of
freedom are left and which constraints turned out to be **redundant** — that is, repeat what was
already said.

## Component arrays and mirrors

A row of bolts or a symmetric pair of brackets is placed by a component array or mirror. The copies
live in one feature: editing happens in one place.

## Interference check

A separate check shows where parts **run into each other**. On an assembly built from joints this is
the first thing worth looking at before release.

## Top-down design

One part's sketch can stand on another part's face — then an explicit **external reference** appears
between them. That is how a bracket's dimensions come from the housing instead of being retyped into
it. The reference is visible in the tree and it can be broken.
