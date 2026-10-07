//! THE SMALL XML READER reads what 3MF and AMF files carry, and refuses what is broken.
use qymcad_io::xml;

#[test]
fn elements_attributes_and_text_read_as_written() {
    let doc = xml::parse(
        "<?xml version=\"1.0\"?>\n<!-- a comment -->\n<!DOCTYPE model>\n<m:model unit='inch' xmlns:m=\"urn:x\">\n <m:object id=\"1\" name=\"a &amp; b &#x41;&#66;\"/>\n <coordinates><x> 1.5 </x><y><![CDATA[2<3]]></y></coordinates>\n</m:model>",
    )
    .expect("parses");
    assert_eq!(doc.name, "model", "a prefix is dropped from the name");
    assert_eq!(doc.attr("unit"), Some("inch"), "single quotes");
    let o = doc.child("object").expect("the self-closing child");
    assert_eq!(o.attr("name"), Some("a & b AB"), "named and numeric entities");
    let c = doc.child("coordinates").expect("coordinates");
    assert_eq!(c.child("x").map(|n| n.text.trim()), Some("1.5"));
    assert_eq!(c.child("y").map(|n| n.text.as_str()), Some("2<3"), "CDATA kept as it is");
    assert_eq!(doc.descendants().len(), 4, "object, coordinates, x and y - every element under the root once");
}

#[test]
fn broken_documents_are_refused() {
    for bad in ["<a><b></a>", "<a>", "<a x=1/>", "not xml", "<a/><b/>", "<a x=\"&nope;\"/>"] {
        assert_eq!(xml::parse(bad).err(), Some(xml::XmlError::Malformed), "{bad:?} was read");
    }
}
