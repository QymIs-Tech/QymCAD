
/// The name table survives a save.
///
/// A descriptor inside a reference is an index into the table of the document. Failing to save it means losing
/// the meaning of every face reference; failing to rebuild the reverse index means issuing a second descriptor
/// for the same name, and the geometry built stops matching the references from the file.
#[test]
fn the_name_table_survives_a_save_and_reload() {
    use qymcad_core::names::{GeoName, Role};
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    let wall = p.intern_name(42, Role::Wall, 7);
    let cap = p.intern_name(42, Role::CapStart, 0);
    assert_ne!(wall, cap, "different names give different descriptors");

    let ron = qymcad_core::model::to_ron(&p).expect("serialisation");
    let mut back = qymcad_core::model::from_ron(&ron).expect("deserialisation");
    assert_eq!(back.names.get(wall), Some(GeoName::new(42, Role::Wall, 7)), "the name reads back under the same descriptor");
    assert_eq!(back.names.get(cap).map(|n| n.role), Some(Role::CapStart));
    // and interning the same name again returns the same descriptor, the reverse index having been rebuilt
    assert_eq!(back.intern_name(42, Role::Wall, 7), wall, "no second descriptor is issued for the same name");
    assert_eq!(back.names.len(), p.names.len(), "the table did not grow for no reason");
}
