//! WHAT IS IN THE WINDOW BEFORE A TOOL IS TAKEN, built the way a person builds it.
use qymcad::Session;

use crate::build;

/// A document to take a tool in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fixture {
    /// The first part of a fresh start, entered, with nothing in it.
    FirstPart,
    /// The first part with the window switched to the light theme by View -> Light.
    FirstPartLight,
    /// A 40 x 30 rectangle on XY in the first part, its sketch finished.
    RectangleSketch,
    /// A 40 x 30 x 10 block in the first part: the rectangle extruded 10.
    Block,
    /// The same block with its top face and its front face copied out as two sheets of surface, lying on the faces
    /// they were taken from and meeting along the top front edge.
    BlockAndTwoSheets,
    /// The same block with a hole 20 across through it, made at the middle of its top face: 12000 - pi * 100 * 10 =
    /// 8858.41 mm^3. A hole that wide lets a click reach its wall, which a hole of the ordinary 6 does not.
    BlockWithHole,
    /// A block far off the vertical axis: a 40 x 20 rectangle from (200, -10) to (240, 10), extruded 10. Seen from
    /// that axis it is 5.7 degrees wide, so even 52 copies turned about it stand apart, each a piece of its own.
    BlockOffAxis,
    /// The block with its extrusion undone by Ctrl+Z: the rectangle stands alone, the extrusion waits to be redone.
    BlockUndone,
    /// The block looked at from so near - the wheel turned in over the middle of the canvas - that it runs off every
    /// side of it.
    BlockZoomedIn,
    /// The block in the first part, the part finished: the assembly is open around it, nothing grounded.
    BlockInAssembly,
    /// The same, with the part picked in the viewport by a click on the top of its block.
    BlockInAssemblyPicked,
    /// Two parts in the assembly, each a 40 x 30 x 10 block: the first from the origin, the second from (60, 0),
    /// both finished, nothing grounded, no joint.
    TwoBlocksInAssembly,
    /// The block in the first part and, in a second part, an upright shaft 10 across and 10 tall about (70, 15), both
    /// finished, nothing grounded, no mate.
    BlockAndShaftInAssembly,
    /// Three parts of the assembly, each extruded 10: a wall from (0, 0) to (10, 30), a wall from (50, 0) to (60, 30),
    /// and a tab from (20, 10) to (30, 20) between them, off the middle; nothing grounded, no mate.
    TwoWallsAndATab,
    /// Three blocks 40 x 30 x 10 in three parts, at 0, 60 and 120 along X, the view fitted; a revolute mate between
    /// the corner (40, 0, 10) of the first and (100, 0, 10) of the second, and one between (40, 0, 0) of the first and
    /// (160, 0, 10) of the third, both laid as built, so no part moves. The mates are "Revolute 1" and "Revolute 2".
    TwoHingesInAssembly,
    /// The two blocks of `TwoBlocksInAssembly`, the second part put 40 back along X: its block stands from 20 to 60 and
    /// runs 20 x 30 x 10 = 6000 mm^3 into the first.
    TwoBlocksOverlapping,
    /// The block, a sheet copied off its top face, then a boss 10 across about (20, 15) grown 20 from the plane under
    /// the block, 10 above its top: the body now crosses the sheet along the circle of the boss, where a trim divides
    /// it.
    BlockSheetAndBoss,
    /// The block in the first part and, in a second part made in context, an empty sketch on the top face of the block
    /// - a live external reference to the first part; the assembly open, the second part taken, its properties shown.
    PartOnNeighbourFace,
    /// The block cut across 5 below its top into two bodies of 6000, then a boss 20 x 10 x 8 pushed down from the top
    /// over 10..30 x 10..20: it grows the lower body to 7000, reaching 1000 into the upper one. The upper body, body A,
    /// picked beside the boss.
    OverlappingPiecesUpperPicked,
    /// The block with a sketch open on its top face, empty, the pencil put down.
    SketchOnBlockTop,
    /// A cylinder 20 across and 10 tall standing on the table about (20, 15), made by extruding a circle.
    CylinderBody,
    /// The block written out as a mesh (File, Export, STL) and brought into a new project's first part as that mesh:
    /// the 12 triangles of a 40 x 30 x 10 box, no faces to take.
    MeshOfBlock,
    /// A square of 10 on the table and a straight line of 40 up the front plane, in two sketches of the first part, the
    /// square's sketch picked in the tree: a profile and a path to sweep it along.
    SweepStraight,
    /// A square of 10 on the table and one of 20 on a datum plane 30 above, centred over it, in two sketches, the
    /// first picked in the tree: two sections to loft between.
    LoftSquares,
    /// A sample project, by its path from the root of the repository, opened; its first part entered.
    Sample(&'static str),
    /// A sketch open on the XY plane of the first part, empty, the arrow in hand.
    SketchOnXy,
    /// The same sketch with a 40 x 30 rectangle drawn from the origin.
    RectangleInSketch,
    /// The same rectangle with its bottom side, from (0, 0) to (40, 0), picked by the arrow.
    RectangleSidePicked,
    /// The same sketch with a line from (10, 5) to (30, 25) and an upright line from (0, -20) to (0, 20) to reflect it
    /// about; the first line picked by the arrow. The first line keeps off the axes: an end put on one is held there.
    LineAndAxisPicked,
    /// The same rectangle with its bottom side cut away by Edit -> Cut, the base point clicked on it: the side waits in
    /// the clipboard.
    RectangleSideCut,
    /// The same sketch with a circle of radius 10 about the origin.
    CircleInSketch,
    /// The same sketch with a quarter arc of radius 10 about the origin, from (10, 0) to (0, 10).
    ArcInSketch,
    /// The same sketch with two lines from the origin: one along X, one at 45 degrees to it.
    TwoLinesInSketch,
    /// The same two lines, the one at 45 degrees 20 long against 30: lines of one length are tied Equal as they are
    /// drawn, and a midpoint of one end on the other line can then be met only by shrinking both to a point.
    TwoLinesOfTwoLengths,
    /// The same sketch with a circle of radius 10 about the origin and a line across it from (-20, 0) to (20, 0): the
    /// line cuts the circle at (-10, 0) and (10, 0).
    LineThroughCircle,
    /// The same sketch with a line from (0, 0) to (20, 0) and a line across its way from (30, -10) to (30, 10): the first
    /// stops 10 short of the second.
    LineShortOfLine,
    /// The same sketch with a corner drawn as one chain of the line tool: from (30, 0) to the origin and up to (0, 30).
    CornerInSketch,
    /// The same sketch with two circles of radius 5: one about (0, 0), one about (30, 0).
    TwoCirclesInSketch,
}

