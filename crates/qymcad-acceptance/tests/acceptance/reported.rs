//! REPORTED BEHAVIOUR THE SET HAD MISSED: troubles found by using the program by hand. Each probe says which hole of
//! the set it closes - a point of the contract not asked, a place of the window not looked at, a path of a person not
//! taken.
use qymcad::{Key, Machine, Modifiers, PointerButton, Session};
use qymcad_acceptance::{build, probe};

/// A project of the repository's samples.
const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/Filter-v2.qcad");

/// CLOSE THE PROGRAM AND ANSWER WHAT IT ASKS, keeping what it keeps between runs.
fn close_the_program(s: Session) -> qymcad::Kept {
    match s.quit() {
        Ok(kept) => kept,
        Err(mut s) => {
            let dont_save = s.word("nav-dont-save");
            s.press_word(&dont_save);
            s.quit().unwrap_or_else(|_| panic!("the window was closed and would not give what it keeps"))
        }
    }
}

probe! {
    /// THE PROJECT OF THE LAST RUN IS OPEN AT THE NEXT START - what a person worked on is where they left it, not
    /// an empty document every time.
    ///
    /// The hole: the set checked that the project is OFFERED among the recent ones, never that it is opened.
    fn the_project_of_the_last_run_opens_at_the_next_start() {
        let mut s = Session::start();
        build::open_project(&mut s, SAMPLE);
        let opened = s.document().path.clone().unwrap_or_else(|| panic!("the sample did not open: the program says {:?}", s.status()));
        let kept = close_the_program(s);
        let mut s = Session::start_on(Machine { kept, ..Machine::default() });
        let now = s.document().path;
        assert!(now.as_deref() == Some(opened.as_str()), "{opened:?} was open when the program closed, and the next start opened {now:?}");
    }
}

probe! {
    /// A FIRST START HOLDS A CUBE, NOT AN EMPTY PART: a person opening the program for the first time is met by a part
    /// called Cube, a sketch of a square and its extrusion in it - stepping in shows how the work goes (decided 25.09).
    fn a_first_start_holds_a_cube() {
        let mut s = Session::start_on(Machine::first_run());
        s.key(Key::Escape);
        let parts: Vec<String> = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).collect();
        let cube = s.word("name-sample-cube");
        assert!(parts == [cube.clone()], "a first start holds the part(s) {parts:?}, not {cube:?}");
        let kinds: Vec<String> = s.document().features.iter().map(|f| f.kind.clone()).collect();
        assert!(kinds == ["Sketch", "Extrude"], "the cube is made of {kinds:?}, not a sketch and its extrusion");
        let bodies: Vec<f64> = s.document().bodies.iter().filter(|b| !b.consumed).map(|b| b.volume).collect();
        assert!(bodies.len() == 1 && (bodies[0] - 8000.0).abs() < 1.0, "the cube holds {bodies:?}, not one body of 8000");
    }
}

probe! {
    /// A LATER START WITH NOTHING TO REOPEN HOLDS AN EMPTY ASSEMBLY, not a part nobody asked for.
    fn a_later_start_holds_no_empty_part() {
        let mut s = Session::start();
        s.key(Key::Escape);
        let parts: Vec<String> = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).collect();
        assert!(parts.is_empty(), "a start holds the part(s) {parts:?} with nothing in them");
    }
}

/// A BLOCK WRITTEN AS AN STL, and its path.
fn an_stl_of_a_block(name: &str) -> String {
    let path = format!("{}/reported-{name}-{}.stl", std::env::temp_dir().display(), std::process::id());
    let _ = std::fs::remove_file(&path);
    let mut s = Session::start();
    build::block(&mut s);
    let (file, export) = (s.word("menu-file"), s.word("file-export"));
    s.menu(&[&file, &export, "STL\u{2026}"]);
    let quality = s.word("stl-standard");
    if s.shows(&quality) {
        s.press_word(&quality);
    }
    s.answer_file(&path);
    assert!(std::path::Path::new(&path).exists(), "the block was not written as an STL: {:?}", s.status());
    path
}

