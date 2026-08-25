# The parts library

Standard **parts** (`.qpart`) go here; they are compiled into the binary at build time.

## The layout: folders are categories

The tree of folders IS the tree of categories and subcategories. Drop a `.qpart` into a folder and it belongs
to that (sub)category. The index is assembled automatically on a scan; nothing is edited by hand.

```
parts/
  Profiles/                <- a category
    category.ron           <- [optional] display name, order, icon
    Aluminium/             <- a subcategory
      profile_2020.qpart
      profile_3030.qpart
  Fasteners/
    Bolts/
      bolt_m8.qpart
```

## The `.qpart` file

A zip bundle, like `.qcad`: `part.ron` (the manifest: name, description, tags, the parameters it exposes) plus
`document.ron` (a small project holding ONE part component) plus `meshes/*`, `faces/*` and `thumb.png` (the
preview).

## `category.ron` (optional, one per folder)

```ron
(title: "Profiles", order: 1, icon: "cube")
```

With no such file the title is the folder's name and the order is alphabetical.

## Adding to it

Draw the part in the application -> right-click the part -> "Save as a standard part" (it writes into the data
directory of the operating system) -> copy the resulting `.qpart` here, into the right category -> commit. The
next build compiles it into the binary for everyone.
