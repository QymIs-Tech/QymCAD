# The fonts compiled into the program

## LiberationSans-Bold.ttf

A bold face for the labels that must stay readable over geometry: the faces of the ViewCube, the X/Y/Z axes,
the dimension labels. The egui set carries no bold face at all - a proportional one and a monospaced one -
and thin lettering on a light face of the cube is simply lost.

**Why Liberation Sans in particular:** the SIL Open Font License (the file `LiberationSans-LICENSE.txt`)
allows shipping it with the program; it covers Latin and Cyrillic, which is both interface languages at once;
and 404 KB is acceptable to compile into the binary.

Boldness used to be faked by drawing the same text several times with an offset. That works, but it costs
five draw calls per label and at a small size it turns the letters into mush.