/// The heading of the import sources in the tree, for `n` of them.
fn sources_heading(s: &Session, n: usize) -> String {
    s.word("tree-import-sources").replace("{ $n }", &n.to_string()).replace("{$n}", &n.to_string())
}

probe! {
    /// AN IMPORT SOURCE GOES WITH ITS PART: the part a file was brought in as is deleted, and the file kept for it
    /// does not stay behind in the tree as a row nothing can be done with.
    ///
    /// The hole: the tree was checked without its section of import sources.
    fn an_import_source_goes_with_its_part() {
        let path = an_stl_of_a_block("goes-with-its-part");
        let mut s = Session::start();
        s.key(Key::Escape);
        build::import(&mut s, &path);
        let one = sources_heading(&s, 1);
        assert!(s.words().iter().any(|w| w.contains(&one)), "a file was brought in and the tree shows no import source; on screen: {:?}", s.words());
        let before: Vec<String> = s.document().parts.iter().map(|p| p.name.clone()).collect();
        let came = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).next_back().expect("the part the file came in as");
        let row = s.find(&came, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {came:?} is not in the tree; on screen: {:?}", s.words()));
        s.click(row.center());
        s.key(Key::Delete);
        let yes = s.word("confirm-yes");
        if s.shows(&yes) {
            s.press_word(&yes);
        }
        assert!(s.document().parts.len() < before.len(), "the part {came:?} was not deleted: {:?}", s.document().parts);
        assert!(!s.words().iter().any(|w| w.contains(&one)), "the part the file came in as was deleted, and its import source still hangs in the tree; on screen: {:?}", s.words());
    }
}

probe! {
    /// AN IMPORT SOURCE CAN BE BROUGHT IN AGAIN: the row of a kept file offers something to do with it besides
    /// deleting it - the file is kept precisely so that it can be read again.
    ///
    /// The hole: the tree was checked without its section of import sources.
    fn an_import_source_can_be_brought_in_again() {
        let path = an_stl_of_a_block("brought-in-again");
        let mut s = Session::start();
        s.key(Key::Escape);
        build::import(&mut s, &path);
        let one = sources_heading(&s, 1);
        let heading = s.words_at().into_iter().find(|(w, _)| w.contains(&one)).map(|(_, r)| r).unwrap_or_else(|| panic!("the tree shows no import source; on screen: {:?}", s.words()));
        s.click(heading.center());
        // the row of the file stands under its heading; what can be pressed on it, besides the bin
        let row_y = heading.max.y + 10.0;
        let offered: Vec<String> = s
            .widgets()
            .into_iter()
            .filter(|w| matches!(w.kind, qymcad::Kind::Button | qymcad::Kind::Link | qymcad::Kind::Other) && w.rect.min.y >= heading.max.y && w.rect.min.y < row_y + 20.0 && w.rect.max.x < heading.max.x + 200.0)
            .map(|w| w.label)
            .filter(|l| !l.is_empty())
            .collect();
        // and the right button on the row: a menu of what can be done with the file shows new words
        let at = qymcad::pos2(heading.min.x + 40.0, row_y + 4.0);
        let before = s.words();
        s.click_with(at, PointerButton::Secondary, Modifiers::default());
        let menu: Vec<String> = s.words().into_iter().filter(|w| !before.contains(w)).collect();
        assert!(!offered.is_empty() || !menu.is_empty(), "the row of a kept file offers nothing but the bin - no way to read it again; on screen: {:?}", s.words());
        // and bringing it in again brings the block in again: one more part holding what the block held
        if !menu.is_empty() {
            s.key(Key::Escape);
        }
        let parts = s.document().parts.iter().filter(|p| !p.assembly).count();
        let again = s.word("tree-source-again");
        s.press_hint(&again);
        let now = s.document().parts.iter().filter(|p| !p.assembly).count();
        assert!(now == parts + 1, "the kept file was brought in again and the assembly holds {now} parts, it held {parts}; the program says {:?}", s.status());
    }
}

