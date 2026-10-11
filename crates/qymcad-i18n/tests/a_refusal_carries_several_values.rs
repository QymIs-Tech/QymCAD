//! A REFUSAL CARRIES SEVERAL VALUES: after the `#`, apart by U+001F, into `$v`, `$w`, `$x` in that order - a place in a
//! file and what stood there. A name with a `#` in it, and a key with one value, read as before.

#[test]
fn several_values_fill_the_message_in_order() {
    let prev = qymcad_i18n::language();
    qymcad_i18n::set_language("en");
    let said = qymcad_i18n::name("io-ply-not-finite-vertex#12\u{1f}684\u{1f}NaN");
    let one = qymcad_i18n::name("io-stl-read-failed#gone");
    let own = qymcad_i18n::name("bolt #3");
    qymcad_i18n::set_language(&prev);
    assert_eq!(said, "PLY: a coordinate of vertex 12 (byte 684) is not a finite number (NaN)");
    assert_eq!(one, "STL: the file cannot be read (gone)", "a key with one value reads as before");
    assert_eq!(own, "bolt #3", "a person's own name with a `#` in it is left as it is");
}
