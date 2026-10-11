//! A VALUE FED TO A HASHER THROUGH ITS OWN SERIALIZATION: every field the file stores, by construction, with no text
//! made (the derived serializations of the document allocate nothing on the way). The state key is computed from it for the parts of the document a hand-written list
//! kept missing (the document's title, a part's colour, a circle's radius, a sketch's notes ...): a forgotten field
//! is a lost edit.
//!
//! A number goes in as its bits, a string as its bytes, a variant as its index, a sequence with its length (a struct
//! has a fixed shape, so its fields go in without names or count). A map goes in regardless of the order
//! it iterates in - each entry is hashed on its own and the results are added - because a map equal to another, cloned
//! for undo or read back from a file, can iterate in another order, and the key must not call that an edit.
use serde::ser::{self, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Feed `value` to `h` as its serialization describes it. A value whose `Serialize` fails (none in the document)
/// feeds what it wrote before the failure.
pub(crate) fn feed<T: Serialize + ?Sized>(value: &T, h: &mut DefaultHasher) {
    let _ = value.serialize(Feed(h));
}

/// What feeding a value yields: nothing but having fed it.
struct Done;

#[derive(Debug)]
struct Never;

impl std::fmt::Display for Never {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a value could not be fed to the hasher")
    }
}

impl std::error::Error for Never {}

impl ser::Error for Never {
    fn custom<M: std::fmt::Display>(_: M) -> Self {
        Never
    }
}

struct Feed<'a>(&'a mut DefaultHasher);

/// A sequence, a tuple or a struct being fed: its parts in turn.
struct Entries<'a> {
    h: &'a mut DefaultHasher,
}

/// A map being fed: each entry hashed on its own, and the sum of them at the end, so the order does not count.
struct MapEntries<'a> {
    h: &'a mut DefaultHasher,
    /// the sum of the hashes of the entries so far
    sum: u64,
    /// the entry being fed: its key is in, its value is next
    entry: DefaultHasher,
}

impl<'a> ser::Serializer for Feed<'a> {
    type Ok = Done;
    type Error = Never;
    type SerializeSeq = Entries<'a>;
    type SerializeTuple = Entries<'a>;
    type SerializeTupleStruct = Entries<'a>;
    type SerializeTupleVariant = Entries<'a>;
    type SerializeMap = MapEntries<'a>;
    type SerializeStruct = Entries<'a>;
    type SerializeStructVariant = Entries<'a>;

    fn serialize_bool(self, v: bool) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_i8(self, v: i8) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_i16(self, v: i16) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_i32(self, v: i32) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_i64(self, v: i64) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_u8(self, v: u8) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_u16(self, v: u16) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_u32(self, v: u32) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_u64(self, v: u64) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_f32(self, v: f32) -> Result<Done, Never> {
        v.to_bits().hash(self.0);
        Ok(Done)
    }
    fn serialize_f64(self, v: f64) -> Result<Done, Never> {
        v.to_bits().hash(self.0);
        Ok(Done)
    }
    fn serialize_char(self, v: char) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_str(self, v: &str) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Done, Never> {
        v.hash(self.0);
        Ok(Done)
    }
    fn serialize_none(self) -> Result<Done, Never> {
        0u8.hash(self.0);
        Ok(Done)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Done, Never> {
        1u8.hash(self.0);
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Done, Never> {
        Ok(Done)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Done, Never> {
        Ok(Done)
    }
    fn serialize_unit_variant(self, _: &'static str, index: u32, _: &'static str) -> Result<Done, Never> {
        index.hash(self.0);
        Ok(Done)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, value: &T) -> Result<Done, Never> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(self, _: &'static str, index: u32, _: &'static str, value: &T) -> Result<Done, Never> {
        index.hash(self.0);
        value.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Entries<'a>, Never> {
        len.hash(self.0);
        Ok(Entries { h: self.0 })
    }
    fn serialize_tuple(self, _: usize) -> Result<Entries<'a>, Never> {
        Ok(Entries { h: self.0 })
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Entries<'a>, Never> {
        Ok(Entries { h: self.0 })
    }
    fn serialize_tuple_variant(self, _: &'static str, index: u32, _: &'static str, _: usize) -> Result<Entries<'a>, Never> {
        index.hash(self.0);
        Ok(Entries { h: self.0 })
    }
    fn serialize_map(self, len: Option<usize>) -> Result<MapEntries<'a>, Never> {
        len.hash(self.0);
        Ok(MapEntries { h: self.0, sum: 0, entry: DefaultHasher::new() })
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Entries<'a>, Never> {
        Ok(Entries { h: self.0 })
    }
    fn serialize_struct_variant(self, _: &'static str, index: u32, _: &'static str, _: usize) -> Result<Entries<'a>, Never> {
        index.hash(self.0);
        Ok(Entries { h: self.0 })
    }
}

impl ser::SerializeSeq for Entries<'_> {
    type Ok = Done;
    type Error = Never;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(self.h)).map(drop)
    }
    fn end(self) -> Result<Done, Never> {
        Ok(Done)
    }
}

impl ser::SerializeTuple for Entries<'_> {
    type Ok = Done;
    type Error = Never;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(self.h)).map(drop)
    }
    fn end(self) -> Result<Done, Never> {
        Ok(Done)
    }
}

impl ser::SerializeTupleStruct for Entries<'_> {
    type Ok = Done;
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(self.h)).map(drop)
    }
    fn end(self) -> Result<Done, Never> {
        Ok(Done)
    }
}

impl ser::SerializeTupleVariant for Entries<'_> {
    type Ok = Done;
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(self.h)).map(drop)
    }
    fn end(self) -> Result<Done, Never> {
        Ok(Done)
    }
}

impl ser::SerializeMap for MapEntries<'_> {
    type Ok = Done;
    type Error = Never;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Never> {
        self.entry = DefaultHasher::new();
        key.serialize(Feed(&mut self.entry)).map(drop)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(Feed(&mut self.entry))?;
        self.sum = self.sum.wrapping_add(self.entry.finish());
        Ok(())
    }
    fn end(self) -> Result<Done, Never> {
        self.sum.hash(self.h);
        Ok(Done)
    }
}

impl ser::SerializeStruct for Entries<'_> {
    type Ok = Done;
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> Result<(), Never> {
        value.serialize(Feed(self.h)).map(drop)
    }
    fn end(self) -> Result<Done, Never> {
        Ok(Done)
    }
}

impl ser::SerializeStructVariant for Entries<'_> {
    type Ok = Done;
    type Error = Never;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> Result<(), Never> {
        value.serialize(Feed(self.h)).map(drop)
    }
    fn end(self) -> Result<Done, Never> {
        Ok(Done)
    }
}