probe! {
    /// EQUAL TIES TWO CIRCLES AS THE CIRCLE TOOL MAKES THEM: drawn with no value typed, each comes with no size of its
    /// own, and Equal gives them one radius without the sketch going over-defined.
    ///
    /// The hole: the constraints were tried on shapes drawn with the automatic constraints turned off - never on the
    /// shapes a person draws, which bring their dimensions with them.
    fn equal_ties_two_circles_the_circle_tool_made() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        build::circle(&mut s, (0.0, 0.0), (10.0, 0.0));
        build::circle(&mut s, (30.0, 0.0), (35.0, 0.0));
        build::pick(&mut s, 0.0, 10.0, false);
        build::pick(&mut s, 30.0, 5.0, true);
        let equal = s.word("con-equal");
        s.press_hint(&equal);
        let said = s.status();
        let sk = s.document().sketches.first().cloned().expect("the sketch of the circles");
        let radius = |s: &mut Session, x: f64, y: f64| match s.sketch_under(x, y) {
            Some(qymcad::SketchPick::Circle { radius, .. }) => radius,
            other => panic!("no circle lies at ({x}, {y}): {other:?}"),
        };
        assert!(sk.redundant == 0, "Equal on two circles as the tool made them left the sketch over-defined ({} redundant); the status line says {said:?}", sk.redundant);
        assert!(said == s.word("sk-constraint-added"), "Equal on two circles as the tool made them did not solve: the status line says {said:?}");
        // the two circles drawn with no size typed carry none: Equal brings them to one radius, the top of the sketch
        let (a, b) = (radius(&mut s, 0.0, sk.max[1]), radius(&mut s, 30.0, sk.max[1]));
        assert!((a - b).abs() < 1e-6, "Equal was taken and the circles are of radius {a} and {b}");
    }
}

