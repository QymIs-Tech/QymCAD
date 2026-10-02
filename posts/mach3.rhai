// posts/mach3.rhai — the Mach3 post-processor.
//
// The dialect follows the controller's own documentation; the output is accepted by diffing it against
// a reference post on one part.
//
// The values this post has to emit:
//   preamble : "G17 G54 G40 G49 G80 G90"
//   postamble: "M05" / "G17 G54 G90 G80 G40" / "M2"
//   drilling cycles: real G81/G82/G83 (translate_drill_cycles = false)
//   tool length offset G43 H after a change; coolant M7/M8/M9; arcs as IJK; comments in (parentheses)
//   axis-modal; word order: X Y Z A B C I J F S T Q R L H D P
//
// Not implemented yet.
