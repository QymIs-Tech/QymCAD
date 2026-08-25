### Error messages. The core reports a CODE; these are the words for it.
### { $name } placeholders carry data from the core — keep them, they are not decoration.

## Operation failed in the geometry kernel.
## The hint in parentheses is the usual cause — it saves a support round-trip.

error-op-failed-extrude = Extrude failed
error-op-failed-extrude-profile = Extrude failed (check the profile)
error-op-failed-extrude-contour = Extrude failed (check the contour)
error-op-failed-revolve = Revolve failed
error-op-failed-revolve-profile = Revolve failed (check the profile)
error-op-failed-revolve-axis = Revolve about the datum axis failed (is the axis in the sketch plane?)
error-op-failed-sweep = Sweep failed (is the profile at the path start and roughly perpendicular to it?)
error-op-failed-loft = Loft failed (sections must be closed and consistent)
error-op-failed-loft-boolean = Loft boolean against the body failed
error-op-failed-boolean = Boolean failed
error-op-failed-body-boolean = Body boolean failed (no intersection, or the bodies are unrelated?)
error-op-failed-fillet = Fillet failed (radius too large, or the edges?)
error-op-failed-fillet-var = Variable fillet failed (radii or edges?)
error-op-failed-chamfer = Chamfer failed (size too large, or the edges?)
error-op-failed-chamfer-asym = Asymmetric chamfer failed (leg or angle too large?)
error-op-failed-shell = Shell failed (thickness or face?)
error-op-failed-shell-center = Centred shell failed (offset or face?)
error-op-failed-draft = Draft failed (can this face be drafted at this angle from this neutral?)
error-op-failed-push-face = The face will not move (curved face, or self-intersection)
error-op-failed-remove-faces = The faces cannot be removed
error-op-failed-replace-faces = The surface did not close the opening — the face will not be replaced
error-op-failed-copy-faces = The face will not copy as a separate surface
error-op-failed-stitch = The sheets will not stitch: no edge matched — they do not appear to touch
error-op-failed-trim = Trim failed: the surface and the tool do not intersect, or there is nothing to cut
error-op-failed-thicken = The face will not thicken (the offset self-intersects?)
error-op-failed-split-body = The plane does not cut the body (it misses it, or lies on a face)
error-op-failed-split-faces = The plane does not split any face (it misses the body)
error-op-failed-hole = Hole failed (diameters or depths?)
error-op-failed-holes = Holes failed (points, diameters or depths?)
error-op-failed-thread = Thread failed
error-op-failed-helix = Helical sweep failed
error-op-failed-auger = Auger failed
error-op-failed-mirror = Mirror failed
error-op-failed-mirror-plane = Mirror about the plane failed
error-op-failed-array = Pattern failed
error-op-failed-move = Move failed
error-op-failed-transform = Transform failed
error-op-failed-cylinder = Cylinder failed
error-op-failed-sphere = Sphere failed
error-op-failed-cone = Cone failed
error-op-failed-torus = Torus failed
error-op-failed-prism = Prism failed
error-op-failed-fuse-profiles = Merging the contours failed
error-op-failed-place = Placement failed

## The operation needs the real OCCT kernel (a stub answered).
## The user normally never sees these — they mean the build has no kernel.

