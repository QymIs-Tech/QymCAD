# Thread

Select the body, click a **cylinder** (an outer thread) or a **hole** (an inner one), set the pitch,
the length, the profile angle and the profile depth, press Enter.

![An M30×3.5 thread over 40 mm: a real cut profile, not a drawn spiral.](img/part-thread.png)

## The thread is real, not a symbol

A true helical surface is built: it is visible in a section, it goes into STEP and STL, and a pair
can be checked for screwing together. The price is triangles and rebuild time — which is why threads
are usually added last.

## What is set

- **Pitch** — the distance between turns.
- **Length** — how far the thread runs along the cylinder.
- **Profile angle** — 60° for a metric thread, other values for trapezoidal and the rest.
- **Profile depth** — how deep it cuts.

## A hint

Too much depth on a small diameter cuts the core away: the program refuses to build it, and rightly
so — such a thread does not exist.

## The auger is the same command, another button

The **Auger** button in the bar builds a **helical flight on a shaft** — the kind that is welded on,
not a groove that is cut out. Hence the difference: a thread REMOVES material, an auger ADDS it.

You set the outer diameter, the pitch, the length, the thickness of the flight and the edge fillet —
all at the geometry.

The outer diameter must be larger than the shaft: a flight that does not stand out of the shaft adds
nothing, and the program says so instead of building emptiness.

That is how conveyors, extruders and feed screws are made.
