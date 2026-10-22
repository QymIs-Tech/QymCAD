/// A tool of a sketch - a drawing tool, the kind of a dimension - its name in a report, and the clicks it is used with.
pub(super) struct Drawing<T> {
    pub(super) tool: T,
    pub(super) name: &'static str,
    pub(super) clicks: &'static [(f64, f64)],
}

pub(super) fn drawing<T>(tool: T, name: &'static str, clicks: &'static [(f64, f64)]) -> Drawing<T> {
    Drawing { tool, name, clicks }
}

/// THE DIMENSIONS OF THE SKETCH OF EVERY TOOL: a length between two picks, the radius of the circle, the angle of two
/// lines.
pub(super) const DIMS: [Drawing<qymcad_ui_state::DimKind>; 3] = [
    Drawing { tool: qymcad_ui_state::DimKind::Linear, name: "linear", clicks: &[(30.0, 0.0), (50.0, 15.0)] },
    Drawing { tool: qymcad_ui_state::DimKind::Radial, name: "radial", clicks: &[(70.0, 10.0)] },
    Drawing { tool: qymcad_ui_state::DimKind::Angular, name: "angular", clicks: &[(0.0, 0.0), (30.0, 0.0), (50.0, 15.0)] },
];
