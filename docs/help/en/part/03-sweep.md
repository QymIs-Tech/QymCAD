# Sweep

Select the **profile** sketch, then point at the **path** sketch above and press Enter.

![A round section swept along a two-segment path.](img/part-sweep.png)

The profile travels along the path and sweeps a body: a tube, a handrail, a cable duct, a seal
around an outline.

## Two sketches, not one

The profile and the path are different sketches and usually lie in different planes: the profile
across, the path along. It is convenient to build them so that the start of the path lies in the
plane of the profile.

## What can go wrong

- **The path turns tighter than the profile allows.** If the turn radius is smaller than the profile
  size, the body intersects itself and the operation fails. Increase the radius or shrink the profile.
- **An open path** is normal; a closed one gives a ring.
