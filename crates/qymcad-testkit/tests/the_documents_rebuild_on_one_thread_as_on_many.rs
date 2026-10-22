//! THE DOCUMENTS REBUILD ON ONE THREAD AS ON MANY. The rebuild works the nodes of one wave on threads of their own, and
//! OCCT shares state between threads where it is not told otherwise: two chamfers of the case of the examples built at
//! once came out one of them a hair short, its ends renamed, in 5 of 50 rounds. Every example and private sample opened
//! on every core but one, then rebuilt whole on one thread: every name stands as it stood. Live OCCT, headless; a
//! process of its own, the number of threads being the process's.
use qymcad_testkit::real_documents;

#[test]
fn every_document_rebuilt_on_one_thread_keeps_the_names_of_many() {
    let fails: Vec<String> = real_documents::documents().iter().flat_map(real_documents::one_thread_and_many).collect();
    assert!(fails.is_empty(), "{} faults:\n  {}", fails.len(), fails.join("\n  "));
}
