<!-- The page for the NEXT release is written here, before the tag is made: what was added, what was
     fixed, what changed. Short lines, one thought each. The changelog link below is generated. -->

## What changed

**Added**

* **One way every tool works.** Sketch, Part and Assembly tools share one bar and one box at the geometry: what was
  selected before the tool is taken, values are fields with formulas and drivers, Enter applies, Esc cancels.
* **Sketch:** splines, ellipses and arcs of an ellipse as real curves - trim, extend, offset, constraints and
  dimensions over any curve; fillets and chamfers at any corner, also where two lines cross, by radius, chord, arc
  length, two legs or a leg and an angle; a rectangle as one shape with its turn and centre.
* **Sweep along edges of a body**, followed through rebuilds; a sweep starts where its profile is drawn.
* **Profiles taken by a box** for an extrusion and a revolution; the line of an inner contour is taken by a click
  next to it, on the sheet and in 3D.
* **Undo inside a sketch** walks the sketch's own steps; a finished sketch is one step.
* **The status line counts the faults of the document** and leads to each one; every warning and error is said in
  words, in its colour.
* **Kazakh and Ukrainian** - the whole interface.
* **Two icon sets, Default and Flat**, switched in **Settings -> Icon Themes**; a set of one's own is added there too.
* **The panels are laid out by hand:** dragged to any edge or onto another panel, hidden and shown, in **Settings ->
  Layout**.
* **Graphics:** the program picks a card that draws, starts in a safe mode after a failed start and says so.
* **Help -> Support the author.**

**Fixed**

* Fillets and chamfers on heavy parts: the check of a value answers in about a second, edits rebuild several times
  faster.
* "Finish" goes up the tree one level after jumps by links.
* A loft takes its sections clicked in 3D; a hole keeps its recess and its shifts when edited.
* Every body of a part goes into STEP, glTF and 3MF.
* The Windows build; the macOS disk image carries a note in every language.
* About 280 more faults found by working through the program by hand.

**Changed**

* The document format changed again, with no backward compatibility: a project saved by an earlier build opens, but
  a rebuild may lay some fillets, chamfers and planes differently.
