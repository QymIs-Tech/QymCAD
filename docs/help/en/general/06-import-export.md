# Import and export

## What can be read

- **STEP** — exact geometry with faces and edges. The best way to get someone else's part: you can
  work with it as with your own, except for the history — it is not in the file.
- **STL** — a triangle mesh only. There is no shape as such in it, so precise operations are
  impossible; it is good for overall size and for printing.
- **DXF** and **SVG** — flat outlines, they land in a sketch.

## What can be written

- **Export to STEP** — exact geometry. This is what goes to a customer and to a machine.
- **Export to STL** — triangles, for printing. Here the document's **geometry accuracy** matters: it
  decides how finely the surface is divided. It travels with the file, so the same project gives the
  same STL to two different people.

## Things to remember

- Only **visible, not consumed** bodies are exported: intermediate states of a chain are not.
- An imported body lives in the timeline as its own feature: you can move, cut and drill it, but you
  cannot “go back to its sketch” — there never was one.
- To simplify an import there is the **Remove face** command: it takes away a hole or a pad that
  should not be in someone else's model.
