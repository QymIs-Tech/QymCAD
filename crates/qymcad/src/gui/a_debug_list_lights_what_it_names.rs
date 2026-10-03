//! THE DEBUG LIST LIGHTS THE ELEMENT IT NAMES, AND LIGHTS NOTHING ELSE.
//!
//! Reported need: the face, the edge and the corner are named all over the program - in the properties panel, in
//! the words of a failed operation, in the list of a tool - and nowhere were they SHOWN. Both halves of the
//! question could be answered apart and not together, so a person who read "edge 21" could not say which of the
//! hundred edges it was. The picking faults this list was wanted for are exactly of that shape: the name and the
//! picture disagree, and there was no way to put the two side by side.
//!
//! What is checked here is the frame, not the flag: the list is drawn for real, the mouse is led onto a row by the
//! name that element is known by, and the SHAPES OF THE VIEWPORT are counted. A name that is stored and never drawn
//! is a list that answers nothing.
//!
//! ONE WORD ON THE FRAME RATE, because it decided how these are written. A collapsing header OPENS BY ANIMATION,
//! so in the frame after the click its body is still clipped to a fraction of its height: the rows are laid out and
//! their rectangles are honest, but they lie outside the clip and cannot be pressed. Every frame here therefore
//! carries a `predicted_dt` of a tenth of a second and the list is left to open for a few frames before a row is
//! aimed at. Without it the checks fail in a way that looks like a defect of the list - "the row does not answer a
//! click" - while the row is not yet on the screen.