impl Fixture {
    /// Build it in the part open for editing - the first part, unless the check went into another.
    pub fn build_here(self, s: &mut Session) {
        match self {
            Fixture::FirstPart => {}
            Fixture::FirstPartLight => {
                let (view, light) = (s.word("menu-view"), s.word("scheme-light"));
                s.menu(&[&view, &light]);
            }
            Fixture::RectangleSketch => {
                build::rectangle_on_xy(s);
                let finish = s.word("wb-finish");
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
            }
            Fixture::Block => {
                Fixture::RectangleSketch.build_here(s);
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude).key(qymcad::Key::Enter);
            }
            Fixture::BlockAndTwoSheets => {
                Fixture::Block.build_here(s);
                let copy = s.word("tb-face-copy-hint");
                for p in [[20.0, 15.0, 10.0], [20.0, 0.0, 5.0]] {
                    s.press_hint(&copy);
                    let at = s.face_at(p);
                    s.click(at);
                    s.key(qymcad::Key::Enter);
                }
            }
            Fixture::BlockWithHole => {
                Fixture::Block.build_here(s);
                let hole = s.word("tb-hole-hint");
                s.press_hint(&hole);
                let at = s.face_at([20.0, 15.0, 10.0]);
                s.click(at);
                let diameter = s.word("f-diameter");
                s.fill(&diameter, "20");
                s.key(qymcad::Key::Enter);
            }
            Fixture::BlockZoomedIn => {
                Fixture::Block.build_here(s);
                let c = s.canvas();
                let middle = c.center();
                for _ in 0..12 {
                    s.wheel(middle, qymcad::vec2(0.0, 200.0), qymcad::Modifiers::default());
                }
            }
            Fixture::BlockUndone => {
                Fixture::Block.build_here(s);
                s.chord(qymcad::Modifiers::COMMAND, qymcad::Key::Z);
            }
            Fixture::BlockInAssembly => {
                Fixture::Block.build_here(s);
                let finish = s.word("wb-finish");
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
            }
            Fixture::BlockInAssemblyPicked => {
                Fixture::BlockInAssembly.build_here(s);
                let top = s.face_at([20.0, 15.0, 10.0]);
                s.click(top);
            }
            Fixture::TwoBlocksInAssembly => {
                Fixture::BlockInAssembly.build_here(s);
                let new_part = s.word("tb-new-part-hint");
                s.press_hint(&new_part);
                let xy = s.word("plane-xy-table");
                s.press_word(&xy);
                draw(s, "tb-rect-hint", &[(60.0, 0.0), (100.0, 30.0)]);
                let finish = s.word("wb-finish");
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude).key(qymcad::Key::Enter);
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
            }
            Fixture::BlockAndShaftInAssembly => {
                Fixture::BlockInAssembly.build_here(s);
                let new_part = s.word("tb-new-part-hint");
                s.press_hint(&new_part);
                let xy = s.word("plane-xy-table");
                s.press_word(&xy);
                draw(s, "tb-circle-hint", &[(70.0, 15.0), (75.0, 15.0)]);
                let finish = s.word("wb-finish");
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude).key(qymcad::Key::Enter);
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
            }
            Fixture::PartOnNeighbourFace => {
                Fixture::Block.build_here(s);
                crate::build::into_a_new_part(s);
                let context = s.word("wb-in-context");
                let switch = s.widgets().into_iter().find(|w| w.label.ends_with(&context)).unwrap_or_else(|| panic!("there is no switch for working in context"));
                if switch.checked == Some(false) {
                    s.click(switch.rect.center());
                }
                crate::build::sketch_on_face(s, [20.0, 15.0, 10.0]);
                let finish = s.word("wb-finish");
                s.press_word(&finish);
                let assembly = s.word("wb-assembly");
                s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
                let second = s.document().parts[1].name.clone();
                let row = s.find(&second, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {second:?} is not in the tree"));
                s.click(row.center());
            }
            Fixture::BlockSheetAndBoss => {
                Fixture::Block.build_here(s);
                let copy = s.word("tb-face-copy-hint");
                s.press_hint(&copy);
                let top = s.face_at([20.0, 15.0, 10.0]);
                s.click(top);
                s.key(qymcad::Key::Enter);
                // the boss is drawn on the plane under the block and grown 20 through it, not on the top face: the
                // sheet lies on that face too, and a sketch put there would sit on the sheet
                let xy = s.word("plane-xy-table");
                s.press_word(&xy);
                draw(s, "tb-circle-hint", &[(20.0, 15.0), (25.0, 15.0)]);
                let finish = s.word("wb-finish");
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude);
                let length = s.word("f-length");
                s.fill(&length, "20").key(qymcad::Key::Enter);
            }
            Fixture::TwoBlocksOverlapping => {
                Fixture::TwoBlocksInAssembly.build_here(s);
                let second = s.document().parts.last().cloned().expect("the second part").name;
                let row = s.find(&second, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {second:?} is not in the tree"));
                s.click(row.center());
                s.fill("X", "-40").key(qymcad::Key::Enter);
            }
            Fixture::TwoHingesInAssembly => {
                Fixture::BlockInAssembly.build_here(s);
                let finish = s.word("wb-finish");
                let extrude = s.word("tb-extrude-hint");
                let xy = s.word("plane-xy-table");
                for x in [60.0, 120.0] {
                    let new_part = s.word("tb-new-part-hint");
                    s.press_hint(&new_part);
                    let made = s.document().parts.last().cloned().expect("the new part").name;
                    s.press_word(&xy);
                    draw(s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
                    s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                    s.press_hint(&extrude).key(qymcad::Key::Enter);
                    s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                    let row = s.find(&made, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {made:?} is not in the tree"));
                    s.click(row.center());
                    s.fill("X", &x.to_string()).key(qymcad::Key::Enter);
                }
                let (view, fit) = (s.word("menu-view"), s.word("menu-fit-view"));
                s.menu(&[&view, &fit]);
                for (a, b) in [([40.0, 0.0, 10.0], [100.0, 0.0, 10.0]), ([40.0, 0.0, 0.0], [160.0, 0.0, 10.0])] {
                    let start = s.word("jp-start-joint");
                    s.press_word_near(&start, qymcad::pos2(1100.0, 400.0));
                    let caption = s.word("j-kind");
                    let list = s.field(&caption);
                    s.click(list.rect.center());
                    let want = s.word("joint-kind-revolute");
                    s.press_word_near(&want, list.rect.center());
                    let as_built = s.word("j-as-built");
                    s.press_word_near(&as_built, qymcad::pos2(640.0, 24.0));
                    let at = s.vertex_at(a);
                    s.click(at);
                    let at = s.vertex_at(b);
                    s.click(at);
                    // Enter keeps the joint the second pick made, Esc then puts the tool down
                    s.key(qymcad::Key::Enter).key(qymcad::Key::Escape);
                }
            }
            Fixture::TwoWallsAndATab => {
                let finish = s.word("wb-finish");
                let extrude = s.word("tb-extrude-hint");
                let xy = s.word("plane-xy-table");
                for (i, (a, b)) in [((0.0, 0.0), (10.0, 30.0)), ((50.0, 0.0), (60.0, 30.0)), ((20.0, 10.0), (30.0, 20.0))].into_iter().enumerate() {
                    if i > 0 {
                        let new_part = s.word("tb-new-part-hint");
                        s.press_hint(&new_part);
                    }
                    s.press_word(&xy);
                    draw(s, "tb-rect-hint", &[a, b]);
                    s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                    s.press_hint(&extrude).key(qymcad::Key::Enter);
                    s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                }
            }
            Fixture::OverlappingPiecesUpperPicked => {
                Fixture::Block.build_here(s);
                let cut = s.word("tb-split-body-hint");
                s.press_hint(&cut);
                let top = s.face_at([20.0, 15.0, 10.0]);
                s.click(top);
                let offset = s.word("f-offset");
                s.fill(&offset, "-5");
                s.key(qymcad::Key::Enter).key(qymcad::Key::Escape);
                // the pieces of a split only touch; the boss joins the lower one and reaches into the upper
                let pencil = s.word("g-sketch-pick-hint");
                s.press_hint(&pencil);
                let top = s.face_at([20.0, 15.0, 10.0]);
                s.click(top);
                draw(s, "tb-rect-hint", &[(10.0, 10.0), (30.0, 20.0)]);
                let finish = s.word("wb-finish");
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude);
                let flip = s.word("cmd-flip");
                s.press_word_near(&flip, qymcad::pos2(640.0, 24.0));
                let length = s.word("f-length");
                s.fill(&length, "8");
                s.key(qymcad::Key::Enter);
                let upper = s.face_at([5.0, 15.0, 10.0]);
                s.double_click(upper); // a double click takes the body, one click only its face
            }
            Fixture::SketchOnBlockTop => {
                Fixture::Block.build_here(s);
                let pencil = s.word("g-sketch-pick-hint");
                s.press_hint(&pencil);
                let top = s.face_at([20.0, 15.0, 10.0]);
                s.click(top);
            }
            Fixture::BlockOffAxis => {
                let xy = s.word("plane-xy-table");
                s.press_word(&xy);
                draw(s, "tb-rect-hint", &[(200.0, -10.0), (240.0, 10.0)]);
                let finish = s.word("wb-finish");
                s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude).key(qymcad::Key::Enter);
            }
            Fixture::SketchOnXy => {
                let xy = s.word("plane-xy-table");
                s.press_word(&xy);
            }
            Fixture::RectangleInSketch => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
            }
            Fixture::RectangleSidePicked => {
                Fixture::RectangleInSketch.build_here(s);
                s.click_on_sketch(20.0, 0.0);
            }
            Fixture::LineAndAxisPicked => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-line-hint", &[(10.0, 5.0), (30.0, 25.0)]);
                draw(s, "tb-line-hint", &[(0.0, -20.0), (0.0, 20.0)]);
                s.click_on_sketch(20.0, 15.0);
            }
            Fixture::RectangleSideCut => {
                Fixture::RectangleSidePicked.build_here(s);
                let (edit, cut) = (s.word("menu-edit"), s.word("menu-cut"));
                s.menu(&[&edit, &cut]);
                s.click_on_sketch(20.0, 0.0);
            }
            Fixture::CircleInSketch => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-circle-hint", &[(0.0, 0.0), (10.0, 0.0)]);
            }
            Fixture::ArcInSketch => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-arc-hint", &[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]);
            }
            Fixture::TwoLinesInSketch => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-line-hint", &[(0.0, 0.0), (30.0, 0.0)]);
                draw(s, "tb-line-hint", &[(0.0, 0.0), (21.21, 21.21)]);
            }
            Fixture::TwoLinesOfTwoLengths => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-line-hint", &[(0.0, 0.0), (30.0, 0.0)]);
                draw(s, "tb-line-hint", &[(0.0, 0.0), (14.14, 14.14)]);
            }
            Fixture::LineThroughCircle => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-circle-hint", &[(0.0, 0.0), (10.0, 0.0)]);
                draw(s, "tb-line-hint", &[(-20.0, 0.0), (20.0, 0.0)]);
            }
            Fixture::LineShortOfLine => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-line-hint", &[(0.0, 0.0), (20.0, 0.0)]);
                draw(s, "tb-line-hint", &[(30.0, -10.0), (30.0, 10.0)]);
            }
            Fixture::CornerInSketch => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-line-hint", &[(30.0, 0.0), (0.0, 0.0), (0.0, 30.0)]);
            }
            Fixture::TwoCirclesInSketch => {
                Fixture::SketchOnXy.build_here(s);
                draw(s, "tb-circle-hint", &[(0.0, 0.0), (5.0, 0.0)]);
                draw(s, "tb-circle-hint", &[(30.0, 0.0), (35.0, 0.0)]);
            }
            Fixture::Sample(path) => panic!("the sample {path} is a document of its own and cannot be built inside another"),
            Fixture::SweepStraight | Fixture::LoftSquares | Fixture::CylinderBody | Fixture::MeshOfBlock => panic!("{self:?} builds its own part from the start and cannot be built inside another"),
        }
    }

    /// A fresh start with it built in the first part - or the sample opened and its first part entered.
    pub fn start(self) -> Session {
        let mut s = Session::start();
        if let Fixture::Sample(path) = self {
            build::open_project(&mut s, &format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR")));
            let part = s.document().parts.first().map(|p| p.name.clone()).unwrap_or_else(|| panic!("the sample {path} holds no part"));
            let row = s.find(&part, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {part:?} of {path} is not in the tree"));
            s.double_click(row.center());
            return s;
        }
        match self {
            Fixture::SweepStraight => crate::bodies::sweep_straight(&mut s),
            Fixture::LoftSquares => crate::bodies::loft_squares(&mut s),
            Fixture::CylinderBody => crate::bodies::cylinder(&mut s),
            Fixture::MeshOfBlock => mesh_of_block(&mut s),
            _ => {
                build::into_the_first_part(&mut s);
                self.build_here(&mut s);
            }
        }
        s
    }
}

/// Draw with the tool whose hint is `hint`, clicking these places of the sheet in turn, and put the tool down: the
/// fixture is what is drawn, with nothing in hand.
fn draw(s: &mut Session, hint: &str, places: &[(f64, f64)]) {
    let hint = s.word(hint);
    s.press_hint(&hint);
    for (x, y) in places {
        s.click_on_sketch(*x, *y);
    }
    s.key(qymcad::Key::Escape);
    let arrow = s.word("tb-select-hint");
    s.press_hint(&arrow);
}

/// The block written out as STL and brought back into a fresh project as a mesh, as a person does it.
fn mesh_of_block(s: &mut Session) {
    build::block(s);
    let path = format!("{}/qymcad-contract-mesh-{}.stl", std::env::temp_dir().display(), std::process::id());
    let (file, export) = (s.word("menu-file"), s.word("file-export"));
    s.menu(&[&file, &export, "STL\u{2026}"]);
    let quality = s.word("stl-standard");
    s.press_word(&quality);
    s.answer_file(&path);
    let (file, new) = (s.word("menu-file"), s.word("file-new"));
    s.menu(&[&file, &new]);
    let dont_save = s.word("nav-dont-save");
    if let Some(button) = s.find(&dont_save, qymcad::pos2(0.0, 0.0)) {
        s.click(button.center());
    }
    build::into_the_first_part(s);
    let (file, import) = (s.word("menu-file"), s.word("file-import"));
    s.menu(&[&file, &import]);
    s.answer_file(&path);
    // the placement and the units are taken as they come
    let go = s.word("import-scale-import");
    s.press_word_near(&go, qymcad::pos2(1200.0, 700.0));
}
