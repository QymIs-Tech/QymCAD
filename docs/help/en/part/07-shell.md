# Shell

Key **H**. Select the body, click the face to be **removed**, set the thickness, press Enter.

![The shell hollowed the body out leaving 2 mm walls; the top face was chosen as open.](img/part-shell.png)

The body becomes hollow: material is taken out inside, the remaining walls get the given thickness,
and the picked face disappears — that is how the cavity opens.

## About the thickness

The thickness goes inwards. A thickness too large for a narrow spot leaves no material and the
operation fails — that is not a fault of the program, such a wall simply does not exist.

## Order in the timeline

A shell goes **after** the main shape but **before** the small fillets: then the fillets land on both
the outer and the inner edges. Put the shell after the fillets and the inner edges stay sharp.

## A hint

If you need an even wall not over the whole body but along an outline, look at the sketch offset: it
works with a flat outline and is sometimes simpler.
