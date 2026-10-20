//! IGES AS A DRAWING: every kind of curve a drawing is made of, read where it is drawn.
//!
//! The files are built here, by hand, in the fixed columns IGES is written in - so each case says exactly
//! which entity it is about, and nothing depends on a file lying on somebody's disk.
use qymcad_core::geom::ProfEdge;
use qymcad_io::read_iges_drawing;

/// One entity: IGES type, form, status (eight digits), the directory entry of its transform (0 for none) and its
/// parameters without the type and the terminator. Directory entries are numbered 1, 3, 5... in this order.
struct E(u32, u32, &'static str, usize, String);

fn iges(unit: (u32, &str), ents: &[E]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{:<72}S{:>7}\n", "a drawing for the check", 1));
    let g = format!(
        "1H,,1H;,5Hcheck,9Hcheck.igs,5Hcheck,5Hcheck,32,38,6,308,15,5Hcheck,1.,{},{}H{},1,0.1,15H20260913.000000,0.001,1000.,5Hcheck,5Hcheck,11,0,15H20260913.000000;",
        unit.0,
        unit.1.len(),
        unit.1
    );
    let gl: Vec<String> = g.as_bytes().chunks(72).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
    for (k, l) in gl.iter().enumerate() {
        out.push_str(&format!("{:<72}G{:>7}\n", l, k + 1));
    }
    let (mut d, mut p) = (String::new(), String::new());
    let mut pseq = 1;
    for (i, E(typ, form, status, matrix, params)) in ents.iter().enumerate() {
        let de = 2 * i + 1;
        let record = format!("{typ},{params};");
        let chunks: Vec<String> = record.as_bytes().chunks(64).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
        d.push_str(&format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}\n", typ, pseq, 0, 0, 0, 0, matrix, 0, status, de));
        d.push_str(&format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}\n", typ, 0, 0, chunks.len(), form, "", "", "", 0, de + 1));
        for c in &chunks {
            p.push_str(&format!("{:<64}{:>8}P{:>7}\n", c, de, pseq));
            pseq += 1;
        }
    }
    out.push_str(&d);
    out.push_str(&p);
    out.push_str(&format!("S{:>7}G{:>7}D{:>7}P{:>7}{:<40}T{:>7}\n", 1, gl.len(), 2 * ents.len(), pseq - 1, "", 1));
    out
}

fn read(name: &str, text: &str) -> Result<qymcad_io::IgesDrawing, String> {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/iges-drawing-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    std::fs::write(&p, text).expect("written");
    read_iges_drawing(&p.to_string_lossy())
}

const TOP: &str = "00000000"; // visible, independent, geometry
const PART: &str = "00020000"; // a member of a subfigure: dependent, not drawn on its own

fn lines(c: &[ProfEdge]) -> Vec<[f64; 4]> {
    c.iter().filter_map(|e| if let ProfEdge::Line { a, b } = e { Some([a.x, a.y, b.x, b.y]) } else { None }).collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A LINE, AN ARC, A CIRCLE AND A CLOSED POLYLINE come in as a line, an arc, a circle and four lines.
#[test]
fn lines_arcs_circles_and_polylines_come_in_as_drawn() {
    let text = iges(
        (2, "MM"),
        &[
            E(110, 0, TOP, 0, "0.,0.,0.,10.,0.,0.".into()),
            E(100, 0, TOP, 0, "0.,0.,0.,5.,0.,0.,5.".into()),
            E(100, 0, TOP, 0, "0.,20.,0.,23.,0.,23.,0.".into()),
            E(106, 63, TOP, 0, "1,4,0.,30.,0.,40.,0.,40.,10.,30.,10.".into()),
        ],
    );
    let d = read("kinds.igs", &text).expect("the drawing reads");
    assert_eq!(d.curves.len(), 7, "a line, an arc, a circle and a closed four-sided polyline: {:?}", d.curves);
    assert!(matches!(d.curves[1], ProfEdge::Arc { a, b, center, ccw: true } if near(a.x, 5.0) && near(b.y, 5.0) && near(center.x, 0.0)), "the arc: {:?}", d.curves[1]);
    assert!(matches!(d.curves[2], ProfEdge::Circle { center, r } if near(center.x, 20.0) && near(r, 3.0)), "the circle: {:?}", d.curves[2]);
    let closing = lines(&d.curves).into_iter().any(|l| near(l[0], 30.0) && near(l[1], 10.0) && near(l[2], 30.0) && near(l[3], 0.0));
    assert!(closing, "a closed polyline (form 63) did not close back to its first corner");
    assert!(!d.definitions_only && d.skipped.is_empty());
}

/// A FILE IN MICRONS COMES IN IN MILLIMETRES - the unit is the file's word, not ours.
#[test]
fn a_file_in_microns_comes_in_in_millimetres() {
    let d = read("microns.igs", &iges((9, "UM"), &[E(110, 0, TOP, 0, "0.,0.,0.,1000.,0.,0.".into())])).expect("reads");
    assert_eq!(lines(&d.curves), vec![[0.0, 0.0, 1.0, 0.0]], "1000 um is 1 mm");
}

/// SUBFIGURES ARE PLACED WHERE THEIR INSTANCES SAY: once scaled by an instance, three times by an array - and the
/// definition itself is not drawn at its own place.
#[test]
fn subfigures_are_placed_where_their_instances_say() {
    let text = iges(
        (2, "MM"),
        &[
            E(110, 0, PART, 0, "0.,0.,0.,1.,0.,0.".into()),             // DE 1: the member
            E(308, 0, "00020201", 0, "0,4Hunit,1,1".into()),            // DE 3: the definition
            E(408, 0, TOP, 0, "3,100.,0.,0.,2.".into()),                // DE 5: placed at x 100, twice the size
            E(412, 0, TOP, 0, "3,1.,0.,50.,0.,3,1,10.,0.,0.,0".into()), // DE 7: three copies, 10 apart, at y 50
        ],
    );
    let d = read("subfigures.igs", &text).expect("reads");
    let mut got = lines(&d.curves);
    got.sort_by(|a, b| (a[1], a[0]).partial_cmp(&(b[1], b[0])).expect("numbers"));
    assert_eq!(got, vec![[100.0, 0.0, 102.0, 0.0], [0.0, 50.0, 1.0, 50.0], [10.0, 50.0, 11.0, 50.0], [20.0, 50.0, 21.0, 50.0]]);
}

/// A LIBRARY PLACES NOTHING: its cells are shown, and it says so - not the parts the cells are made of.
#[test]
fn a_library_shows_its_cells_and_says_so() {
    let text = iges(
        (2, "MM"),
        &[
            E(110, 0, PART, 0, "0.,0.,0.,1.,0.,0.".into()),  // DE 1: a member of the part
            E(308, 0, "00020201", 0, "0,4Hpart,1,1".into()), // DE 3: a part...
            E(408, 0, PART, 0, "3,5.,0.,0.".into()),         // DE 5: ...placed inside the cell
            E(308, 0, "00020201", 0, "1,4Hcell,1,5".into()), // DE 7: the cell, placed nowhere
        ],
    );
    let d = read("library.igs", &text).expect("reads");
    assert!(d.definitions_only, "a file of definitions was not told apart from a drawing");
    assert_eq!(lines(&d.curves), vec![[5.0, 0.0, 6.0, 0.0]], "the cell, with its part in place, and nothing else");
}

/// A TRANSFORM MOVES WHAT IT CARRIES; a turn or a mirror keeps an arc an arc, and a mirror turns it round.
#[test]
fn a_transform_moves_what_it_carries_and_keeps_arcs_arcs() {
    let text = iges(
        (2, "MM"),
        &[
            E(124, 0, "00010000", 0, "1.,0.,0.,5.,0.,1.,0.,7.,0.,0.,1.,0.".into()), // DE 1: a shift
            E(110, 0, TOP, 1, "0.,0.,0.,10.,0.,0.".into()),
            E(124, 0, "00010000", 0, "0.,-1.,0.,0.,1.,0.,0.,0.,0.,0.,1.,0.".into()), // DE 5: a quarter turn
            E(100, 0, TOP, 5, "0.,0.,0.,5.,0.,0.,5.".into()),
            E(124, 0, "00010000", 0, "-1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.".into()), // DE 9: a mirror
            E(100, 0, TOP, 9, "0.,0.,0.,5.,0.,0.,5.".into()),
        ],
    );
    let d = read("transforms.igs", &text).expect("reads");
    assert_eq!(lines(&d.curves), vec![[5.0, 7.0, 15.0, 7.0]], "the shifted line");
    assert!(matches!(d.curves[1], ProfEdge::Arc { a, b, ccw: true, .. } if near(a.x, 0.0) && near(a.y, 5.0) && near(b.x, -5.0) && near(b.y, 0.0)), "the turned arc: {:?}", d.curves[1]);
    assert!(matches!(d.curves[2], ProfEdge::Arc { a, b, ccw: false, .. } if near(a.x, -5.0) && near(b.y, 5.0)), "the mirrored arc: {:?}", d.curves[2]);
}

/// A SPLINE IS LAID AS A SPLINE ALONG ITS CURVE: a rational quarter circle comes in as one spline of the sketch, on its
/// radius within 0.001 mm, from its start to its end.
#[test]
fn a_rational_spline_stays_on_its_circle() {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let text = iges((2, "MM"), &[E(126, 0, TOP, 0, format!("2,2,1,0,0,0,0.,0.,0.,1.,1.,1.,1.,{w},1.,5.,0.,0.,5.,5.,0.,0.,5.,0.,0.,1.,0.,0.,1."))]);
    let d = read("spline.igs", &text).expect("reads");
    let [ProfEdge::Spline(b)] = d.curves.as_slice() else { panic!("a quarter circle came in as {:?}, not one spline", d.curves) };
    let off = (0..=1000).map(|k| b.at(k as f64 / 1000.0)).map(|q| (q.x.hypot(q.y) - 5.0).abs()).fold(0.0, f64::max);
    assert!(off < 1e-3, "the spline stands up to {off:.5} off its circle");
    let ends = [b.at(0.0), b.at(1.0)];
    assert!(near(ends[0].x, 5.0) && near(ends[0].y, 0.0) && near(ends[1].x, 0.0) && near(ends[1].y, 5.0), "the spline does not run from (5, 0) to (0, 5): {ends:?}");
}

/// NOTES AND DIMENSIONS ARE COUNTED, NOT DRAWN - and a file of nothing else is refused by name.
#[test]
fn notes_are_counted_and_a_file_of_nothing_is_refused() {
    let note = E(212, 0, TOP, 0, "1,5,1.,1.,0.,0.,0,0,0.,0.,0.,5Hnote.".into());
    let d = read("note.igs", &iges((2, "MM"), &[note, E(110, 0, TOP, 0, "0.,0.,0.,1.,0.,0.".into())])).expect("reads");
    assert_eq!(d.skipped, vec![qymcad_io::IgesCount { typ: 212, count: 1 }], "the note was not counted");
    assert_eq!(d.curves.len(), 1);
    let only = E(212, 0, TOP, 0, "1,5,1.,1.,0.,0.,0,0,0.,0.,0.,5Hnote.".into());
    assert_eq!(read("only-note.igs", &iges((2, "MM"), &[only])).err().as_deref(), Some("io-iges-no-curves"));
    assert_eq!(read("rubbish.igs", "not an IGES file\n").err().as_deref(), Some("io-iges-not-iges"));
}

/// A file of `tests/data`, read as the window reads an IGES with no surfaces.
fn data(name: &str) -> qymcad_io::IgesDrawing {
    read_iges_drawing(&format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).expect("the drawing reads")
}

/// The lines of a drawing, ordered by their start, each rounded to 1e-9 so that a turn's -0 and 6e-17 read as 0.
fn sorted_lines(d: &qymcad_io::IgesDrawing) -> Vec<[f64; 4]> {
    let mut got: Vec<[f64; 4]> = lines(&d.curves).into_iter().map(|l| l.map(|v| (v * 1e9).round() / 1e9 + 0.0)).collect();
    got.sort_by(|a, b| a.partial_cmp(b).expect("numbers"));
    got
}

/// A NETWORK SUBFIGURE INSTANCE (420) IS MOVED BY ITS PARAMETERS 2-4 AND SCALED BY 5-7 (IGES 5.3, 4.140), as a singular
/// one (408) is by 2-4 and 5. `net420.igs`: the line (0,0)-(10,0) of a 320 placed at (100,50) twice its size, beside an
/// independent line (100,40)-(120,40). Reported behaviour: the instance came in from (2,2) to (1002,2) - scaled by
/// (100,50) and moved by (2,2). `sub408.igs` is the same placement by a 408.
#[test]
fn a_network_instance_is_moved_by_its_translation_and_scaled_by_its_scale() {
    let want = vec![[100.0, 40.0, 120.0, 40.0], [100.0, 50.0, 120.0, 50.0]];
    assert_eq!(sorted_lines(&data("net420.igs")), want, "the 420 instance");
    assert_eq!(sorted_lines(&data("sub408.igs")), want, "the 408 instance of the same placement");
    // only XS given: YS and ZS are XS, not 1
    let text = iges(
        (2, "MM"),
        &[
            E(110, 0, PART, 0, "0.,0.,0.,10.,10.,0.".into()),          // DE 1: the member, a diagonal
            E(320, 0, "00000200", 0, "0,4HCELL,1,1,0,1HC,0,0".into()), // DE 3: the definition
            E(420, 0, TOP, 0, "3,100.,50.,0.,2.".into()),              // DE 5: at (100,50), XS 2
        ],
    );
    assert_eq!(sorted_lines(&read("net420-xs.igs", &text).expect("reads")), vec![[100.0, 50.0, 120.0, 70.0]], "a 420 with only XS given");
}

/// A RECTANGULAR ARRAY (412) DRAWS WHAT ITS DO/DON'T LIST SAYS (IGES 5.3, 4.136): parameter 11 the count of the list,
/// 12 the flag (0 DO, 1 DON'T), 13 on the positions, counted from the lower left up each column, then up the next.
/// `arr412.igs`: the line (0,0)-(4,0) in three columns 10 apart from (100,0), DON'T position 2. Reported behaviour: all
/// three copies came in, the middle one at (110,0)-(114,0) too.
#[test]
fn a_rectangular_array_draws_what_its_do_dont_list_says() {
    assert_eq!(sorted_lines(&data("arr412.igs")), vec![[100.0, 0.0, 104.0, 0.0], [120.0, 0.0, 124.0, 0.0]], "DON'T position 2 of three");
    let array = |list: &str| {
        let text = iges(
            (2, "MM"),
            &[
                E(110, 0, "00010000", 0, "0.,0.,0.,4.,0.,0.".into()),                // DE 1: the base
                E(412, 0, TOP, 0, format!("1,1.,100.,0.,0.,2,2,10.,20.,0.,{list}")), // DE 3: two columns of two rows
            ],
        );
        sorted_lines(&read("arr412-list.igs", &text).expect("reads"))
    };
    // positions 1 (100,0), 2 (100,20), 3 (110,0), 4 (110,20): up the first column, then up the second
    assert_eq!(array("1,0,2"), vec![[100.0, 20.0, 104.0, 20.0]], "DO position 2: only the upper one of the first column");
    assert_eq!(array("2,1,1,4"), vec![[100.0, 20.0, 104.0, 20.0], [110.0, 0.0, 114.0, 0.0]], "DON'T positions 1 and 4");
    assert_eq!(array("0"), vec![[100.0, 0.0, 104.0, 0.0], [100.0, 20.0, 104.0, 20.0], [110.0, 0.0, 114.0, 0.0], [110.0, 20.0, 114.0, 20.0]], "an empty list: all");
}

/// AN INSTANCE'S OWN MATRIX TURNS WHAT IT PLACES, NOT WHERE IT STANDS: scale, then the instance's own 124, then its
/// X, Y, Z, which that matrix does not move (IGES 5.3, 3.6.2) - for a 408, a 420 and the corner of a 412 alike.
/// `rot408.igs`: the line (0,0)-(10,0) placed at (100,0) by a 408 whose own matrix is a quarter turn, over a reference
/// line (90,-5)-(110,-5). Reported behaviour: the quarter turn swung the position too, and the line came in at
/// (0,100)-(0,110).
#[test]
fn an_instance_is_turned_by_its_own_matrix_where_it_stands() {
    assert_eq!(sorted_lines(&data("rot408.igs")), vec![[90.0, -5.0, 110.0, -5.0], [100.0, 0.0, 100.0, 10.0]], "the 408");
    const QUARTER: &str = "0.,-1.,0.,0.,1.,0.,0.,0.,0.,0.,1.,0.";
    let text = iges(
        (2, "MM"),
        &[
            E(124, 0, "00010000", 0, QUARTER.into()),                  // DE 1: a quarter turn
            E(110, 0, PART, 0, "0.,0.,0.,10.,0.,0.".into()),           // DE 3: the member
            E(320, 0, "00000200", 0, "0,4HCELL,1,3,0,1HC,0,0".into()), // DE 5: the definition
            E(420, 0, TOP, 1, "5,100.,0.,0.,1.".into()),               // DE 7: at (100,0), turned by DE 1
        ],
    );
    assert_eq!(sorted_lines(&read("rot420.igs", &text).expect("reads")), vec![[100.0, 0.0, 100.0, 10.0]], "the 420");
    let text = iges(
        (2, "MM"),
        &[
            E(124, 0, "00010000", 0, QUARTER.into()),                    // DE 1: a quarter turn
            E(110, 0, "00010000", 0, "0.,0.,0.,4.,0.,0.".into()),        // DE 3: the base
            E(412, 0, TOP, 1, "3,1.,100.,0.,0.,2,1,10.,0.,0.,0".into()), // DE 5: two columns from (100,0), turned by DE 1
        ],
    );
    assert_eq!(sorted_lines(&read("rot412.igs", &text).expect("reads")), vec![[100.0, 0.0, 100.0, 4.0], [110.0, 0.0, 110.0, 4.0]], "the 412");
}

/// A MEMBER OF A GROUP IS DRAWN: subordinate switch 02, logically dependent, is an entity that "can exist alone" and
/// is only gathered by a grouping entity such as a 402 (IGES 5.3, 2.2.4.4.9.2). `group402.igs`: a line at y 0 and two
/// at y 20 and 40 in a 402. Reported behaviour: only the line at y 0 came in, and nothing was said. A drawing whose
/// every line is in a group comes in too, not refused as one with no curves.
#[test]
fn a_member_of_a_group_is_drawn() {
    let want = vec![[0.0, 0.0, 50.0, 0.0], [0.0, 20.0, 50.0, 20.0], [0.0, 40.0, 50.0, 40.0]];
    assert_eq!(sorted_lines(&data("group402.igs")), want, "the grouped lines");
    let grouped = "00020000";
    let text = iges(
        (2, "MM"),
        &[
            E(110, 0, grouped, 0, "0.,0.,0.,50.,0.,0.".into()),   // DE 1
            E(110, 0, grouped, 0, "0.,20.,0.,50.,20.,0.".into()), // DE 3
            E(402, 1, TOP, 0, "2,1,3".into()),                    // DE 5: the group
        ],
    );
    let d = read("all-grouped.igs", &text).expect("a drawing of grouped lines reads");
    assert_eq!(sorted_lines(&d), vec![[0.0, 0.0, 50.0, 0.0], [0.0, 20.0, 50.0, 20.0]], "every line in the group");
    assert!(!d.definitions_only, "grouped lines are not a library");
}

/// THE REPORTED FILE: a chip library cell, defined and placed nowhere. `QYM_IGES` names it.
#[test]
#[ignore = "a file on this machine"]
fn the_reported_file() {
    let Ok(path) = std::env::var("QYM_IGES") else { return };
    let d = read_iges_drawing(&path).expect("the drawing reads");
    eprintln!("PROBE {path}: {} curves, definitions only: {}, skipped {:?}", d.curves.len(), d.definitions_only, d.skipped);
    assert!(d.definitions_only && d.curves.len() >= 80, "the cell came in as {} curves", d.curves.len());
}