probe! {
    /// EVERY MOUSE LAYOUT MOVES AND SCALES THE SHEET OF A SKETCH as it moves and scales the space: a sheet has
    /// nothing to turn, so what a layout owes it is its gesture of moving and its gesture of scaling. Every layout is
    /// tried, and what fails is told at the end.
    ///
    /// The hole: the layouts were tried in the 3D view alone.
    fn every_mouse_layout_moves_and_scales_the_sketch_sheet() {
        use qymcad_acceptance::mouse::{choose_the_layout, make, scale_up, LAYOUTS};
        let mut problems: Vec<String> = Vec::new();
        for layout in &LAYOUTS {
            let mut s = Session::start();
            s.key(Key::Escape);
            choose_the_layout(&mut s, layout.code);
            build::into_the_first_part(&mut s);
            let xy = s.word("plane-xy-table");
            s.press_word(&xy);
            let seen = |s: &mut Session| [(0.0, 0.0), (40.0, 0.0)].map(|(x, y)| s.seen_on_sketch(x, y));
            let middle = s.canvas().center();
            // MOVING: both points of the sheet go the way the pointer went
            let before = seen(&mut s);
            let to = qymcad::pos2(middle.x + 120.0, middle.y + 50.0);
            make(&mut s, layout.pan, middle, to);
            let after = seen(&mut s);
            match (before, after) {
                ([Some(a0), Some(b0)], [Some(a1), Some(b1)]) => {
                    let (ma, mb) = (a1 - a0, b1 - b0);
                    if ma.length() < 20.0 || (ma - mb).length() > 1.0 {
                        problems.push(format!("{}: its gesture of moving ({:?}) moved the sheet by {ma:?} and {mb:?}, the pointer went {:?}", layout.code, layout.pan, to - middle));
                    }
                }
                other => problems.push(format!("{}: the points of the sheet are not on the canvas to watch: {other:?}", layout.code)),
            }
            // SCALING: the two points come further apart
            let before = seen(&mut s);
            let centre = s.canvas().center();
            scale_up(&mut s, layout, centre);
            let after = seen(&mut s);
            if let ([Some(a0), Some(b0)], [Some(a1), Some(b1)]) = (before, after) {
                let (was, now) = ((b0 - a0).length(), (b1 - a1).length());
                if now < was * 1.1 {
                    problems.push(format!("{}: its gesture of scaling left the sheet at {now} points for 40 mm, it was {was}", layout.code));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}

probe! {
    /// A NEW PROJECT FROM THE WINDOW OF PROJECTS ASKS NOTHING ABOUT THE ONE JUST MADE: File -> New project gives a new,
    /// untouched project; where the window of where to start then stands, its New part makes a project again - with no
    /// question about saving the untouched one, and one project after it, not two.
    ///
    /// Reported behaviour: a new project is made and the window of projects comes up at once; New project in it asks
    /// to save the project just made, and makes another one.
    fn a_new_project_from_the_window_of_projects_asks_nothing() {
        let mut s = Session::start();
        s.key(Key::Escape);
        let (file, new) = (s.word("menu-file"), s.word("file-new"));
        s.menu(&[&file, &new]);
        let question = s.word("nav-unsaved-title");
        // the first start's own document calls itself unsaved (finding 1): that question is answered as a person
        // answers it, so that what comes after it can be seen
        let dont_save = s.word("nav-dont-save");
        if let Some(b) = s.find(&dont_save, qymcad::pos2(0.0, 0.0)) {
            s.click(b.center());
        }
        assert!(!s.shows(&question), "the question about saving stays after it was answered");
        let start = s.word("start-title");
        let window = s.shows(&start);
        let made = s.document();
        assert!(!made.unsaved && made.undo.is_empty(), "File -> New project makes a project already unsaved, with steps to undo that nobody took: {:?} - the next New project will ask to save it (the window of where to start {})", made.undo, if window { "came up" } else { "did not come up" });
        if window {
            let part = s.word("start-new-part");
            s.press_word(&part);
            assert!(!s.shows(&question), "New part in the window of where to start asks to save the project File -> New project has just made");
        }
    }
}

/// Back to the assembly by its word at the head of the path.
fn to_the_assembly(s: &mut Session) {
    let assembly = s.word("wb-assembly");
    s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
}

/// Two parts, each a block: the first part's block, then a new part with a 10 by 10 by 10 block at (60, 0).
fn two_parts() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    build::into_a_new_part(&mut s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    build::draw(&mut s, "tb-rect-hint", &[(60.0, 0.0), (70.0, 10.0)]);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude).key(Key::Enter);
    s
}

probe! {
    /// A PART KEEPS ITS COLOUR IN THE ASSEMBLY: each of two parts is drawn in one colour inside itself, and in the
    /// same colour when the assembly is shown - as the card is handed it, which is what a screen shows.
    ///
    /// Reported behaviour: a part is made in one colour, and back in the assembly it has another.
    fn a_part_keeps_its_colour_in_the_assembly() {
        let mut s = two_parts();
        let second = s.drawn();
        to_the_assembly(&mut s);
        let both = s.drawn();
        let first_name = s.document().parts[0].name.clone();
        let row = s.find(&first_name, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the first part is not in the tree"));
        std::thread::sleep(std::time::Duration::from_millis(700));
        s.double_click(row.center());
        let first = s.drawn();
        let colours = |d: &[([u8; 3], bool)]| d.iter().map(|(c, _)| *c).collect::<Vec<_>>();
        let mut apart = colours(&both);
        apart.dedup();
        assert!(apart.len() >= 2, "the assembly draws its two parts in one colour, {apart:?}: nothing tells them apart, and the check would see nothing");
        for (inside, what) in [(&first, "the first part"), (&second, "the second part")] {
            let own: Vec<[u8; 3]> = colours(inside);
            assert!(own.iter().all(|c| colours(&both).contains(c)), "{what} is drawn {own:?} inside itself, and the assembly draws its parts {:?}", colours(&both));
        }
    }
}

probe! {
    /// A PART STEPPED INTO IS NOT DRAWN AS PICKED: the part entered with a double click on its row is drawn in its own
    /// colour, not lit as the selection is - nothing was picked.
    ///
    /// Reported behaviour: on entering a part it is lit in 3D as if selected, and nothing clears it.
    fn a_part_stepped_into_is_not_drawn_as_picked() {
        let mut s = two_parts();
        to_the_assembly(&mut s);
        // the check can see a lit part: the first one clicked in the assembly is drawn as picked
        let body = s.face_at([20.0, 15.0, 10.0]);
        s.click(body);
        assert!(s.drawn().iter().any(|(_, lit)| *lit), "a part clicked in the assembly is not drawn as picked - the check would see nothing: {:?}", s.drawn());
        s.key(Key::Escape);
        let first_name = s.document().parts[0].name.clone();
        let row = s.find(&first_name, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the first part is not in the tree"));
        std::thread::sleep(std::time::Duration::from_millis(700));
        s.double_click(row.center());
        let drawn = s.drawn();
        assert!(!drawn.is_empty(), "the part stepped into shows nothing");
        assert!(drawn.iter().all(|(_, lit)| !lit), "the part stepped into is drawn as picked: {drawn:?}");
    }
}

probe! {
    /// AN ASSEMBLY BROUGHT IN FROM A STEP FILE, STEPPED INTO: every component entered with a double click on its row is
    /// drawn in the colour the assembly draws it in, and nothing in it is lit as picked.
    fn the_components_of_a_step_assembly_keep_their_look_when_stepped_into() {
        let mut s = Session::start();
        s.key(Key::Escape);
        build::import(&mut s, &format!("{}/../qymcad-kernel/tests/data/assembly.step", env!("CARGO_MANIFEST_DIR")));
        let whole = s.drawn();
        assert!(whole.len() >= 2, "the assembly brought in draws {} pieces", whole.len());
        let colours: Vec<[u8; 3]> = whole.iter().map(|(c, _)| *c).collect();
        let names: Vec<String> = s.document().parts.iter().map(|p| p.name.clone()).collect();
        let mut problems = Vec::new();
        let mut entered = 0;
        let top = names.first().cloned().unwrap_or_default();
        for name in &names {
            to_the_assembly(&mut s);
            // a component of the assembly brought in is reached through it: its row is folded under the assembly's
            if s.find(name, qymcad::pos2(0.0, 300.0)).is_none() {
                if let Some(row) = s.find(&top, qymcad::pos2(0.0, 300.0)) {
                    std::thread::sleep(std::time::Duration::from_millis(700));
                    s.double_click(row.center());
                }
            }
            let Some(row) = s.find(name, qymcad::pos2(0.0, 300.0)) else { continue };
            std::thread::sleep(std::time::Duration::from_millis(700));
            s.double_click(row.center());
            if s.document().context != *name {
                continue;
            }
            entered += 1;
            let inside = s.drawn();
            if inside.iter().any(|(_, lit)| *lit) {
                problems.push(format!("{name:?} stepped into is drawn as picked: {inside:?}"));
            }
            if let Some((c, _)) = inside.iter().find(|(c, _)| !colours.contains(c)) {
                problems.push(format!("{name:?} stepped into is drawn {c:?}, a colour the assembly draws nothing in ({colours:?})"));
            }
        }
        assert!(entered >= 2, "only {entered} of the components {names:?} could be stepped into - the check saw too little");
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}

probe! {
    /// A DRAWING KEPT AS A SOURCE IS BROUGHT IN AGAIN FROM ITS ROW: the door asks where its curves go, as File -> Import
    /// asks, and a click on the table lays a second sketch of it. Reported behaviour: a drawing's row said to go through
    /// File -> Import instead.
    fn a_drawing_source_is_brought_in_again_from_its_row() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(format!("{}/../../examples/plate.dxf", env!("CARGO_MANIFEST_DIR")));
        let table = s.in_space([0.0, 0.0, 0.0]);
        s.click(table);
        let finish = s.word("wb-finish");
        s.press_word_near(&finish, qymcad::pos2(0.0, 0.0));
        let sketches = s.document().sketches.len();
        let one = sources_heading(&s, 1);
        let heading = s.words_at().into_iter().find(|(w, _)| w.contains(&one)).map(|(_, r)| r).unwrap_or_else(|| panic!("the tree shows no import source; on screen: {:?}", s.words()));
        s.click(heading.center());
        let again = s.word("tree-source-again");
        s.press_hint(&again);
        let table = s.in_space([0.0, 0.0, 0.0]);
        s.click(table);
        let now = s.document().sketches.len();
        assert!(now == sketches + 1, "the drawing was brought in again and the document holds {now} sketches, it held {sketches}; the program says {:?}", s.status());
    }
}

probe! {
    /// A CHAMFER EXACTLY AS WIDE AS THE FACE IT RUNS ACROSS BUILDS, by each of its modes. Reported behaviour: a step
    /// stood 4 from the edge, the chamfer of that edge took 3.99999 and refused 4.
    ///
    /// The block 40 x 30 x 10 has a step cut across its top 5 deep from y = 4 back, so the top face left in front of
    /// it is 4 wide; the chamfer of its front edge with a leg of 4 reaches the edge of the step exactly and eats that
    /// face whole: 12000 - 40 * 26 * 5 = 6800 of the stepped block, less 4 * 4 / 2 * 40 = 320 for the chamfer.
    fn a_chamfer_exactly_to_a_step_builds() {
        let mut problems: Vec<String> = Vec::new();
        for mode in ["cmd-symmetric", "cmd-two-distances", "cmd-leg-angle"] {
            let mut s = Session::start();
            build::block(&mut s);
            qymcad_acceptance::bodies::on_top(&mut s, |s| build::draw(s, "tb-rect-hint", &[(-2.0, 4.0), (42.0, 32.0)]), Some("cmd-cut"), "5");
            let hint = s.word("tb-chamfer-body-hint");
            s.press_hint(&hint);
            let edge = s.edge_at([20.0, 0.0, 10.0]);
            s.click(edge);
            qymcad_acceptance::bodies::bar(&mut s, mode);
            let leg = s.word("f-leg");
            s.fill(&leg, "4");
            // by two legs a person types both; by a leg and an angle the angle of the mode stays
            if mode == "cmd-two-distances" {
                let second = s.word("cmd-leg2");
                s.fill(&second, "4");
            }
            s.key(Key::Enter);
            let doc = s.document();
            let Some(chamfer) = doc.features.iter().rev().find(|f| f.kind == "Chamfer") else {
                problems.push(format!("{mode}: no chamfer in the timeline; the program says {:?}", s.status()));
                continue;
            };
            if let Some(e) = &chamfer.error {
                problems.push(format!("{mode}: the chamfer of 4 is red - {e}"));
                continue;
            }
            let volumes: Vec<f64> = doc.bodies.iter().filter(|b| !b.consumed).map(|b| b.volume).collect();
            if !(volumes.len() == 1 && (volumes[0] - 6480.0).abs() < 1.0) {
                problems.push(format!("{mode}: the part holds {volumes:?}, not one body of 6480"));
            }
        }
        assert!(problems.is_empty(), "\nA CHAMFER OF 4 TO A STEP 4 AWAY ({}):\n{}", problems.len(), problems.join("\n"));
    }
}