error-kernel-required-extrude = Extrude needs the OCCT kernel
error-kernel-required-extrude-profile = Extrude needs the OCCT kernel
error-kernel-required-extrude-contour = Extrude needs the OCCT kernel
error-kernel-required-revolve = Revolve needs the OCCT kernel
error-kernel-required-revolve-profile = Revolve needs the OCCT kernel
error-kernel-required-revolve-axis = Revolve needs the OCCT kernel
error-kernel-required-sweep = Sweep needs the OCCT kernel
error-kernel-required-loft = Loft needs the OCCT kernel
error-kernel-required-loft-boolean = Loft boolean needs the OCCT kernel
error-kernel-required-boolean = Boolean needs the OCCT kernel
error-kernel-required-body-boolean = Body boolean needs the OCCT kernel
error-kernel-required-fillet = Fillet needs the OCCT kernel
error-kernel-required-fillet-var = Variable fillet needs the OCCT kernel
error-kernel-required-chamfer = Chamfer needs the OCCT kernel
error-kernel-required-chamfer-asym = Asymmetric chamfer needs the OCCT kernel
error-kernel-required-shell = Shell needs the OCCT kernel
error-kernel-required-shell-center = Centred shell needs the OCCT kernel
error-kernel-required-draft = Draft needs the OCCT kernel
error-kernel-required-push-face = Push/pull face needs the OCCT kernel
error-kernel-required-remove-faces = Removing faces needs the OCCT kernel
error-kernel-required-replace-faces = Replacing a face with a surface needs the OCCT kernel
error-kernel-required-copy-faces = Copying a face needs the OCCT kernel
error-kernel-required-thicken = Thicken needs the OCCT kernel
error-kernel-required-split-body = Split body needs the OCCT kernel
error-kernel-required-split-faces = Split faces needs the OCCT kernel
error-kernel-required-hole = Hole needs the OCCT kernel
error-kernel-required-holes = Holes need the OCCT kernel
error-kernel-required-thread = Thread needs the OCCT kernel
error-kernel-required-helix = Helical sweep needs the OCCT kernel
error-kernel-required-auger = Auger needs the OCCT kernel
error-kernel-required-mirror = Mirror needs the OCCT kernel
error-kernel-required-mirror-plane = Mirror needs the OCCT kernel
error-kernel-required-array = Pattern needs the OCCT kernel
error-kernel-required-move = Move needs the OCCT kernel
error-kernel-required-transform = Transform needs the OCCT kernel
error-kernel-required-cylinder = Cylinder needs the OCCT kernel
error-kernel-required-sphere = Sphere needs the OCCT kernel
error-kernel-required-cone = Cone needs the OCCT kernel
error-kernel-required-torus = Torus needs the OCCT kernel
error-kernel-required-prism = Prism needs the OCCT kernel
error-kernel-required-fuse-profiles = Merging contours needs the OCCT kernel
error-kernel-required-place = Placement needs the OCCT kernel

## Inputs that are missing or stale

error-source-body-not-built = The source body was not built — fix the feature above it first
error-source-part-has-no-body = The source part has no body
error-body-a-not-built = Body A was not built
error-body-b-not-built = Body B was not built
error-face-not-found = The face is no longer in the source body — the reference went stale
error-faces-not-found = The faces are no longer in the source body — the references went stale
error-profile-not-found = The sketch profile was not found
error-revolve-profile-crosses-axis = The profile crosses the axis of revolution — no CAD can build that. Push the profile against the axis (half a section: a semicircle rather than a circle), or move the axis clear of the profile.
error-sweep-profile-missing = The sweep profile was not found
error-sweep-path-missing = The sweep path was not found
error-no-isolated-points-for-holes = The sketch has no isolated points to place holes at
error-no-points-for-holes = No points to place holes at

## Reference planes

error-cut-plane-deleted = The cutting plane was deleted — pick another one or delete the split
error-mirror-plane-deleted = The mirror plane was deleted — pick another one or delete the mirror
error-split-plane-deleted = The splitting plane was deleted — pick another one or delete the operation
error-mirror-plane-unset = The mirror plane is not set — recreate the mirrored part
error-zero-normal = The plane normal is zero — no direction is defined

## Values that make no sense

