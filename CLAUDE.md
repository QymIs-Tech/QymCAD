# QymCAD

The source of truth is the code and a run of it, not a description of it.

## Texts

* Comments and check messages are in English.
* A comment says what the code does and why it does it that way, with measured numbers. No retelling of a
  conversation, no development history, no internal work codes, no references to plans or other work
  documents, no names of other products. A complaint is written as `Reported behaviour: …`, without a name.
* ASCII in comments: `->`, `mm^2`, `deg`, `x`, `pi` — not `→`, `мм²`, `°`, `×`, `π`.
* Interface strings only through the `i18n/` catalogue, never hard-coded.
* Interface tone: a command is a verb in the infinitive ("Extrude"); a hint addresses the reader politely and
  impersonally ("Select a contour"). No shouting in capitals. The table is in `i18n/README.md`.
* Arrows only as ASCII `->`. A raw Unicode symbol is drawn as a box.
* Icons in egui only from `ph::*`. Unicode is fine in Markdown.
* Help says what to press, what happens, and what to do if it did not work. No internal words ("persistent",
  "kernel"), no defending decisions, no development history.

Guards: `comment_ratchet.rs`, `i18n/tests.rs`, `i18n/ratchet.rs`, `gui/font_coverage.rs`, `gui/help_voice.rs`.

## Tests

* Every fix comes with a test in the same layer the fault lives in.
* The test is red before the fix. A test green before the fix is blind.
* A new feature comes with its own checks, not "later": a check in the layer of the logic (core —
  `qymcad-testkit` or `qymcad-core/tests`), an acceptance check by hand through `Session`, for a tool — its
  contract in `qymcad-acceptance/src/tools`, and a step in `user_case`.
* A red check is a fault and is fixed before the commit. There is no list of "known red": `tools/gate.py`
  stops on any red.
* The interface is driven only through `Hand` (`gui/hand.rs`), never by poking `App` fields.
* Besides unit tests there is an end-to-end run by hand — `gui/user_case.rs`: a chain of actions (a sketch on
  a face of another part -> holes -> shell -> a second part -> assembly -> a dimension edited in the middle)
  with `check_all` after EVERY step. A chain breaks where a single action does not: a new tool is added as a
  step of the scenario, not only as its own test.
* The mouse through a real frame: `ctx.run` + `PointerMoved`/`PointerButton`. Calling a handler directly
  skips the frame's parsing and lies.
* The result is looked at as a picture: `App::rasterize_3d` + `color_image_to_png`.
* A reported fault is reproduced through the reporter's own path, on their document, starting from the click.
  An extra call the live window does not make is a fault found, not a crutch for the measurement.
* The core and associativity — matrices in `crates/qymcad-testkit` (live OCCT, headless) and
  `qymcad-core/tests`: tool x case, failures accumulate and are reported together.
* Before a commit: `cargo test --workspace` AND `cargo check --workspace --all-targets`; read the exit code of
  cargo, not of the last command in a pipeline. One is not enough: `cargo test` builds the binary only as a
  test build, and a fault absent under `cfg(test)` passes a green run.
* Heavy runs under a memory cap: `systemd-run --user --scope -p MemoryMax=16G -p MemorySwapMax=0 …`, the
  acceptance set with `--test-threads=6`.
* Samples that cannot be distributed live outside the tree; a check that needs one says `PASSED OVER` and
  returns when it is absent (`qymcad_acceptance::private_sample`).

## Code

* An operation that changes the topology of a project is one core method under test. The interface calls the
  delegate and does not inline the logic. After it, always `resync_after_topology_change`, not
  `regenerate_all`.
* A part is one body. The named exception: the pieces of "Split body" and of a cut that went right through
  the body are several bodies of one part, and booleans between them work there; a piece becomes a part
  only by the explicit "Make Part" command. An operation is one node of the timeline (`profiles: Vec<Id>`).
* No migrations and no backward compatibility: a format is changed directly, the old shape goes into
  defaults.
* Zero warnings (`dead_code`/`unused_must_use` are `deny` in the manifest). When `dead_code` goes red, first
  check whether a path from the interface was forgotten. `#[allow]` only with the reason named beside it.
* OCCT is not thread-safe: low-level booleans under the lock, probes on a copy (`BRepBuilderAPI_Copy`), not
  through `SetNonDestructive`.

**Nothing is added to `App` — no fields, no methods, no logic.** Not for a new feature, not for a bug fix,
not "one line". New code is a free function in the crate whose task it is, taking a narrow context
(`PartCtx`, `SketchCtx`, `TreeCtx`, `WinCtx`, `BarCtx`); at most ONE line of call stays in `impl App`.
Derived data is computed on demand, not stored in a field.

