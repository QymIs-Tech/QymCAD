//! `qymcad-core`: the headless core.
//!
//! It holds the document model, the basic geometry, the contract of the operations and the central toolpath
//! intermediate representation, the neutral layer between geometry and the post-processor. It does not depend on
//! the interface and builds and tests without one.

pub mod asm;
mod doc_file;
pub mod drivers;
pub mod errors;
pub mod expr;
pub mod feature;
pub mod feeds;
pub mod geom;
pub mod heightmap;
pub mod ir;
pub mod measure;
pub mod model;
pub mod names;
pub mod offset;
pub mod ops;
pub mod part;
pub mod refs;
pub mod solver;
pub mod subdiv;
pub mod text;
pub mod thread;
pub mod tool;

#[cfg(test)]
mod tests {
    use crate::geom::Point3;
    use crate::ir::*;

    #[test]
    fn ir_builds() {
        let mut tp = Toolpath::new(
            Units::Mm,
            OpMeta { name: "smoke".into(), op_type: "contour".into(), ..Default::default() },
        );
        tp.push(Move::SpindleOn { rpm: 12000.0, dir: SpindleDir::Cw });
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, 5.0) });
        tp.push(Move::Plunge { to: Point3::new(0.0, 0.0, -1.0), feed: 200.0 });
        tp.push(Move::Linear { to: Point3::new(10.0, 0.0, -1.0), feed: 600.0 });
        tp.push(Move::SpindleOff);

        assert_eq!(tp.moves.len(), 5);
        assert!(matches!(tp.moves[0], Move::SpindleOn { .. }));
        assert_eq!(tp.units, Units::Mm);
    }
}