error-zero-thickness = Zero thickness — there would be no plate
error-zero-push-distance = Zero distance — there is nowhere to move the face
error-broken-solid = The kernel returned an unusable solid — the operation was cancelled and the part is unchanged. This usually happens when the face borders a fillet or chamfer: try a smaller distance, or move the operation before the fillet in the timeline
error-split-piece-count = The plane now cuts the body into { $got } parts instead of { $want } — move the plane back or recreate the split
error-loft-needs-two-sections = A loft needs at least two closed sections
error-draft-needs-faces = A draft needs the faces to tilt and a neutral face
error-no-contours = No contours for the operation
error-all-edges-smooth = Every selected edge is a smooth joint (a fillet boundary) — there is nothing to round or chamfer
error-fillet-radius-too-big = Fillet R{ $radius } did not take on: { $issues }{ $smooth }
# One edge in that list. «takes up to» tells the user the largest radius that WOULD work.
error-fillet-edge-takes-up-to = edge { $edge } (takes up to { $max })
error-fillet-edge-takes-none = edge { $edge } (takes no radius at all — it runs into a tangent joint of an earlier fillet; remove this edge or round its neighbour first)
error-fillet-smooth-skipped = ; { $n } smooth joints were skipped automatically
error-fillet-edges-one-by-one = Fillet R{ $radius }: these edges only take one at a time — neighbouring fillets overlap
error-chamfer-too-big = Chamfer { $dist } mm failed — the leg is larger than the side
error-surface-does-not-close = The surface does not match the opening: { $n } edges are left unpaired. The boundaries differ — build the patch on the same edges that bound the face being replaced
error-push-face-on-sheet = A surface face cannot be pushed: this is a solid operation. To give a surface thickness, use "Thicken"
error-needs-solid-not-sheet = This is a solid tool: it does not apply to a surface. Give the surface a thickness and work with it as an ordinary body
error-draft-failed = A draft of { $angle }° will not take on these faces. Usually a thin wall is in the way: after a shell there is little left to slope — apply the draft before the shell, or use a smaller angle

## Threads and augers

error-thread-rim-not-found = The cylinder or hole rim (a circular edge) was not found
error-thread-length-unset = The thread length is not set
error-thread-pitch-too-small = Pitch { $pitch } mm is too small
error-thread-too-many-turns = { $turns } turns is too many — increase the pitch or shorten the thread
error-thread-depth-too-deep = Thread depth { $depth } mm is at or beyond the radius { $radius } mm: for Ø{ $dia } the pitch { $pitch } is too coarse
error-thread-removed-nothing = The thread removed nothing ({ $before } -> { $after } mm³) — check the chosen face, pitch and length
error-thread-failed = The thread did not build (check pitch, length and diameter)
error-auger-rim-not-found = The shaft rim (a circular edge) was not found
error-auger-bad-pitch-or-length = Auger pitch and length must both be greater than zero
error-auger-outer-not-bigger = Auger outer Ø{ $outer } is not larger than the shaft Ø{ $shaft }
error-auger-added-nothing = The auger flight added nothing ({ $before } -> { $after } mm³) — check the outer Ø and the chosen shaft
error-auger-flight-failed = The auger flight did not build (check pitch, thickness and outer diameter)

## Isolation: a part owns its geometry

error-body-only-in-part = A body can only be built inside a Part (an Assembly does not hold bodies)
error-cross-component-input = Cross-component reference is not allowed: input { $input } belongs to another component
error-sketch-on-foreign-face = Sketch input { $input } sits on a face of another component's body without an external reference
error-sketch-face-ref-lost = The sketch face reference on body { $body } was not found by name after the rebuild — the closest match was used, so check where the feature landed

## Results that are empty

error-array-empty = The pattern produced nothing
error-empty-result = The result is an empty body
error-remove-faces-failed = The faces cannot be removed: { $why }

## Assembly

error-joint-unsatisfied = The joint is not satisfied — residual { $residual } mm

## Expressions

error-expr-unknown-char = Unknown character «{ $what }»
error-expr-unknown-fn = Unknown function «{ $what }»
error-expr-unknown-name = unknown name: { $what } — there is no such parameter
error-expr-needs-one-arg = { $what }() takes one argument
error-expr-needs-two-args = { $what }() takes two arguments
error-expr-expected-paren = Expected «)»
error-expr-expected-paren-after-args = Expected «)» after the arguments
error-expr-unexpected-token = Unexpected token { $what }
error-expr-unexpected-end = the expression ends too early: a number or a name was expected
error-expr-trailing-input = Trailing input at «{ $what }»
error-expr-not-a-number = The result is not a number (division by zero?)

