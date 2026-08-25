# Remove face

Click the faces of a feature — a hole, a boss, a pocket — and press Enter. The feature is taken away
and the neighbouring faces extend and meet each other.

![Before and after: the hole goes away with its face, and the neighbouring faces close up by themselves.](img/part-remove-face/)

Select all the faces of the feature: for a hole that is the cylinder (and the bottom, if it is
blind), for a boss its side and top.

## What it is for

Above all — to **simplify someone else's geometry**. A STEP file arrives with threads, tiny fillets
and manufacturing holes, and all you need is the envelope for layout. It has no history and nothing
to roll back: a detail can only be removed together with its faces.

The second use is taking away what should not be in the model: a hole for an old fastener, a pad from
a previous revision.

## When it will not work

If the neighbouring faces cannot be stretched to meet — deleting a face where half the part
converges, say — the command refuses: there is nothing to close such a gap with.

Then remove one face at a time, starting with the smallest.
