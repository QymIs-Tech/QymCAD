# The names of faces, edges and vertices

A feature refers to a face or an edge of the body before it: a fillet to its edges, a sketch to the face it stands
on, a shell to the faces it opens. A reference has to hold through an edit above it - a height changed, a sketch
redrawn, a node suppressed - and through a save and an open. That is what the names are for.

## The contract: the bridge names, an operation only says what a face is

**Every name of a face, an edge or a vertex is given by the bridge**, by its own rules, in one place:
`naming.rs` of this crate. No operation, no tool of a workbench and no code of the document invents a name.

* An operation hands the bridge **roles**: "this face is the wall swept from sketch entity E", "this face is the
  surface of the rounding of edge R" (`door::Roles`). That is what only the operation knows. It hands no names and
  no numbers that stand for names.
* Its result lands through **one door**, `qym_result` (`occt_bridge.cpp`) on the C++ side and `naming::silent` on the
  Rust side: the bridge carries the names of the operands through the history of the algorithm, names the roles,
  and names what the history was silent about, by the rules below.
* The document holds the names in its table (`qymcad_core::names::NameTable`) and resolves references by them. It
  makes none of its own: it only takes into its table names the bridge gave - a vertex when it is first referenced, the
  names of a part copied in from another document.

The guard is `names_are_minted_in_known_places` (`qymcad-core/tests`): it reads every source of every crate, and a call
that creates a name anywhere but the functions it lists turns the build red. A new operation goes through the door; it
does not get a place on the list.

## The principles the bridge follows

1. **A name comes from the recipe and from the history of the operation**, never from the order of a traversal. The
   place of an element is the last resort, and it is never silent.
2. **The same document gives the same names**: two rebuilds in a row rename nothing.
3. **An edit renames only what it touched**: nothing above the node edited, nothing beside what it changed.
4. **The history of an operation is part of what it gives back**: what became of each element. Guessing it after the
   operation is not allowed, except in a named place with its reason.
5. **When two elements become one, the operand the operation works on keeps its name** (the base, not the tool);
   between names of one rank the smaller descriptor wins; the loser is recorded as absorbed.
6. **Every fallback is visible**: a reference found by place marks its step in yellow until the person accepts it.

## What a name is

A structure in the name table, written to the file; outwards it travels as a descriptor, an index into the table with
a tag in the high bits.

* **A face** (`GeoName`): the node that made it, its role (a cap, a wall, a blend, a seam, ...), its source and, when
  one source gave several faces, what cut the piece off (`CutOff`) and its number among the pieces cut off alike. The
  source is what the person gave the node: the sketch entity a
  wall was swept from, the contour picked for a region (its caps), the place in the pick of the edge a blend rounds -
  never the name of an element above, which changes when its own neighbours do (`ops::profile::name_sides`,
  `ops::blend::blend_roles`, `a_blend_is_named_after_the_edge_it_was_picked_on`); a corner patch, after the end of the edge of the lowest place
  among those meeting at its vertex (`a_corner_patch_is_named_after_its_vertex`). Two picks on one edge - two edges
  an edit above merged into one - give its surface, and a corner where picked edges meet, to the pick holding its edge
  by its name before one translated onto it, then to the lower place. An edge a description gave (every edge of a
  face) has no place in a pick, and its blend is named after the edge.
