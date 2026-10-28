//! `qymcad-mcp`: the program driven by a model rather than by a hand.
//!
//! A Model Context Protocol server. The tools are written once, against a `Workshop`: the document of a running
//! window, where every step is drawn as it is made, goes onto the person's undo stack and can be looked at
//! (`screenshot`); or, with no window listening, a document of this process built by the same kernel. Either way the
//! answer says what was built: the bodies with their volumes and boxes, the faces and edges by their persistent ids,
//! the nodes that failed and why.
//!
//! It goes through the core - the adders of `Project`, `finish_base_body` - the way the core's own checks build a part.
//! What a person does with a click (pick a face, pick an edge) a model does with an id read from `describe_body`.
pub mod bench;
pub mod channel;
pub mod link;
pub mod picture;
pub mod protocol;
pub mod tools;
pub mod workshop;

/// The value an answer is made of, and the way to write one, so a caller holds answers without naming the JSON crate.
pub use serde_json::{json, Value};

mod args;
mod reading;
