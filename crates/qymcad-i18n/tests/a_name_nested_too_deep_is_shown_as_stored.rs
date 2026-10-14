//! A NAME NESTED TOO DEEP IS SHOWN AS STORED, NOT UNWRAPPED UNTIL THE STACK RUNS OUT. A substitution that is a key is
//! unwrapped one level down per call; a name stored `name-mirror-of#` 20,000 times over ran a 2 MiB thread out of
//! stack, and a stack overflow ends the whole program. Real names - a mirror of a mirror - read as before.

/// The name read on a thread with a 256 KiB stack: a quarter of the smallest a name is drawn on (the 1 MiB main thread
/// on Windows; names are formatted on the interface thread), so the limit holds with the rest of that stack already in
/// use by the frame's drawing. A stack run out aborts the whole binary rather than failing here.
fn read_on_a_small_stack(stored: String) -> String {
    std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || {
            qymcad_i18n::set_language("en");
            qymcad_i18n::name(&stored)
        })
        .expect("a thread")
        .join()
        .expect("the name was read")
}

#[test]
fn a_deep_name_is_unwrapped_to_the_limit_and_the_rest_shown_as_stored() {
    let levels = 20_000;
    let shown = read_on_a_small_stack(format!("{}name-plane", "name-mirror-of#".repeat(levels)));
    let rest = format!("{}name-plane", "name-mirror-of#".repeat(levels - qymcad_i18n::NAME_DEPTH));
    assert_eq!(shown, format!("{rest}{}", " (mirror)".repeat(qymcad_i18n::NAME_DEPTH)));
}

#[test]
fn a_mirror_of_a_mirror_reads_as_before() {
    let shown = read_on_a_small_stack("name-mirror-of#name-mirror-of#name-plane".to_string());
    assert_eq!(shown, "Plane (mirror) (mirror)");
}
