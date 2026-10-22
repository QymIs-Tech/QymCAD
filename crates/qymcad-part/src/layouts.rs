//! THE LAYOUTS OF THE BOXES OF THE PART, where a box holds more than one piece of sense: the directions of a pattern, a
//! hole and its recess, a thread and its ends, the coordinates of a datum, the offset and the tilts of a section. A box of
//! one piece - a fillet, a shell - lays its fields a row each, as the frame does with a field no row names.

use qymcad_ui_state::{Folded, Layout, Row, Section};

/// THE ROW OF A FIELD OF THE COMMAND under its own caption.
fn own(fields: &[qymcad_ui_state::CmdParam], key: &str) -> Row {
    Row::new(fields.iter().find(|p| p.key == key).map(qymcad_ui_state::field_caption).unwrap_or_default()).field(key)
}

/// THE KEYS OF THE FIELDS OF ONE DIRECTION OF A PATTERN: its copies and its pitch.
struct Direction {
    count: &'static str,
    step: &'static str,
}

/// The directions of a pattern of bodies.
const BODY_DIRECTIONS: [Direction; 3] = [Direction { count: "count", step: "step" }, Direction { count: "count2", step: "step2" }, Direction { count: "count3", step: "step3" }];

/// The directions of a pattern of parts.
const PART_DIRECTIONS: [Direction; 3] = [Direction { count: "count", step: "cstep" }, Direction { count: "count2", step: "cstep2" }, Direction { count: "count3", step: "cstep3" }];

/// THE ROWS OF THE DIRECTIONS OF A PATTERN: the copies and the pitch of each direction on its line.
fn directions(keys: &[Direction; 3]) -> Vec<Row> {
    keys.iter()
        .enumerate()
        .map(|(n, d)| {
            Row::new(qymcad_i18n::tr1("box-direction-n", "n", &(n + 1).to_string())).tagged(qymcad_i18n::tr("cmd-copies"), d.count).tagged(qymcad_i18n::tr("f-pitch"), d.step).unit("unit-mm-suffix")
        })
        .collect()
}

/// THE LAYOUT OF THE BOX OF THE PART COMMAND IN HAND.
pub(crate) fn command_layout(pc: &qymcad_ui_state::PartCtx) -> Layout {
    let f = &pc.cmd.params;
    let xyz = |caption: &str, keys: [&str; 3]| ["X", "Y", "Z"].into_iter().zip(keys).fold(Row::new(qymcad_i18n::tr(caption)), |r, (tag, key)| r.tagged(tag, key));
    let sections = match pc.armed.part_tool() {
        Some(qymcad_ui_state::PartTool::LinearPattern) => vec![Section::plain(directions(&BODY_DIRECTIONS))],
        // the hole and its shift from the centre of the face, then its recess - a counterbore or a countersink
        Some(qymcad_ui_state::PartTool::Hole) => vec![
            // the shift a row each: on one line the box grew over the centre of the face the hole is picked on
            Section::plain(vec![own(f, "diameter"), own(f, "depth"), own(f, "off_u"), own(f, "off_v")]),
            Section::titled(qymcad_i18n::tr("box-recess"), Folded::No, vec![own(f, "dia2"), own(f, "depth2")]),
        ],
        // the thread, how it begins and ends, and the roundings of its profile - folded, as they are seldom touched
        Some(qymcad_ui_state::PartTool::Thread) => vec![
            Section::plain(["nominal", "outer", "pitch", "angle", "depth", "thickness", "length", "fit"].into_iter().map(|k| own(f, k)).collect()),
            Section::titled(qymcad_i18n::tr("box-ends"), Folded::No, vec![own(f, "lead_in"), own(f, "lead_out")]),
            Section::titled(qymcad_i18n::tr("box-fillets"), Folded::Yes, vec![own(f, "crest_r"), own(f, "root_r"), own(f, "edge_r")]),
        ],
        Some(qymcad_ui_state::PartTool::DatumPoint) => vec![Section::plain(vec![xyz("cmd-coordinates", ["x", "y", "z"]).unit("unit-mm-suffix")])],
        Some(qymcad_ui_state::PartTool::DatumAxis) => vec![Section::plain(vec![xyz("box-origin", ["ox", "oy", "oz"]).unit("unit-mm-suffix"), xyz("cmd-direction", ["dx", "dy", "dz"])])],
        _ => Vec::new(),
    };
    Layout { sections, foot: Vec::new() }
}

/// THE LAYOUT OF THE BOX OF A PATTERN OF PARTS: a linear one by its directions, a circular one a field a row.
pub(crate) fn comp_array_layout(pc: &qymcad_ui_state::PartCtx) -> Layout {
    let sections = if pc.carr.mode == Some(qymcad_ui_state::PatternKind::Linear) { vec![Section::plain(directions(&PART_DIRECTIONS))] } else { Vec::new() };
    Layout { sections, foot: Vec::new() }
}

/// THE LAYOUT OF THE BOX OF A SECTION: the offset, and the two tilts on one line.
pub(crate) fn section_layout() -> Layout {
    let rows = vec![
        Row::new(qymcad_i18n::tr("sec-offset")).field("offset").unit("unit-mm-suffix"),
        Row::new(qymcad_i18n::tr("box-tilts")).tagged("1", "tilt1").tagged("2", "tilt2").unit("unit-deg-suffix"),
    ];
    Layout { sections: vec![Section::plain(rows)], foot: Vec::new() }
}