The `god_object_ratchet` guard holds the number of lines in `impl App` by EQUALITY. Growth is a regression,
and it is not paid for by raising the mark: it is paid by moving out what does not belong there, and the
mark goes down in the same commit.

## Workbenches, modules, resources

Three different words; the sign is who sees it.

* **A workbench = an interface plus what that interface calls.** Its own buttons, panels, viewport,
  commands. A separate crate for each.
* **A module = a task.** A crate with code solving a task: the kernel, the constraint solver, geometry,
  toolpaths, input/output. No interface strings, no switch. Sketch and Part both call the solver — the solver
  is a module, they are workbenches.
* **A resource** — a file a person brings and chooses without rebuilding the program: a post-processor
  script, a palette, a template, a library part, a dictionary, a panel layout. Built-in ones through
  `include_dir`, the user's in `ProjectDirs`.

**The DOCUMENT is not split up; the code is.** Sketches, the timeline, components, mates and parameters live
in one crate and are tied by associativity: a Part feature refers to a Sketch, an Assembly to Parts, and
undo, saving and rebuilding pass through all three. Splitting that into three crates is impossible — a
circular dependency at the first reference.

| | CAD workbenches (Sketch, Part, Assembly) | Add-on workbenches (CAM, printer) |
| --- | --- | --- |
| separate crate | yes | yes |
| own section of the document | **no** | yes (`workbenches/<code>.ron`) |
| switch in the settings | **no**, always on | yes |
| own dictionary and help | yes | yes |

Sketch has no switch for the same reason it has no section of its own in the document: a CAD where Sketch
can be switched off is a document that opens and cannot be edited. The mechanism is one for every workbench;
the policy differs. An add-on has a guard: switched off -> not one of its words on screen.

**The interface shell is separate from the content.** The panel layout is shared: tool bar, button panel,
tree, properties, viewport. The shell knows KINDS OF CONTAINERS, not a list of panels; a workbench registers
its panels into places and fills them with a call (`&mut Ui` bounded by an area), not with a description.
**The name of a workbench never appears in the shell crate** — checked by search.

Rules:

1. A module does not depend on a workbench.
2. A workbench does not depend on a workbench: what is shared moves into a module.
3. A shared module is created at the SECOND caller. Shared code designed from one example is designed wrong.
4. `&mut App` does not cross a function boundary. A panel gets a narrow context and pushes a typed
   operation; the document changes through one door. Derived data is computed on demand and remembered,
   not stored in a field.

Breaking 1 or 2 is a build that does not build.

## Tools (Sketch / Part / Assembly / CAM)

Every tool goes through the `feat_cmd` + `cmd_params` + `feat_cmd_bar` + `feat_cmd_popup` + `apply_feat_cmd`
frame. The model is the sketcher. The rules are the same for every workbench: tools are uniform.

1. A tool is taken -> the top options bar.
2. What was selected BEFORE the command is taken at once: the tool lands in hand with that selection and
   opens the value field. Nothing selected — the tool in hand waits for the person to point at what to take.
   A button with nothing selected is never silent and does nothing "by default". Applies to every tool a
   selection fits (rounding all corners, chamfer, offset, mirror, patterns, part fillets and so on).
3. Values are expression fields at the geometry, not in the right panel.
4. References (plane, axis, face) are a click-pick in the viewport with highlighting, not a combo box.
5. Live preview before Enter.
6. Enter applies, Esc cancels.
7. A double click in the tree reopens the command.

Forbidden as the main path: a button that creates with hard-coded values, editing only in the properties.
The right panel adds to a created feature; it is not the way to set up a tool.

## Working

* One branch per task. Inside a branch: a commit per meaningful piece; a branch may be rewritten until it is
  merged. `main` only grows: merges fast-forward only, zero merge commits; a mistake on `main` is fixed by a
  new commit, never by rewriting.
* A commit message, in English: a conventional subject (`fix(scope): …`), then the body — what it was, why,
  what proves the fix. Dry, facts only, no pronouns, the same style as comments.
* A trial edit is taken back by reversing the same edit, not `git checkout -- <file>`: that also wipes
  uncommitted work of your own.
* Run the GUI only as `HOME=<sandbox> cargo run -p qymcad` — never on a real user's data.
* Dependency versions are not raised silently. The dependency guard only SHOWS the gap; raising is a separate,
  deliberate decision.
* Formatting: the tree has not been run through rustfmt yet and is laid out by hand. Do not run `cargo fmt`
  on files you are not changing; the whole tree is reformatted once, in a dedicated commit.