* **An edge** (`EdgeName`): the pair of faces it runs between. An edge has no recipe of its own; it is where two
  surfaces meet. The one edge of its pair is named by the pair alone: the faces at its ends change with any edit beside
  it (told by them, 901 edges of the examples that did not move were renamed over the matrix of edits). Several edges of
  one pair - the upper and the lower edge of a slot, the pieces of an edge a row of notches cut - are told apart by the
  two smallest named faces at their ends, apart from their own two (`naming::ends_of`), never by their place in
  an order of the pair: numbered within the pair as the first number free, an edit that took one of them away
  renumbered the others, and a chamfer took the edge beside its own (`an_edge_keeps_its_name_when_its_twin_goes`). Only edges of one pair with the same ends carry a number,
  by where they stand (`twins_with_the_same_ends_are_numbered_by_where_they_stand`); a number a neighbour wears through
  the history passes to the next. A name carried through the history that names a face gone from the body - the
  two sides of a strip taken out, both "beside the strip" - is named again after the pair the edge runs between, the old
  name recorded as yielded to the new one (`naming::edges`, `an_edge_named_beside_a_face_gone_takes_the_name_of_its_pair`).
  A name of another pair whose faces are all in the body stays; a reference reads it through its pair.
  **An edge with one face on both sides** - the seam where a round face closes, an edge lying inside a face - is numbered
  by where it comes from and where it stands (`naming::edges_on_their_face`): the seam
  where the surface closes along U from 0 (a revolution closes there, at the plane its profile starts on; a cylinder at
  the start of its curve), along V from 1000, an edge inside the face from 2000; several of one kind in the order of
  where they stand on the face (along the seam, then across it). Never by the order the kernel gives the edges in,
  never as the first number free. A number a seam takes from an edge of another pair wearing it through the history is
  that edge's to yield: it is named by its own pair (`a_seam_is_numbered_by_where_it_comes_from`,
  `a_seam_keeps_its_name_whatever_order_the_edits_come_in`).
  **Where the seam stands** on a round face an extrusion makes of a circle the lines of its sketch cut into arcs: in the
  middle of the arc whose wall takes the smallest name - the order the model numbers walls in (the region, the sketch
  entity, the piece) - never on a point where two arcs met, never by length or by the order the kernel hands the edges
  in (`seams_inside_arcs`, `occt_bridge.cpp`). Those points are where the other lines touch the circle and where the
  walls of later steps come to it; a seam on one ran straight on from the end of an edge there, and a rounding of that
  edge was refused at any radius. A whole circle no line cut keeps its seam where its curve starts. An edit moving the
  points moves the seam within its arc
  (`the_seam_of_the_post_stands_inside_an_arc_and_the_edge_at_a_joint_is_rounded`,
  `the_rounding_of_the_vacuum_cleaner_takes_all_eight_edges_after_an_edit`).
* **A vertex** (`VertexName`): the three smallest descriptors of the edges meeting at it.

## How the bridge names a result

In `qym_result`, in this order:

1. **The names of the operands are carried through the history** (`carry_ids_with`): `Modified`, `Generated`, removed.
   The body worked on gives every name; a tool gives only the names its roles made.
2. **The roles are named** (`naming::land_roles`).
3. **Two elements became one** - coplanar faces glued into the monolith, the two sides of a strip taken out, an edge of
   the base and of the tool lying together (`merge_names`), the bindings of a profile's edges to its walls
   (`carry_binding`), by one rule in every operation (`takes_the_name`): a name of the document
   beats a role of the operation, a role beats a positional number, and of one rank the smaller descriptor wins
   (`a_merged_wall_keeps_the_name_of_the_base`). The order of a walk never decides. A name another element already
   wears - its source went on elsewhere too - is not in the draw: one name, one element.
4. **A face split in pieces** (`naming::pieces`) keeps the name of the whole on one
   piece: the first in the order of the recipe - the faces the node made beside it, then the faces of the node the split
   face comes from beside it. The faces of other nodes beside a piece do not take part: an edit brought the walls of
   another window beside the halves of a wall of the filter, and told by them the name of the whole went to the other
   half (`the_whole_stays_on_its_half_whatever_borders_it`).
   Every other piece is named after **what cut it off**: the face of the node beside it first in the order of its
   recipe, and the side of that face it lies on (`qym_shape_piece_sides`: the normal of that face at the middle of an
   edge between them, against the line to the centre of the piece) - and numbered among the pieces cut off by the same
   face on the same side. Neither depends on how many other pieces there are: an edit that takes one piece away renames
   no other (`a_piece_keeps_its_name_when_another_piece_goes`). A piece beside a piece of another face the same
   operation split, still under a number, is beside that face: the lower halves of the walls of a window through a tube
   border only one another. Pieces the recipe cannot tell apart - those two halves - come in the order of what cut them
   off and of where they stand, never of the number the kernel gave them
   (`the_halves_of_a_wall_are_named_by_where_they_stand`). A piece no face of the node stands beside is numbered among
   such pieces by the same order. **A piece the history does not tell** - a face with no name on one surface of the
   kernel with named faces all of one face, that face among them - is a piece of it (`naming::untold_pieces`): the pieces of a split face keep its surface, and on the filter the history of a
   widened window told only the upper half of a wall (`a_piece_without_history_is_a_piece_of_its_surface`).
