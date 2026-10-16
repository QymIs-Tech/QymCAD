//! THE ROLLBACK BAR DRAGGED IN THE TREE REBUILDS ONLY WHAT IT MOVED ACROSS, AND THE PART STAYS ON SCREEN MEANWHILE.
//!
//! Reported behaviour: the bar dragged one row up, above a rounding at the end of a part with two threads, and the card
//! "Rebuilding 27 nodes, a thread among them" counted every node of the timeline for minutes. Dragged back down, the
//! part vanished from the viewport while the rounding was computed.
//!
//! The bar is dragged here by the mouse in real frames, as a person drags it, and what follows is watched frame by
//! frame: the kind of every rebuild in flight (a loud one is what puts up the card counting the threads) and the bodies
//! on screen.
#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;
    use qymcad_core::model::Id;
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};

    /// A sleeve of 30 x 20 with a hole of 8.5, M10 inside it, its outer rims rounded last.
    struct Sleeve {
        app: App,
        threaded: Id,
        rounded: Id,
    }

    fn round_edges(app: &mut App, body: Id, r: f64) -> Vec<u32> {
        crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
        app.project.regen_edges.get(&body).map(|es| es.iter().filter(|e| (e.radius - r).abs() < 0.05).map(|e| e.id).collect()).unwrap_or_default()
    }

    /// The sleeve, built; a rebuild in the worker waits `hold_ms` before it starts, when given.
    fn sleeve(hold_ms: Option<u64>) -> Sleeve {
        let mut app = App { project: qymcad_core::model::Project::default(), ..Default::default() };
        app.project.new_document();
        if let Some(ms) = hold_ms {
            app.project.meta.comment = format!("{}{ms}", crate::gui::io_jobs::REBUILD_HOLDS_FOR_TEST);
        }
        let outer = app.project.add_cylinder(15.0, 20.0);
        let hole = app.project.add_cylinder(4.25, 30.0);
        let sleeve = app.project.add_body_boolean(outer, hole, 0);
        let rim = *round_edges(&mut app, sleeve, 4.25).first().expect("setup: the rim of the hole");
        let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, internal: true, fit: 0.0, ..Default::default() };
        let threaded = app.project.add_thread(sleeve, rim, spec, 8.0, 0.0, 0.0);
        let rims = round_edges(&mut app, threaded, 15.0);
        assert!(!rims.is_empty(), "setup: the outer rims of the threaded sleeve");
        let rounded = app.project.add_fillet(threaded, 1.0, rims);
        crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
        assert!(app.project.mesh_index(rounded).is_some(), "setup: the rounding does not build: {:?}", app.project.regen_errors);
        Sleeve { app, threaded, rounded }
    }

    fn node_of(app: &App, body: Id) -> usize {
        app.project.timeline.iter().position(|n| n.kind.body() == Some(body)).expect("the node of the body")
    }

    /// The part of the sleeve stepped into from the tree, as a person steps in to see its timeline.
    fn into_the_part(h: &mut Hand, body: Id) {
        let part = h.app.project.body_owner(body).and_then(|c| h.app.project.components.iter().find(|x| x.id == c)).map(|c| crate::i18n::name(&c.name)).expect("the part of the sleeve");
        assert!(h.double_click_word(&part), "the tree shows the part as {part:?}");
    }

    /// The words of the row of the node `ti` in the tree, its icon aside.
    fn label_of(app: &App, ti: usize) -> String {
        let label: String = crate::gui::panels_tree::feature_row_label(&app.project, ti).chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
        label.trim().to_string()
    }

    /// Where the row of the node `ti` is written in the tree.
    fn row_of(h: &mut Hand, ti: usize) -> egui::Rect {
        let label = label_of(h.app, ti);
        h.written_at(&label).unwrap_or_else(|| panic!("the tree shows no row {label:?}; it shows {:?}", h.words_drawn().iter().map(|w| w.0.clone()).collect::<Vec<_>>()))
    }

    /// What a frame after the release showed: the label of a loud rebuild in flight, and whether a body of the sleeve was
    /// on screen.
    struct Watched {
        loud: Vec<String>,
        /// the warnings egui draws over the ids of widgets in a dev build - red frames over the tree
        clashes: Vec<String>,
        empty_frames: usize,
        frames: usize,
        /// how high the row of the rounding was written, frame by frame: the rows above it must not come and go
        row_tops: Vec<f32>,
    }

    /// The warnings egui drew over the last frame of a dev build: two widgets given one id (in words), and a widget that
    /// took another id at the same place than in the frame before (a red frame of 2 points round it, the red frames
    /// over the tree that were reported).
    fn id_clashes(h: &Hand) -> Vec<String> {
        let mut out: Vec<String> = h.words_drawn().iter().filter(|w| w.0.contains("use of") && w.0.contains(" ID ")).map(|w| w.0.clone()).collect();
        for cs in h.shapes_drawn() {
            if let egui::Shape::Rect(r) = &cs.shape {
                if r.stroke.color == egui::Color32::RED && (r.stroke.width - 2.0).abs() < 1e-3 && r.fill == egui::Color32::TRANSPARENT {
                    out.push(format!("a widget took another id at {:?}", r.rect));
                }
            }
        }
        out
    }

    /// Frames until the program rests, each read for the rebuild in flight and the bodies on screen.
    fn watch_to_rest(h: &mut Hand, bodies: &[Id], row: &str) -> Watched {
        let mut w = Watched { loud: Vec::new(), clashes: Vec::new(), empty_frames: 0, frames: 0, row_tops: Vec::new() };
        let began = std::time::Instant::now();
        let mut rest = 0;
        while rest < 10 && began.elapsed() < std::time::Duration::from_secs(120) {
            h.frame(Vec::new());
            w.frames += 1;
            w.clashes.extend(id_clashes(h));
            if let Some(at) = h.words_drawn().iter().find(|x| x.0 == row) {
                w.row_tops.push(at.1.top());
            }
            let shown = qymcad_ui_state::shown_bodies(&h.app.painting());
            if !shown.iter().any(|s| bodies.contains(&s.1)) {
                w.empty_frames += 1;
            }
            match h.app.regen.busy.as_ref().filter(|b| b.kind == crate::gui::BgKind::Regen) {
                Some(b) if !b.quiet => w.loud.push(b.label.clone()),
                Some(_) => {}
                None if !h.app.regen.wanted => rest += 1,
                None => {}
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        w.loud.dedup();
        w.clashes.sort();
        w.clashes.dedup();
        w
    }

    /// THE TREE STANDS STILL while the rebuild after a move of the bar runs: the row of the rounding is written at one
    /// height in every frame. Reported behaviour: the section of the part's bodies came and went, and everything under
    /// it, the bar held by the cursor included, jumped.
    fn steady(w: &Watched, way: &str) {
        let first = *w.row_tops.first().expect("the row of the rounding was written in no frame");
        let moved: Vec<f32> = w.row_tops.iter().copied().filter(|y| (y - first).abs() > 0.5).collect();
        assert!(moved.is_empty(), "the bar went {way} and the row next to it jumped from {first} to {moved:?} while the rebuild ran");
    }

    /// What a move of the bar is to leave: a quiet rebuild, the part on screen in every frame, the tree still and with
    /// no two widgets of one id.
    fn clean(w: &Watched, way: &str) {
        assert!(w.clashes.is_empty(), "the bar went {way} and frames drew the warnings of egui over the ids of widgets: {:?}", w.clashes);
        steady(w, way);
        assert!(w.loud.is_empty(), "the bar went {way}, and the rebuild after it is the long one: {:?}", w.loud);
        assert_eq!(w.empty_frames, 0, "the bar went {way} and {} of {} frames showed no body of the part", w.empty_frames, w.frames);
    }

    /// The bar, standing right under the last row, dragged one row (20 points) up, above the rounding.
    fn bar_up_over_the_rounding(h: &mut Hand, rounded: Id) {
        let fillet = node_of(h.app, rounded);
        let row = row_of(h, fillet);
        let from = egui::pos2(row.left() + 40.0, row.bottom() + 10.0);
        h.drag_screen(from, from - egui::vec2(0.0, 24.0));
        assert_eq!(h.app.project.rollback, Some(fillet), "setup: the bar dragged from under the rounding one row up does not stand above it");
    }

    #[test]
    fn the_bar_dragged_one_row_up_rebuilds_nothing_above_it() {
        let Sleeve { mut app, threaded, rounded } = sleeve(None);
        let mut h = Hand::new(&mut app);
        into_the_part(&mut h, rounded);
        bar_up_over_the_rounding(&mut h, rounded);
        let row = label_of(h.app, node_of(h.app, rounded));
        let w = watch_to_rest(&mut h, &[threaded, rounded], &row);
        steady(&w, "up");
        assert!(w.loud.is_empty(), "the bar went one row up, above the rounding, and the rebuild after it is the long one: {:?}", w.loud);
        assert!(w.clashes.is_empty(), "the bar went one row up and frames drew two widgets of one id: {:?}", w.clashes);
        assert!(h.app.project.mesh_index(threaded).is_some(), "the bar went above the rounding and the threaded sleeve is gone from the screen");
        assert!(h.app.project.mesh_index(rounded).is_none(), "the bar went above the rounding and the rounding is still on screen");
    }

    /// Back down under the rounding, its rebuild held 800 ms in the worker: the threaded sleeve stays on screen in every
    /// frame until the rounding is in.
    #[test]
    fn the_bar_dragged_back_down_keeps_the_part_on_screen() {
        let Sleeve { mut app, threaded, rounded } = sleeve(Some(800));
        let mut h = Hand::new(&mut app);
        into_the_part(&mut h, rounded);
        bar_up_over_the_rounding(&mut h, rounded);
        let label = label_of(h.app, node_of(h.app, rounded));
        let _ = watch_to_rest(&mut h, &[threaded, rounded], &label);
        // the bar stands now between the last cut and the rounding, right over the rounding's row
        let fillet = node_of(h.app, rounded);
        let row = row_of(&mut h, fillet);
        let from = egui::pos2(row.left() + 40.0, row.top() - 9.0);
        h.drag_screen(from, from + egui::vec2(0.0, 24.0));
        assert_eq!(h.app.project.rollback, None, "setup: the bar dragged one row down does not stand under the rounding");
        let w = watch_to_rest(&mut h, &[threaded, rounded], &label);
        steady(&w, "back down");
        assert!(w.loud.is_empty(), "the bar went back down under the rounding, and the rebuild after it is the long one: {:?}", w.loud);
        assert!(w.clashes.is_empty(), "the bar went back down and frames drew two widgets of one id: {:?}", w.clashes);
        assert_eq!(w.empty_frames, 0, "the bar went back down under the rounding, and {} of {} frames showed no body of the part while it was computed", w.empty_frames, w.frames);
        let shown: Vec<Id> = qymcad_ui_state::shown_bodies(&h.app.painting()).into_iter().map(|s| s.1).collect();
        assert!(shown.contains(&rounded) && !shown.contains(&threaded), "the rounding is computed and the screen shows {shown:?}: the rounding {rounded} alone is to stand");
    }

    /// THE DOCUMENT OF THE REPORT: the tube of the filter, two threads above two cuts. The bar dragged over the last cut
    /// and back: no rebuild is the long one, the tube stays on screen, the tree stands still and draws no two widgets of
    /// one id.
    #[test]
    fn on_the_filter_the_bar_over_the_last_cut_and_back_rebuilds_no_thread() {
        let mut app = App::default();
        crate::gui::io_jobs::spawn_project_load(&mut app.regen, format!("{}/../../examples/Filter-v2.qcad", env!("CARGO_MANIFEST_DIR")));
        app.drain_busy_for_test();
        app.wait_bg();
        let tube = app.project.components.iter().find(|c| c.name == "Tube").map(|c| c.id).expect("the tube of the filter");
        let cut = app.project.timeline.iter().rposition(|n| n.parent == Some(tube) && n.kind.body().is_some()).expect("the last step of the tube");
        let bodies: Vec<Id> = app.project.component_bodies(tube);
        let label = label_of(&app, cut);
        let mut h = Hand::new(&mut app);
        assert!(h.double_click_word("Tube"), "the tree shows no part Tube");
        let row = row_of(&mut h, cut);
        let from = egui::pos2(row.left() + 40.0, row.bottom() + 10.0);
        h.drag_screen(from, from - egui::vec2(0.0, 24.0));
        assert_eq!(h.app.project.rollback, Some(cut), "setup: the bar dragged from under the last cut one row up does not stand above it");
        let up = watch_to_rest(&mut h, &bodies, &label);
        let row = row_of(&mut h, cut);
        let from = egui::pos2(row.left() + 40.0, row.top() - 9.0);
        h.drag_screen(from, from + egui::vec2(0.0, 24.0));
        assert_eq!(h.app.project.rollback, None, "setup: the bar dragged one row down does not stand under the last cut");
        let down = watch_to_rest(&mut h, &bodies, &label);
        clean(&up, "up");
        clean(&down, "back down");
    }
}
