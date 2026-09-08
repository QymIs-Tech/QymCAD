# Keyboard shortcuts

The full list is “Help -> Keyboard shortcuts”. Keys are reassigned there too.

![The shortcut reference: the key on the left, what it does on the right.](img/hotkeys.png)

## General

`Esc` cancels a command, `Enter` applies, `Ctrl+Enter` finishes the sketch, part or subassembly,
`Delete` removes the selection, `F2` renames the selection in the tree, `Ctrl+Z` and `Ctrl+Y` undo and
redo, `Ctrl+S` saves.

These seven **cannot be reassigned**: they are the same in every program, and they work as expected here.

`Ctrl+Enter` goes one level up: out of a sketch into the part, out of a part into the assembly, out of a
subassembly into the assembly holding it. The same as the “Done” button at the top, without taking your
hands off the keyboard. While a command or an array is open it leaves them alone — there `Enter` applies,
and you leave afterwards.

`F2` opens the name right in the tree row — a part, a subassembly, a sketch, a feature, a body or a
datum. `Enter` confirms the name, `Esc` cancels. With nothing selected, `F2` does nothing.

## Per workbench

Workbench keys are a single letter without modifiers, because the other hand is on the mouse. The
same letter means different things in different workbenches: `C` is a circle in Sketch and a chamfer
in Part. There is no confusion: only one workbench is active at a time.

Part: `E` extrude, `Q` cut, `R` revolve, `F` fillet, `C` chamfer, `H` shell, `O` hole, `M` mirror,
`B` box, `Y` cylinder, `D` datum plane, `K` sketch on a face, `I` measure, `U` reselect the
outline.

Sketch: `L` line, `R` rectangle, `C` circle, `A` arc, `P` point, `G` polygon, `O` slot, `E` ellipse,
`N` spline, `T` text, `D` dimension, `F` corner fillet, `K` trim, `M` mirror, `X` construction, `S`
select.

Assembly: `I` insert, `N` new part, `U` sub-assembly, `J` joint, `D` datum plane.

## When the cursor is in a text field

While the cursor is in a field — the extrusion depth, a name, the search box — **a letter is typed**.
That is what you want: fields take expressions like `w*2`, where a letter has to stay a letter.

To call a tool straight from a field, **hold Alt**: `Alt+U` instead of `U`.

| Where the cursor is | How to call a tool |
| --- | --- |
| in the scene, in the tree, anywhere outside a field | the bare letter: `U` |
| in a text field | `Alt` + the letter: `Alt+U` |

There is no need to click away to free the keyboard.

`Ctrl+K` opens the command search **always**, including from a field. Space does the same, but only
while the cursor is outside a field — inside one it types a space.

## Reassigning

In the reference window click the key of an action, then press the one you want. If it is already
taken in the same workbench, the program says which command has it and leaves things as they were.

Reassignments are kept in the settings and travel with the profile. “Reset” restores the factory
layout.
