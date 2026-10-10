# Corner fillet and chamfer

![A sharp corner and the same one rounded: the radius becomes a dimension, the tangencies become constraints.](img/sketch-corner/)

## How to do it

- **Fillet** (key **F**): click the corner of two lines or of a line and an arc, type the radius into the field at
  the corner, **Enter**.
- **Fillet by its chord or arc length**: on the bar above press **Chord** (the straight distance between the two
  ends of the arc) or **Arc length** instead of **Radius**, then click the corner, type the value, **Enter**.
- **Chamfer**: click the corner of two lines, type the size into the field at the corner, **Enter**. The size is the
  length of the cut itself; its dimension stands on the two ends of the cut, and the cut stands back from the corner
  equally along both lines.
- **Chamfer by two values**: on the bar above press **Two distances** (**Leg 1** and **Leg 2**) or **Leg and angle**
  (**Length** and the **Angle** between that line and the cut); **Symmetric** gives one size, the length of the cut. The
  first value runs along the line you click nearer to: click the corner a little to the side of that line. Type the first
  value, **Tab** to the second, **Enter**.
- **The corner of the lines**: choose the lines (with **Shift**, if there is more than one) — the corners they make
  are cut together, by one value and one **Enter**. With two lines that share a corner already chosen, the button
  offers to take that corner off straight away.
- **Fillet every corner of the contour**: select the contour, press the button, type the **R of every corner**,
  **Enter**.
- **A corner at a crossing**: two lines going through each other make four corners. Click the fillet or the chamfer
  inside the corner you want, by the crossing - both lines are cut there, that corner is rounded or cut, and the other
  two halves stay lines. Or choose both lines with **Shift** - the preview of the corner follows the pointer from
  quarter to quarter; click in the one you want, the preview changes colour and stays there, and the field of the size
  opens beside it. One **Ctrl+Z** brings the whole lines back; so does **Esc** before **Enter**.
- **A corner at a point of several shapes**: where the corners of several shapes meet at one point, click the point on
  the side of the corner you want.

## How corners are named

A corner is not something one points at. **A corner is what the chosen lines make.** The field opens at once for the
**whole set**, and one value cuts all of it on one **Enter**, as one step of undo.

A corner exists where **two chosen lines share a point** (coincident ends). A plain crossing of two lines is not a
corner.

- a triangle of three lines is three corners; four lines of a polyline are three; two separate pairs are two.
- A line that meets none of the chosen ones adds nothing: it is half of the next corner and waits for company. The
  corners named before it stay where they are.

**The lines at one point are paired two at a time, in the order they were chosen.** A line already in a corner at that
point makes no second corner there.

- A cross of four lines: choose **A** — no corners; add **B** — **AB** appears; add **C** — **C** waits; add **D** —
  **CD** appears.
- Let **B** go — only **CD** is left and **A** waits. No corner **AC** arises: nobody asked for it.
- Let **C** go as well — **AD** is what stands. Corners are **remembered**, not rebuilt from the selection afresh on
  every click.

**A joint of two segments on one straight line (180°) is not a corner** — there is nothing to cut there. If a **third**
chosen line arrives at such a point it is **not taken**, and the status names that point: the corners at a point are
taken two at a time, and the third would have to be cut against a straight joint.

## Shift and the single selection

**Shift is the only way to choose more than one line**, as in ordinary selection.

- **A click without Shift is a single selection**: only the line under the cursor is chosen.
- A click **without Shift puts the tool in single mode, wherever it lands** — on a line, on a point, or on empty
  space. A click on a point and a click on empty space do **not** take the chosen lines away: they speak about the
  mode alone.
- **Shift + click on a line** adds it to the set (or takes it out again if it is already chosen).
- **A click on a point does not move the choice of lines at all**, with Shift or without it. A point is not one of the
  lines a corner is cut from.

## Corners named at a point

A point where **nothing stands** may name a corner of its own. While the cursor is inside the zone of that point
(three times as wide as the point is picked), a **yellow** preview of the corner a click there would name is drawn; out
of the zone it disappears. The click makes it **violet** — a corner in the set.

- Where more than two lines stand, the yellow preview shows the pair the cursor **stands between**.
- Where a corner of the lines already stands at that point, no preview is drawn and a second corner cannot be named
  there: it is one place, read twice.
- A click on a point carrying a corner **named at the point** takes it away, with Shift or without it; the point is
  free again.
- A click on a point carrying a corner **of the lines** puts that corner away (it is not deleted), and **the same**
  click brings it back.
- **A corner put away goes when one of its lines is let go**, hidden or not: it is a corner *of those two lines*.
  Choosing the line again makes the corner again, and this time it stands.
- A corner of the lines that takes the place of one named at the point **kills** it: it is one place, and there is
  one corner there.
- A corner named at a point also dies when one of its lines is let go: it is a corner of those two lines.

## Lines chosen before the mode

Lines chosen **before** the tool was taken stay chosen and become that same set: the corners they make are rounded or
chamfered exactly as if they had been chosen with **Shift** already in the mode.

## The value and the undo

One value cuts the **whole set**. It is held by the tightest corner in the set and by what the lines between the
corners spend on themselves: rounding every corner of a rectangle takes half its short side. A chord or an arc length
is kept on every corner of the set, each corner making the radius of its own angle. If one corner of the set does not
take the value, no corner is cut: the reason is written at the field — type a smaller value.

- **Enter** (or the tick) applies the whole set — one step of undo.
- **Esc** cancels.
- The value is remembered for the next corner.

## What you get

A fillet inserts an arc and adds **two tangencies** and a **radius dimension** - or, given by its chord or arc length,
a dimension of the chord or of the arc length; a chamfer adds a segment and
its dimensions, measured from the sharp corner: the legs, or a leg and the angle. The sharp corner stays as a point
the dimensions stand on. All of these are constraints: move a side — the corner rebuilds itself; change the dimension — the fillet
changes.

Fillets usually go **last**, once the contour is defined: before that they get in the way of picking corners.

## If it did not work

- The size is not taken — it is more than the corner holds (longer than a side); the reason is written at the field.
- A chord or an arc length is not taken — the arc it makes reaches past the end of a side. Type a smaller value.
- The angle of a chamfer is not taken — with that angle the cut does not meet the other line. Make the angle smaller.
- The legs of a chamfer went the other way round — click the corner nearer to the line the first value should run
  along.
- The click did not take the corner — not exactly two lines meet at that point, or two of them lie along one straight
  line (180 degrees). Click right on the vertex of the corner, or on the two lines that meet there at an angle.
- A line was not taken and the status names a point — that is a straight joint: two chosen lines lie along one
  straight line and the third would have fallen on it. Let go of the line already standing at that point, or choose
  another pair.
- A click by a crossing took no corner — click nearer the crossing, inside the corner you want.
- The wrong corner of the four at one point was taken — the cursor has to stand in that sector, and where a line was
  picked first, only the corners that line takes part in are among the answers.
- A click by the end of a fillet took the line rather than a corner — the line goes on from the arc along its tangent
  there, and there is no corner.