#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_ui_state::Sel;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// One solid block standing in the root, with its faces and its edges from the kernel - the list reads the
    /// document, so a document without them would leave nothing to read.
    fn block(app: &mut App) {
        crate::gui::joint_flow::tests::add_part_at(app, 0.0);
        for _ in 0..4 {
            if qymcad_ui_state::current_ctx_id(&app.active_path, &app.project) == app.project.root {
                break;
            }
            app.exit_context();
        }
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        // THE LIVE SHAPE, without which the drawing of an edge or a corner has no polyline to walk: `draw_piece`
        // reads the edges of the body from the kernel's own copy, not from the document.
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 6.0;
        app.viewing.cam.target = [10.0, 10.0, 5.0];
        // THE SPLASH IS PUT AWAY, as `Hand` does before a gesture: a whole frame draws nothing at all while
        // the greeting is up, so Ctrl+C would be read by no frame and the check would be green over a key that
        // does nothing.
        app.waiting.splash_until = None;
    }

    fn ev(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    /// One frame of the tree panel, with these events in it.
    ///
    /// The tenth of a second is what moves the header's opening along; see the note at the head of this file.
    fn tree_frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
        // THE LIST IS SWITCHED ON, and it is these checks that say so rather than the program: the list is
        // off for a person who has not asked for it, so a check that walked the tree without this line
        // would be reading an empty panel and calling it a panel with nothing on it.
        app.set.show_model_elements = true;
        let mut input = egui::RawInput {
            screen_rect: Some(viewport()),
            predicted_dt: 0.1,
            focused: true,
            ..Default::default()
        };
        input.events.extend(events);
        let _ = ctx.run_ui(input, |c| {
            egui::CentralPanel::default().show(c, |ui| app.build_tree_for_test(ui));
        });
    }

    fn rect_of(app: &App, key: &str) -> Option<egui::Rect> {
        app.tree.debug_row_rects.iter().find(|(k, _)| k == key).map(|(_, r)| *r)
    }

    fn key_of(app: &App, prefix: char) -> String {
        app.tree
            .debug_row_rects
            .iter()
            .map(|(k, _)| k.clone())
            .find(|k| k.starts_with(prefix))
            .unwrap_or_else(|| panic!("the list names no row starting with '{prefix}'"))
    }

    /// THE LIST IS OPENED THE WAY A PERSON OPENS IT - by pressing its header - AND LEFT TO FINISH OPENING.
    fn open_list(app: &mut App, ctx: &egui::Context) {
        tree_frame(app, ctx, vec![]);
        let at = rect_of(app, "header").expect("the list has a header in the frame").center();
        tree_frame(app, ctx, vec![egui::Event::PointerMoved(at), ev(at, true), ev(at, false)]);
        for _ in 0..3 {
            tree_frame(app, ctx, vec![]);
        }
    }

    /// A ROW NAMED `key` IS PRESSED, at the coordinates the frame gave for it.
    ///
    /// The rectangle is taken from the frame rather than from where the panel is known to sit: a test that knew
    /// the layout would break the moment a row is added above the list, and would go on passing a list that moved.
    fn press_row(app: &mut App, ctx: &egui::Context, key: &str) {
        for _ in 0..20 {
            if on_screen(app, key) {
                break;
            }
            scroll_list_down(app, ctx);
        }
        let at = rect_of(app, key).unwrap_or_else(|| panic!("the row '{key}' is not in the frame")).center();
        tree_frame(app, ctx, vec![egui::Event::PointerMoved(at), ev(at, true), ev(at, false)]);
    }

    /// IS THE ROW INSIDE THE LIST, rather than laid out below its edge?
    ///
    /// A row that is not on the screen cannot be pressed, and cannot be seen lit either - so a check that reads a
    /// rectangle alone would be aiming at a place where the cursor does nothing, and would call that a broken row.
    fn on_screen(app: &App, key: &str) -> bool {
        match (rect_of(app, "list"), rect_of(app, key)) {
            (Some(list), Some(r)) => r.center().y > list.top() && r.center().y < list.bottom(),
            _ => false,
        }
    }

    /// THE BRANCH OF THE WANTED KIND IS OPENED, the way a person opens it: by pressing its own header.
    ///
    /// The branches are the three kinds, and only the faces start open - so without this the corners and the edges
    /// would never be reached, and the checks would be green over a list that only ever showed faces.
    fn open_branch_until(app: &mut App, ctx: &egui::Context, prefix: char) {
        // BROKEN OUT OF, AND NOT RETURNED FROM: the frames below are what a person waits for without noticing, and
        // a branch found already open needs them as much as one just opened.
        for _ in 0..20 {
            if on_screen(app, &format!("branch{prefix}")) {
                break;
            }
            let Some(at) = rect_of(app, &format!("branch{prefix}")) else {
                return; // the branch has no header: there is nothing here to reach
            };
            if on_screen(app, &format!("branch{prefix}")) {
                break;
            }
            scroll_list_down(app, ctx);
            let _ = at;
        }
        let Some(at) = rect_of(app, &format!("branch{prefix}")) else { return };
        let at = at.center();
        tree_frame(app, ctx, vec![egui::Event::PointerMoved(at), ev(at, true), ev(at, false)]);
        // THE BRANCH OPENS BY ANIMATION, and a body still opening is clipped: its rows have honest rectangles but
        // cannot be pressed, which looks exactly like a row that ignores a click.
        for _ in 0..3 {
            tree_frame(app, ctx, vec![]);
        }
    }

    /// THE LIST IS SCROLLED DOWN BY THE WHEEL, the way a person scrolls it.
    ///
    /// This is the answer to a worry that shaped the list wrongly once: a scroll area inside the tree panel's own
    /// scroll area was thought not to be scrollable, because the panel outside takes the wheel. It does not - egui
    /// gives a scroll to the innermost area under the cursor and takes the delta away from the one outside it - and
    /// the check is here so that the belief cannot come back and hide half the list behind a button again.
    fn scroll_list_down(app: &mut App, ctx: &egui::Context) -> bool {
        let at = egui::pos2(120.0, 400.0);
        let before = app.tree.debug_row_rects.iter().find(|(k, _)| k.starts_with('f')).map(|(_, r)| r.top());
        for _ in 0..6 {
            let mut evs = vec![egui::Event::PointerMoved(at)];
            for _ in 0..5 {
                // NEGATIVE IS DOWN. egui reads the delta as the way to move the CONTENT: positive carries the
                // content down and reveals what is above, which is a scroll up. The first attempt at this helper
                // pushed the wrong way and looked, from the outside, like a list that cannot be scrolled at all.
                evs.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -120.0), modifiers: Default::default(), phase: egui::TouchPhase::Move });
            }
            tree_frame(app, ctx, evs);
        }
        let after = app.tree.debug_row_rects.iter().find(|(k, _)| k.starts_with('f')).map(|(_, r)| r.top());
        matches!((before, after), (Some(b), Some(a)) if a < b)
    }

    /// HOW MANY SHAPES THE DEBUG HIGHLIGHT DREW. egui hands the shapes back through the context's output, so they
    /// are counted there - and the panel's own shapes are in that number too, which is why the checks compare two
    /// readings against each other rather than against a figure.
    fn drawn_shapes(app: &mut App) -> usize {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
            egui::CentralPanel::default().show(c, |ui| {
                let painter = ui.painter().clone();
                crate::gui::render::draw_debug_pick(&app.painting(), &painter, viewport());
            });
        });
        out.shapes.len()
    }

    /// A FACE NAMED IN THE LIST IS LIT IN THE VIEWPORT, AND LIFTING THE FINGER PUTS THE LIGHT OUT.
    ///
    /// The picture is read twice with the same document and the same viewport: once with no finger on anything and
    /// once with a finger on the face. What the check asks is the DIFFERENCE - a highlight that was drawn all the
    /// time, a wash over one element of the body, would pass a test that only looked for shapes.
    #[test]
    fn a_face_named_in_the_list_is_lit_in_the_viewport() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);
        // THE BASELINE IS READ WITH NOTHING LIT. Asking for the list is enough to leave a finger on something -
        // a press that lands on a row is a press - and a baseline taken over a lit piece counts a shape that the
        // row under test did not draw, which is how a corner that IS drawn came to look as if nothing were.
        app.tree.debug = None;
        let quiet = drawn_shapes(&mut app);

        let key = key_of(&app, 'f');
        press_row(&mut app, &ctx, &key);

        assert!(matches!(app.tree.debug, Some(Sel::Face(..))), "pressing a face row did not put its finger on the face");
        let lit = drawn_shapes(&mut app);
        assert!(lit > quiet, "the face is named but nothing was lit: {lit} shapes against {quiet} with the face named");

        // THE FINGER IS LIFTED BY PRESSING THE SAME ROW AGAIN, not by reaching for the cursor.
        press_row(&mut app, &ctx, &key);
        assert!(app.tree.debug.is_none(), "the light stayed on after the row was pressed a second time");
        assert_eq!(drawn_shapes(&mut app), quiet, "the viewport still carries the highlight of a face nobody is asking about");
    }

    /// THE EDGES AND THE CORNERS ARE NAMED TOO, under the names the picker uses.
    ///
    /// The ids are what `Sel::Edge` carries, so a row and a cursor can be compared word for word - which is the
    /// whole use of the list. Checked on a body that came out of the kernel: a mesh body has no ids at all
    /// (`MeshEdge::id` is 0 for every edge), and a column of zeros would read as a fault that is not there.
    #[test]
    fn the_edges_and_the_corners_are_named_by_their_ids() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);
        open_branch_until(&mut app, &ctx, 'e');
        open_branch_until(&mut app, &ctx, 'p');

        let mut any_edge = false;
        let mut any_corner = false;
        for (k, _) in &app.tree.debug_row_rects {
            if let Some(rest) = k.strip_prefix('e') {
                let (mi, ei) = rest.split_once('_').expect("an edge row is named by its body and its number");
                let mi: usize = mi.parse().expect("the body number");
                let ei: usize = ei.parse().expect("the edge number");
                let body = app.project.mesh_id(mi).expect("the row names a body of the document");
                let edge = app.project.regen_edges.get(&body).and_then(|es| es.get(ei));
                assert!(edge.is_some_and(|e| e.id > 0), "an edge row names no edge of the body: {k}");
                any_edge = true;
            }
            if k.starts_with('p') {
                any_corner = true;
            }
        }
        assert!(any_edge, "the list names no edge at all");
        assert!(any_corner, "the list names no corner at all");
    }

    /// NOTHING IS SELECTED BY THE LIST: the finger is not a choice.
    ///
    /// The list is read while a command is in hand. A row that selected would put an element into the command that
    /// the person reading the list never chose, and the command would go on believing it was asked.
    #[test]
    fn a_row_does_not_touch_the_selection() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);

        let key = key_of(&app, 'f');
        press_row(&mut app, &ctx, &key);

        assert!(matches!(app.chosen.sel, Sel::None), "the debug list selected something for a command that is not asking for it");
        assert!(app.tree.debug.is_some(), "and it did not even light anything, which is worse");
    }

    /// THE FILTER NARROWS THE LIST, so a body with a thousand elements can be read at all.
    ///
    /// The filter is asked by the NAME OF AN EDGE'S ID, because that is how a person who read a number in a
    /// message looks for it.
    #[test]
    fn the_filter_leaves_the_rows_that_name_the_thing_asked_for() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);
        open_branch_until(&mut app, &ctx, 'e');
        let all = app.tree.debug_row_rects.len();

        let id = app
            .project
            .regen_edges
            .values()
            .flatten()
            .find(|e| e.id > 0)
            .expect("the block has an edge with an id")
            .id;
        app.tree.debug_filter = id.to_string();
        tree_frame(&mut app, &ctx, vec![]);

        let rows = &app.tree.debug_row_rects;
        assert!(rows.len() < all, "the filter removed nothing out of {all} rows");
        assert!(
            rows.iter().any(|(k, _)| k.starts_with('e')),
            "the rows of edge {id} are not in the list any more: the filter is not reaching the rows it should"
        );
    }

    /// A ROW OF A CORNER LIGHTS THE CORNER, not the whole edge it belongs to.
    ///
    /// A corner IS the end of an edge, and the drawing knows the difference: an end is a dot, the edge is a line.
    /// The check is that the row names a vertex - were it a whole edge, the list would promise a point and deliver
    /// a line, and a person looking for the corner would find the corner of some other corner's edge.
    #[test]
    fn a_corner_named_in_the_list_is_lit_as_a_corner() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);
        open_branch_until(&mut app, &ctx, 'p');
        // THE BASELINE IS READ WITH NOTHING LIT. Asking for the list is enough to leave a finger on something -
        // a press that lands on a row is a press - and a baseline taken over a lit piece counts a shape that the
        // row under test did not draw, which is how a corner that IS drawn came to look as if nothing were.
        app.tree.debug = None;
        let quiet = drawn_shapes(&mut app);

        let key = key_of(&app, 'p');
        press_row(&mut app, &ctx, &key);

        assert!(matches!(app.tree.debug, Some(Sel::Vertex(..))), "a corner row did not name a corner");
        assert!(drawn_shapes(&mut app) > quiet, "the corner is named and nothing is lit where it is");
    }

    /// THE LIST DOES NOT TAKE THE SCREEN, and neither does the field above it.
    ///
    /// Reported: pressing a row and the field swelling over the whole window, the tree pushed out of sight. The
    /// panel is in a scrolled area, and an area without a height of its own grows to whatever is put into it - so
    /// the check is about the frame the rows landed in, not about a number in the source: a row that reached past
    /// the edge of the window would take the drawing with it.
    ///
    /// The list is pressed first, because a list that is only ever looked at does not grow: the growth and the
    /// pressing have to happen in the same frame for the fault to appear, and a check that only opens the list
    /// would be green over the very thing it was written for.
    #[test]
    fn the_list_stays_inside_the_window() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);

        let key = key_of(&app, 'f');
        press_row(&mut app, &ctx, &key);

        let screen = viewport();
        for (k, r) in &app.tree.debug_row_rects {
            assert!(
                r.left() >= screen.left() - 1.0 && r.right() <= screen.right() + 1.0,
                "the row '{k}' reaches from {:?} to {:?}, past the edge of the window {:?}",
                r.left(),
                r.right(),
                screen
            );
        }
    }

    /// THE THREE KINDS ARE THREE BRANCHES, AND EACH IS FOLDED BY ITSELF.
    ///
    /// Reported: the faces, the edges and the corners wanted as three lists that can be shut. One list of
    /// everything answers "what is element 37?" and nothing else - a person comparing a face with the edges around
    /// it has to read past every face of the body to get there.
    #[test]
    fn the_faces_the_edges_and_the_corners_are_branches_of_their_own() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);

        // THE FACES ARE OPEN WHEN THE LIST OPENS; THE OTHER TWO ARE NOT, and that is what the check reads: a shut
        // branch has a header and no rows, so the two states are told apart by the rows themselves.
        let faces_open = app.tree.debug_row_rects.iter().any(|(k, _)| k.starts_with('f'));
        let edges_shut = !app.tree.debug_row_rects.iter().any(|(k, _)| k.starts_with('e'));
        assert!(faces_open && edges_shut, "the branches do not open and shut as they are meant to: faces {faces_open}, edges shut {edges_shut}");

        // A SHUT BRANCH OPENS ON ITS OWN HEADER, and only that branch: the faces must still be there afterwards,
        // or "opening the corners" has silently closed the faces.
        let before = app.tree.debug_row_rects.iter().filter(|(k, _)| k.starts_with('f')).count();
        open_branch_until(&mut app, &ctx, 'e');
        assert!(app.tree.debug_row_rects.iter().any(|(k, _)| k.starts_with('e')), "the edges branch did not open when its header was pressed");
        assert_eq!(app.tree.debug_row_rects.iter().filter(|(k, _)| k.starts_with('f')).count(), before, "opening the edges shut the faces");

        // AND IT SHUTS AGAIN ON THE SECOND PRESS, which is what "each branch can be folded" means.
        let at = rect_of(&app, "branche").expect("the edges branch has a header").center();
        tree_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), ev(at, true), ev(at, false)]);
        tree_frame(&mut app, &ctx, vec![]);
        tree_frame(&mut app, &ctx, vec![]);
        assert!(!app.tree.debug_row_rects.iter().any(|(k, _)| k.starts_with('e')), "the edges branch stayed open after its header was pressed again");
    }

    /// EVERY BODY IS A BRANCH OF ITS OWN INSIDE THE KIND, with its own elements under its own heading.
    ///
    /// Reported: the three kinds are three lists, and each list ran from one body straight into the next - so
    /// "face 4" and "face 5" were two things on opposite sides of the drawing, and a person comparing a face
    /// with the edges around it had to read past every face of every other body to get there.
    ///
    /// TWO BODIES ARE STANDING IN THE MODEL, because with one body every grouping looks like one group: a check
    /// that cannot tell the bodies apart cannot tell whether the rows are gathered under headings or merely
    /// printed one after another.
    #[test]
    fn every_body_has_a_heading_of_its_own_and_holds_its_own_elements() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);
        let bodies = app.project.bodies.len();
        assert!(bodies >= 2, "setup: the model holds {bodies} bodies, so no two lists could be told apart");

        // THE HEADING NAMES THE BODY AND SAYS HOW MANY FACES IT HAS - the question asked before the rows are
        // read at all, and the answer a person wants when a report speaks of a body with a hundred faces.
        for mi in 0..bodies {
            let want = format!("{} ({})", crate::i18n::name(&app.project.mesh_name(mi)), app.project.bodies[mi].faces.len());
            assert!(text_rect(&ctx, &mut app, &want).is_some(), "nothing on the screen reads '{want}': the list has no heading for body {mi}");
        }
        let head_top = |mi: usize| {
            rect_of(&app, &format!("bodyf{mi}"))
                .unwrap_or_else(|| panic!("the list drew no heading for body {mi}, so its faces belong to nobody"))
                .top()
        };

        // THE ROWS OF A BODY LIE UNDER THAT BODY'S OWN HEADING, and not under another's: the two are told apart
        // by where they are on the screen, which is the whole of the complaint.
        for mi in 0..bodies {
            let below = head_top(mi);
            let until = if mi + 1 < bodies { head_top(mi + 1) } else { f32::MAX };
            let tops: Vec<f32> = app.tree.debug_row_rects.iter().filter_map(|(k, r)| k.starts_with(&format!("f{mi}_")).then_some(r.top())).collect();
            assert!(!tops.is_empty(), "body {mi} has faces in the document and the list names none of them");
            assert!(tops.iter().all(|y| *y >= below && *y < until), "the faces of body {mi} are not all under its own heading");
        }

        // AND FOLDING ONE BODY'S HEADING HIDES THAT BODY'S ROWS AND LEAVES THE OTHER BODY'S ALONE - which is
        // what makes it a branch and not a label.
        let shut = bodies - 1;
        let at = rect_of(&app, &format!("bodyf{shut}")).expect("the heading is in the frame").center();
        tree_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), ev(at, true), ev(at, false)]);
        tree_frame(&mut app, &ctx, vec![]);
        tree_frame(&mut app, &ctx, vec![]);
        assert!(!app.tree.debug_row_rects.iter().any(|(k, _)| k.starts_with(&format!("f{shut}_"))), "the faces of body {shut} stayed after its own heading was pressed");
        assert!(app.tree.debug_row_rects.iter().any(|(k, _)| k.starts_with("f0_")), "folding the last body took the first body's faces with it");
    }

    /// THE FINGER WALKS THE ROWS WITH THE ARROW KEYS.
    ///
    /// Reported need: click an element and there was no way to step to the next one - so a hundred faces had to be
    /// read one press at a time, in the wrong order for comparing neighbours.
    ///
    /// The check walks DOWN twice and UP once, which is the only way to tell a walk from a jump: down-down-back
    /// lands back on the first row, and a list that only ever lit its first row would pass a single step.
    #[test]
    fn the_arrow_keys_walk_the_finger_along_the_rows() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);

        let first = key_of(&app, 'f');
        press_row(&mut app, &ctx, &first);
        let start = app.tree.debug;
        assert!(start.is_some(), "the first row lit nothing to walk from");

        let key = |k: egui::Key| vec![egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }];
        tree_frame(&mut app, &ctx, key(egui::Key::ArrowDown));
        let down = app.tree.debug;
        assert!(down.is_some() && down != start, "the down key did not move the finger off the row it was on");

        tree_frame(&mut app, &ctx, key(egui::Key::ArrowDown));
        let further = app.tree.debug;
        assert!(further.is_some() && further != down, "a second press of the down key went nowhere");

        tree_frame(&mut app, &ctx, key(egui::Key::ArrowUp));
        assert!(app.tree.debug == down, "the up key did not bring the finger back to the row before it");

        // THE FINGER IS LIT WHERE IT STANDS: the walk is visible, not only stored.
        let lit = app.tree.debug;
        app.tree.debug = None;
        let quiet = drawn_shapes(&mut app);
        app.tree.debug = lit;
        assert!(drawn_shapes(&mut app) > quiet, "the finger walked and nothing is lit where it stands");
    }

    /// THE WALK STOPS AT THE ENDS, AND A SHUT BRANCH IS STEPPED OVER.
    ///
    /// A finger that wraps from the last face to the first edge of another body answers a question nobody asked, and
    /// a finger that walked into a shut branch would move onto rows that are not on the screen - the keys must move
    /// the finger exactly as far as the eye can follow.
    #[test]
    fn the_walk_stops_at_the_ends_and_skips_what_is_shut() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);

        // THE EDGES ARE SHUT, so the visible rows are the faces alone: walking must never produce an edge.
        let first = key_of(&app, 'f');
        press_row(&mut app, &ctx, &first);
        let key = |k: egui::Key| vec![egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }];
        for _ in 0..40 {
            tree_frame(&mut app, &ctx, key(egui::Key::ArrowDown));
        }
        assert!(matches!(app.tree.debug, Some(Sel::Face(..))), "the walk left the faces for a branch that is shut: {:?}", app.tree.debug.is_some());

        // AND UP FROM THE FIRST ROW STAYS THERE.
        for _ in 0..40 {
            tree_frame(&mut app, &ctx, key(egui::Key::ArrowUp));
        }
        assert!(app.tree.debug == Some(Sel::Face(0, 0)), "the walk did not stop at the first row");
    }

    /// A CLICK IN THE VIEWPORT PUTS THE FINGER ON WHAT THE CLICK TOOK, so that the keys walk on from there.
    ///
    /// This is the half of the request that has nothing to do with the list: the walk used to be able to begin only
    /// at a row, so the list decided where a walk starts rather than the drawing.
    ///
    /// The click goes through the program, not through the picker: the whole point is what a person's click leaves
    /// behind, and calling the pick functions by hand would answer a different question.
    #[test]
    fn a_click_in_the_viewport_puts_the_finger_where_the_click_took() {
        let mut app = App::default();
        app.project.new_document();
        // STANDING INSIDE THE PART, in the Part workbench: that is where a click takes a FACE. In the root a click
        // takes the body or the component, and there would be no face for the finger to follow - the check would be
        // green over a click that took the wrong thing entirely.
        crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
        app.workbench = super::super::Workbench::Part;
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 6.0;
        app.viewing.cam.target = [10.0, 10.0, 5.0];
        assert!(app.tree.debug.is_none(), "something lit the debug list before anything was clicked");

        // THE TOP FACE OF THE BLOCK, aimed at by its own centre - what a person aims at.
        let mi = 0;
        let centroid = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))
            .map(|f| [f.centroid.x, f.centroid.y, f.centroid.z])
            .expect("the block has a top face");
        let basis = app.viewing.cam.basis();
        let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(centroid).0;

        // TRAP GUARD, as the picking checks carry: there must be something under the cursor, or there is nothing to
        // click and the check below would be green over an empty stage.
        assert!(crate::gui::pick::pick_body_at(&app.painting(), viewport(), at).is_some(), "setup: there is no body under the cursor at {at:?}");

        app.viewport_3d_click_at(at, viewport(), &basis);

        assert!(matches!(app.chosen.sel, Sel::Face(..)), "the click took no face, so there is nothing for the finger to follow");
        assert!(app.tree.debug == Some(app.chosen.sel), "the finger is not on the face the click took, and the arrow keys would walk from somewhere else");

    }

    /// THE LIST NAMES THE EDGES AND THE CORNERS OF A FILE THAT WAS OPENED, not only of one this session built.
    ///
    /// Reported: the model opened, the faces were all there, and the edges and the corners were simply not - on a
    /// model with hundreds of edges in it.
    ///
    /// The cause is a field that is not saved: `project.regen_edges` is filled by the post pass of a rebuild, and
    /// opening a document does not rebuild. So the list was reading the one source of edges that an opened file does
    /// not have. It reads the picker's cache now - the live B-rep, which opening does prepare - and the check stands
    /// in exactly the state a person opens a file in: the derived field cleared by hand, as in
    /// `an_edge_anchor_survives_reopening`.
    #[test]
    fn an_opened_document_still_names_its_edges_and_its_corners() {
        let mut maker = App::default();
        maker.project.new_document();
        block(&mut maker);
        let path = std::env::temp_dir().join("qym-debug-list-reopened.qcad").to_string_lossy().into_owned();
        qymcad_io::save_project(&maker.project, &path).expect("the document was written");

        let mut app = App::default();
        let project = qymcad_io::load_project(&path).expect("the document opened");
        app.finish_project_load(path, project, Vec::new());
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 6.0;
        app.viewing.cam.target = [10.0, 10.0, 5.0];
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
        // THAT IS EXACTLY WHAT AN OPENED DOCUMENT LOOKS LIKE, and the trap guard says so rather than assuming it.
        app.project.regen_edges.clear();
        assert!(app.project.regen_edges.is_empty(), "setup: the derived field is not empty, so the check is not standing in an opened file");

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);
        open_branch_until(&mut app, &ctx, 'e');
        open_branch_until(&mut app, &ctx, 'p');

        let edges = app.tree.debug_row_rects.iter().filter(|(k, _)| k.starts_with('e')).count();
        let corners = app.tree.debug_row_rects.iter().filter(|(k, _)| k.starts_with('p')).count();
        assert!(edges > 0, "the list names no edge of an opened document, and the picker finds them under the cursor");
        assert!(corners > 0, "the list names no corner of an opened document, and the picker takes them under the cursor");
    }

    /// THE RIGHT BUTTON ON A ROW COPIES ITS IDENTIFIER, and Ctrl+C does the same thing.
    ///
    /// Reported need: read the number off the screen and write it into a report - which is where a number gets
    /// mistyped, and a mistyped id in a report is worth nothing. Both gestures are checked and against the same
    /// name, because "the menu copies it and the key does something else" is a failure that looks like two features
    /// working.
    ///
    /// The clipboard is read where a person reads it: in what the frame hands the window. `output.commands` is the
    /// only way a program may touch the clipboard, and the menu is found by its TEXT on the screen - a check that
    /// knew the item's rectangle instead would be green over a menu drawn somewhere else.
    #[test]
    fn the_right_button_on_a_row_copies_its_identifier() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);

        let key = key_of(&app, 'f');
        press_row(&mut app, &ctx, &key);
        let name = crate::gui::panels_tree::debug_identifier(&app.tree).expect("the row pressed names an element");
        assert!(!name.is_empty(), "the identifier copied is empty, and an empty id in a report is worse than none");

        // THE RIGHT BUTTON, down and up on the row.
        let at = rect_of(&app, &key).expect("the row is in the frame").center();
        let secondary = |pressed: bool| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
        tree_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), secondary(true)]);
        tree_frame(&mut app, &ctx, vec![secondary(false)]);
        // THE MENU, found by its words and pressed where they are - as a person does.
        let item = text_rect(&ctx, &mut app, &crate::i18n::tr("tree-debug-copy-id")).unwrap_or_else(|| panic!("the right button on a row opened no menu with the copy in it"));
        let on = item.center();
        let primary = |pressed: bool| egui::Event::PointerButton { pos: on, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        // THE CLICK AND ITS COPY ARE IN THE SAME FRAME. A frame's output is that frame's own, so reading the
        // clipboard out of any other one would be reading a frame in which nothing was asked of it - and the copy
        // would look like a menu that does nothing.
        let mut got = copied(&tree_frame_out(&mut app, &ctx, vec![egui::Event::PointerMoved(on), primary(true)]));
        got = got.or_else(|| copied(&tree_frame_out(&mut app, &ctx, vec![primary(false)])));
        assert_eq!(got.as_deref(), Some(name.as_str()), "the right button on a row did not put its identifier into the clipboard: {got:?}");
    }

    /// CTRL+C COPIES THE IDENTIFIER OF THE ELEMENT UNDER THE FINGER - the whole frame, keys and all.
    ///
    /// egui turns the system's Ctrl+C into `Event::Copy` before the program sees it, and the program reads that
    /// first; that is the event sent here, so the check walks the same path the keyboard does.
    #[test]
    fn ctrl_c_copies_the_identifier_under_the_finger() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        open_list(&mut app, &ctx);

        let key = key_of(&app, 'f');
        press_row(&mut app, &ctx, &key);
        let name = crate::gui::panels_tree::debug_identifier(&app.tree).expect("the row pressed names an element");

        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), events: vec![egui::Event::Copy], ..Default::default() }, |ui| app.draw_frame(ui));
        assert_eq!(copied(&out).as_deref(), Some(name.as_str()), "Ctrl+C did not copy the identifier of the element under the finger");
    }

    /// WITH NOTHING NAMED, CTRL+C IS STILL THE PROGRAM'S OWN COPY.
    ///
    /// The other half of the same change: a shortcut taken over for good would stop a person copying a part the
    /// first time the debug list was used and the finger left on a row. So this asks for the program's own marker
    /// - the ping that says "the system clipboard is ours now" - and finds it still there.
    #[test]
    fn ctrl_c_is_still_the_programs_own_when_no_element_is_named() {
        let mut app = App::default();
        app.project.new_document();
        block(&mut app);
        crate::gui::joint_flow::tests::add_part_at(&mut app, 40.0);
        assert!(crate::gui::panels_tree::debug_identifier(&app.tree).is_none(), "something is named before anything was clicked");

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), events: vec![egui::Event::Copy], ..Default::default() }, |ui| app.draw_frame(ui));
        assert!(
            !copied(&out).is_some_and(|t| t.starts_with(&crate::i18n::tr("tree-debug-face")[..])),
            "Ctrl+C with nothing named copied an element that was never named"
        );
    }

    /// WHAT A FRAME ASKED THE CLIPBOARD TO RECEIVE.
    ///
    /// Read where a window integration reads it: the clipboard is an OUTPUT COMMAND, and the platform output is
    /// where a frame leaves it. A check that looked at the shapes instead would be looking at the picture rather
    /// than at the request - and a menu that opens perfectly would pass while nothing reached the clipboard.
    fn copied(out: &egui::FullOutput) -> Option<String> {
        out.platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
    }

    /// ONE TREE FRAME WITH THESE EVENTS IN IT, and everything the frame left behind: the shapes it drew and
    /// the commands it gave.
    fn tree_frame_out(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        app.set.show_model_elements = true; // the list is on, as in `tree_frame`
        let mut input = egui::RawInput { screen_rect: Some(viewport()), predicted_dt: 0.1, focused: true, ..Default::default() };
        input.events.extend(events);
        ctx.run_ui(input, |c| {
            egui::CentralPanel::default().show(c, |ui| app.build_tree_for_test(ui));
        })
    }

    /// WHERE A PIECE OF TEXT WAS DRAWN IN THE LAST FRAME - which is how a menu is found without knowing where it
    /// opens, and without a check that breaks the moment the menu grows a second item.
    fn text_rect(ctx: &egui::Context, app: &mut App, want: &str) -> Option<egui::Rect> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => {
                    let text: String = t.galley.text().chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
                    out.push((text.trim().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size())));
                }
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
            egui::CentralPanel::default().show(c, |ui| app.build_tree_for_test(ui));
        });
        let mut texts = Vec::new();
        for sh in &out.shapes {
            walk(&sh.shape, &mut texts);
        }
        texts.into_iter().find(|(t, _)| t == want).map(|(_, r)| r)
    }
}