5. **What the history was silent about** (`naming::silent`), read of the body in one walk before any face takes its
   name (`qym_shape_naming_facts`, `naming::Facts`) and through what each face has become since: the seams, in waves, after a named neighbour
   (`naming::seams`); a face with no history and no named neighbour - a body brought in from a file or made of a mesh -
   by where it lies (`naming::placed`); the edges, after their pair of faces (`naming::edges`); the vertices after
   their edges (`naming::name_edge_ends`).

The caps of a profile operation of several regions are named after the profile they are drawn from - the contour the
person picked; a face merged from several profiles takes the caps of the lowest of them (`ops::profile::name_sides`,
`caps_of_merged_regions_are_named_by_their_profiles`).

**Where an algorithm hides its history**, the bridge reads it where the algorithm keeps it: the turn of the seam of a
round face through the record of the cutting asked of what it rebuilt (`QymRepairAsk::Apply`), then the modifier of
the turn; the simple offset of a sheet through a modifier the bridge runs itself (`SimpleOffset`), the algorithm that
wraps the same modifier answering none; the edges of a patch through what the filling generated from each boundary
edge. No operation names by place but the bodies with no history at all - a body brought in from a file, a body made
of a mesh (`no_operation_names_by_place_but_a_body_with_no_history`).

### What the bridge relies on in OCCT

Each behaviour of OCCT the names stand on is held by a check that turns red when a version of OCCT changes it:

| what the bridge relies on | the check |
| --- | --- |
| the record of `ShapeUpgrade_ShapeDivideClosed`, asked by `Apply`, gives every rebuilt element its image | `a_seam_on_the_end_of_an_edge` |
| `BRepTools_Modifier` with `BRepOffset_SimpleOffset` answers the image of each face; a reversed face comes forward | `an_offset_surface_moves_its_faces` |
| `BRepOffsetAPI_MakeFilling` generates a patch edge from each boundary edge | `every_operation_names_what_it_makes` (the patch) |
| a fault inside OCCT comes back as an exception, not the end of the program | `a_fault_of_occt_is_a_refusal` |

## How a reference finds its element

The document, not the bridge, resolves a reference, in this order (`qymcad_core::refs::resolve`,
`Project::resolve_face_id`, `Project::resolve_edge_ids_in`):

1. **The name**, as it is.
2. **A recorded merge**: the name it yielded to; a piece returns to its whole, and the edge of the wholes follows the
   name it yielded to in turn (`a_reference_to_a_piece_follows_the_name_its_whole_yielded`) - but only while the face
   is whole again: with another piece of it alive - numbered or named after what cut it off - the whole is the piece
   beside it, and the reference goes on to the next steps (`NameTable::absorbed_target_among`,
   `a_piece_beside_the_whole_is_not_the_whole`, `a_piece_named_after_its_cut_beside_the_whole_is_not_the_whole`).
3. **The canonical pair** of an edge: its faces reduced to their wholes, when one edge has that pair - or, of several,
   one edge has the same faces at its ends, reduced the same way.
4. **The structure of the name** (`NameTable::same_recipe`): the one live face of the same node, role and source, or
   the one edge between two such faces; two candidates give none.
5. **The place**: only the one element standing exactly where the reference saw it, to 1e-6 (`PLACE_EXACT`). The
   step is then marked (`Project::placed_refs`): yellow, in words, in the file, until the person accepts it in its
   properties or picks the element again (`a_reference_found_by_place_waits_for_the_person`).
6. **The other piece**, where no place was recorded (`NameTable::other_piece`): of the elements of the same recipe, the
   one that is a piece where the reference is a piece and whole where it is whole - of two pieces of a face, one keeps
   the name of the whole, and a reference to the other is the other
   (`a_reference_to_the_other_piece_goes_to_the_other_piece`).

A reference not found is said: the node goes red, or - a blend that took the rest - yellow with how many edges were
left out. A node deleted hands its consumers to the body below it, and their references are translated to the one
element standing exactly where theirs stood (`Project::geom_name_map`).

## The measure

`names_hold_on_real_documents` opens every example and private sample as the window opens it, rebuilds it twice with
the same names, edits its first number and undoes it: no reference lost by name, none found by place, no face that did
not move renamed, every name back as it opened.
