# Project body geometry

The same as projecting edges, but aimed at **another** part: click the edges of the underlying body
or switch on “Face outline” above.

![The face outline taken from the body: the entities are driven — change the body and the outline follows.](img/sketch-project.png)

## An external reference

When one part's sketch stands on another part's geometry, an explicit **external reference** appears
between them. It is visible in the tree and it can be broken — unlike a silent dependency, which you
learn about only when something has moved.

This is top-down design: the bracket's dimensions come from the housing instead of being retyped
into it.

## What to remember

Projected geometry is **driven**: it follows its source. Move the source part in the assembly and
the outline follows — which is exactly what the link is for.
