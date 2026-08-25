# qymcad-testkit - the harness for reproducing defects

Loads a `.qcad` or a bare `document.ron` and regenerates the project with the REAL OCCT kernel (headless), so
that a defect in geometry can be debugged against an actual document instead of by guesswork.

NOTE: `src/lib.rs` holds a COPY of `OcctKernel` from the application (the block is clean, without egui).
TODO: move `OcctKernel` into `qymcad-kernel` and use it from both places, so the copy goes away.