## A message from the kernel itself — passed through untranslated: it is diagnostics, not prose.

error-kernel-message = Kernel: { $message }

# ── GEOMETRY KERNEL BRIDGE (OCCT) ──
cad-no-faces-picked = no face is picked
cad-faces-not-in-body = the picked faces are not in this body (the reference is stale)
cad-neighbours-not-extendable = the neighbouring surfaces do not extend — a whole element is being removed (hole, boss)
cad-step-no-shapes = STEP: the bodies could not be read
cad-step-nothing-to-export = STEP: there are no bodies to export
cad-step-write-failed = STEP: writing failed (code { $v })
cad-step-read-failed = STEP: the geometry could not be read or handed over
cad-step-empty-tessellation = STEP: empty tessellation (no bodies/faces?)
cad-extrude-needs-3-points = an extrusion profile needs >=3 points
cad-extrude-failed = OCCT: the profile could not be extruded (self-intersection?)
cad-extrude-empty = the extrusion produced an empty body
cad-revolve-needs-3-points = a revolve profile needs >=3 points
cad-revolve-failed = OCCT: the revolve failed (does the profile cross the axis?)
cad-revolve-empty = the revolve produced an empty body
cad-boolean-needs-3-points = both profiles need >=3 points
cad-boolean-failed = OCCT: the boolean operation failed
cad-boolean-empty = the boolean operation produced an empty body

# ── FILE LAYER: codes come from qymcad-io, the argument is the path and the OS text ──
io-file-create = could not create { $v }
io-file-replace = could not replace { $v }
io-file-read = could not read { $v }
io-not-a-qpart = this is not a .qpart (not a zip container)
io-not-a-qcad = this is not a .qcad (not a zip container): the old format is not supported
io-refuse-empty-over-full = refused: an empty document over a non-empty file ({ $v } nodes) — save it as a new file
io-stl-no-triangles = STL: there are no triangles to export
io-stl-too-many-triangles = STL: too many triangles
io-stl-write-failed = STL: writing failed: { $v }

io-svg-empty-sketch = SVG: the sketch is empty
io-svg-write-failed = SVG: writing failed: { $v }
io-dxf-empty-sketch = DXF: the sketch is empty
io-dxf-write-failed = DXF: writing failed: { $v }
verify-axis-out-of-table = the travel leaves the table — { $v }
post-not-implemented = the post-processor is not implemented yet
error-edges-not-found = Not one of the { $asked } named edges is left in the body. Their names came from an operation higher in the timeline and it has changed — pick the edges again.
error-op-failed-patch = A surface will not span these edges
error-shell-thickness-over-round = A { $t } mm wall is thicker than the smallest round on the body ({ $r } mm): the offset consumes it entirely and the shell cannot be built. Use a wall thinner than { $r } mm or enlarge the round
error-operation-split-body = The operation split the part into { $n } bodies: a part holds exactly one body. Reduce the value or apply the operation to a different face
error-mirror-of-hollow-body = Mirroring a hollow part about its own face is beyond the kernel for now: joining the halves leaves stray shells. Mirror the part before shelling it, or pick another plane
error-shell-of-multi-shell-body = The kernel cannot shell a body made of { $n } shells: it is already hollow or assembled from copies (pattern, mirror). Shell it earlier — before patterning, mirroring or a second shell
error-shell-not-built-here = The shell could not be built on this body: the face offset fails inside the kernel. Try a different wall thickness, or shell the body earlier in the history while it is simpler
error-cut-removed-nothing = The cut removed nothing: the tool does not intersect the part. Check where the tool sits and how deep the cut goes
error-stitch-nothing-joined = Nothing to stitch: the chosen surfaces share no edges — they do not touch. After fillets, neighbouring faces are separated by a rounded band; pick surfaces that actually meet
