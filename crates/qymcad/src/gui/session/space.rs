//! THE CANVAS AND THE SPACE OF A SESSION: points of a sketch and of the world, the cube, gizmos, faces, edges, vertices.

use super::*;

impl Session {
    /// THE CANVAS - the sketch sheet or the 3D view - where the window laid it out.
    pub fn canvas(&mut self) -> Rect {
        self.settle();
        self.app.viewing.view_rect
    }

    /// WHERE THE POINT (x, y) OF THE SKETCH OPEN FOR EDITING STANDS ON SCREEN, on the canvas as the window laid it
    /// out. A point out of view is brought into it first, the way a person does it: the sheet is dragged with the
    /// middle button.
    ///
    /// # Panics
    /// When no sketch is open on the flat canvas, or the point cannot be brought into view.
    pub fn on_sketch(&mut self, x: f64, y: f64) -> Pos2 {
        self.settle();
        assert!(!self.app.viewing.mode_3d && qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses).is_some(), "no sketch is open on the flat canvas");
        let at = |s: &Session| qymcad_ui_state::Sheet { view: s.app.viewing.view, rect: s.app.viewing.view_rect }.at(qymcad_core::geom::Point2::new(x, y));
        self.bring_into_view(&at, &[PointerButton::Middle], Modifiers::default());
        at(self)
    }

    /// WHERE THE POINT (x, y) OF THE FLAT SHEET IS CLICKED, whatever sketch the sheet shows - one open for editing, or
    /// a finished one a tool lays flat for its contours to be picked - brought into view as [`Session::on_sketch`]
    /// brings it.
    ///
    /// # Panics
    /// When the canvas is not the flat sheet, or the point cannot be brought into view.
    pub fn on_sheet(&mut self, x: f64, y: f64) -> Pos2 {
        self.settle();
        assert!(!self.app.viewing.mode_3d, "the canvas is not the flat sheet");
        let at = |s: &Session| qymcad_ui_state::Sheet { view: s.app.viewing.view, rect: s.app.viewing.view_rect }.at(qymcad_core::geom::Point2::new(x, y));
        self.bring_into_view(&at, &[PointerButton::Middle], Modifiers::default());
        at(self)
    }

    /// WHERE THE POINT (x, y) OF THE OPEN SKETCH IS SEEN NOW, with the sheet left as it stands - what the eye reads
    /// off the screen while the sheet is moved or scaled. `None` when it falls outside the canvas or no sketch is open
    /// on the flat canvas. Unlike [`Session::on_sketch`] it moves nothing.
    pub fn seen_on_sketch(&mut self, x: f64, y: f64) -> Option<Pos2> {
        self.settle();
        if self.app.viewing.mode_3d || qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses).is_none() {
            return None;
        }
        let at = qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.app.viewing.view_rect }.at(qymcad_core::geom::Point2::new(x, y));
        self.app.viewing.view_rect.contains(at).then_some(at)
    }

    /// CLICK THE POINT (x, y) OF THE OPEN SKETCH, brought into view as [`Session::on_sketch`] brings it.
    pub fn click_on_sketch(&mut self, x: f64, y: f64) -> &mut Self {
        let at = self.on_sketch(x, y);
        self.click(at)
    }

    /// DRAG ON THE OPEN SKETCH from the point `from` to the point `to` with the left button: both are brought into
    /// view together first - a place worked out before the view moved is a place on another part of the sheet.
    ///
    /// # Panics
    /// When no sketch is open, or the two points do not fit on the canvas at once.
    pub fn drag_on_sketch(&mut self, from: (f64, f64), to: (f64, f64)) -> &mut Self {
        let mid = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
        self.on_sketch(mid.0, mid.1);
        let sheet = qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.app.viewing.view_rect };
        let (a, b) = (sheet.at(qymcad_core::geom::Point2::new(from.0, from.1)), sheet.at(qymcad_core::geom::Point2::new(to.0, to.1)));
        let canvas = self.app.viewing.view_rect;
        assert!(canvas.contains(a) && canvas.contains(b), "{from:?} and {to:?} do not fit on the canvas at once ({a:?}, {b:?} on {canvas:?}): zoom out first");
        self.drag(a, b, PointerButton::Primary, Modifiers::default())
    }

    /// THE GLYPH OF THE SNAP SHOWN AT THE POINTER, as the frame draws it - none where the pointer snaps to nothing a
    /// click would be tied to.
    pub fn snap_glyph(&mut self) -> Option<SnapGlyph> {
        self.settle();
        qymcad_pick::snap_infer_glyph(&self.app.painting()).map(|(g, p)| SnapGlyph { glyph: format!("{g:?}"), at: (p.x, p.y) })
    }

    /// THE BADGES OF THE CONSTRAINTS DRAWN ON THE OPEN SKETCH, as the frame draws them; none outside a sketch.
    pub fn badges(&mut self) -> Vec<Badge> {
        self.settle();
        let Some(si) = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses) else { return Vec::new() };
        qymcad_pick::visible_constraint_glyphs(&self.app.painting(), self.app.viewing.view_rect, si).into_iter().map(|(_, at, g)| Badge { glyph: format!("{g:?}"), at }).collect()
    }

    /// WHAT A CLICK AT (x, y) OF THE OPEN SKETCH WOULD TAKE, brought into view as [`Session::on_sketch`] does.
    pub fn sketch_under(&mut self, x: f64, y: f64) -> Option<SketchPick> {
        let pos = self.on_sketch(x, y);
        let si = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses)?;
        let (kind, id) = qymcad_sketch::sketch_hit(&self.app.pick_ctx(), self.app.viewing.view_rect, pos, si)?;
        let sk = &self.app.project.sketches[si];
        let pt = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y)).unwrap_or((f64::NAN, f64::NAN));
        if kind == 0 {
            return Some(SketchPick::Point { at: pt(id) });
        }
        use qymcad_core::model::EntityKind;
        if let Some(e) = sk.entities.iter().find(|e| e.id == id) {
            return Some(match e.kind {
                EntityKind::Line { a, b } => SketchPick::Line { from: pt(a), to: pt(b) },
                EntityKind::Arc { center, a, b, ccw } => SketchPick::Arc { centre: pt(center), from: pt(a), to: pt(b), ccw },
                EntityKind::Circle { center, r } => SketchPick::Circle { centre: pt(center), radius: r },
                EntityKind::Ellipse { c, ma, mi } => SketchPick::Ellipse { centre: pt(c), major: pt(ma), minor: pt(mi) },
            });
        }
        let spline = sk.splines.iter().find(|sp| sp.points.contains(&id))?;
        Some(SketchPick::Spline { nodes: spline.points.iter().map(|p| pt(*p)).collect() })
    }

    /// WHERE THE POINT `p` OF THE 3D VIEW FALLS ON SCREEN AS THE VIEW STANDS NOW - no gesture, nothing brought into view,
    /// covered or not: to see whether the view shows the whole of something.
    pub fn projected(&mut self, p: [f64; 3]) -> Pos2 {
        self.settle();
        let basis = self.app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis }.at(p).0
    }

    /// WHERE THE POINT `p` OF THE 3D VIEW STANDS ON SCREEN, brought into view first with the gesture that moves
    /// the view under the person's mouse settings.
    ///
    /// # Panics
    /// When the view is not 3D, the point cannot be brought into view, or something nearer to the eye covers it.
    pub fn in_space(&mut self, p: [f64; 3]) -> Pos2 {
        self.settle();
        assert!(self.app.viewing.mode_3d, "the canvas is not the 3D view");
        let at = move |s: &Session| {
            let basis = s.app.viewing.cam.basis();
            qymcad_ui_state::Screen { cam: &s.app.viewing.cam, set: &s.app.set, rect: s.app.viewing.view_rect, basis: &basis }.at(p).0
        };
        let pan = self.app.set.mouse_nav.pan();
        // any button of ours moves the view with Shift, and the middle one does it anywhere: Shift and the left one, begun
        // on a part in an assembly, carries the part
        let buttons: Vec<PointerButton> = if pan.any_button { vec![PointerButton::Middle] } else { pan.buttons.to_vec() };
        let modifiers = Modifiers { shift: pan.shift, ctrl: pan.ctrl, alt: pan.alt, command: pan.ctrl, mac_cmd: false };
        self.bring_into_view(&at, &buttons, modifiers);
        let pos = at(self);
        let (tol, depth) = self.pixel_and_depth(p);
        if let Some((hit, _, face)) = qymcad_pick::face_under_cursor(&self.app.painting(), self.app.viewing.view_rect, pos) {
            assert!(hit >= depth - 4.0 * tol, "{p:?} is hidden: a face around {:?} lies {} mm in front of it; turn the view first", face.centroid, depth - hit);
        }
        if let Some(w) = self.covering(pos) {
            panic!("{p:?} is covered on screen by {:?} {:?}: a click there lands on it, not on the model", w.kind, w.label);
        }
        pos
    }

    /// WHAT OF THE WINDOW STANDS OVER THE CANVAS AT `pos` - a button, a field, a panel - and so takes a click aimed at
    /// the model there. A widget takes a click that falls within its reach of it, not only on it: 1.4 px above the
    /// Apply button of a tool's popup the click went to the popup, and the edge under it was not picked.
    pub(crate) fn covering(&mut self, pos: Pos2) -> Option<Widget> {
        let canvas = self.app.viewing.view_rect;
        let reach = self.win.ctx.global_style().interaction.interact_radius;
        self.win.widgets.iter().find(|w| w.rect.expand(reach).contains(pos) && w.rect != canvas).cloned()
    }

    /// WHERE THE POINT `p` OF THE WORLD IS SEEN NOW, with the view left as it stands - what the eye reads off the
    /// screen while the view is being turned, moved or scaled. `None` when it falls outside the canvas or the
    /// canvas is the flat sheet. Unlike [`Session::in_space`] it moves nothing.
    pub fn seen_at(&mut self, p: [f64; 3]) -> Option<Pos2> {
        self.settle();
        if !self.app.viewing.mode_3d {
            return None;
        }
        let basis = self.app.viewing.cam.basis();
        let pos = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis }.at(p).0;
        self.app.viewing.view_rect.contains(pos).then_some(pos)
    }

    /// WHERE THE SIDE OF THE NAVIGATION CUBE THAT READS `side` IS CLICKED - TOP, FRONT and the rest. The captions
    /// of the cube are baked into the drawing of it, so they stand on no word of the screen; the eye reads them off
    /// the cube all the same. `None` when no side reads that.
    pub fn cube_side(&mut self, side: &str) -> Option<Pos2> {
        self.settle();
        let cube = self.app.cube_ctx();
        let rect = self.app.viewing.view_rect;
        let zones = super::super::viewcube::zones();
        let i = zones.iter().position(|z| z.label.is_some_and(|k| crate::i18n::tr(k) == side))?;
        Some(super::super::viewcube::zone_center(&cube, rect, i))
    }

    /// WHERE THE NAVIGATION CUBE IS CLICKED TO LOOK FROM `dir` - the way from the model to the eye, each coordinate
    /// -1, 0 or 1: a side of the cube for one axis, an edge of it for two, a corner for three. `None` when that part
    /// of the cube is on its far side, out of the eye's reach, as it stands now: a person turns the cube to it first.
    pub fn cube_toward(&mut self, dir: [i8; 3]) -> Option<Pos2> {
        self.settle();
        let cube = self.app.cube_ctx();
        let rect = self.app.viewing.view_rect;
        let zones = super::super::viewcube::zones();
        let sign = |v: f64| {
            if v > 1e-9 {
                1
            } else if v < -1e-9 {
                -1
            } else {
                0
            }
        };
        let i = zones.iter().position(|z| z.dir.map(sign) == dir)?;
        let at = super::super::viewcube::zone_center(&cube, rect, i);
        (super::super::viewcube::zone_at(&cube, rect, at) == Some(i)).then_some(at)
    }

    /// CAN THE POINT `p` OF SPACE BE SEEN AS THE VIEW STANDS: on the canvas, and no face nearer to the eye covers it.
    /// The view is not moved.
    pub fn sees(&mut self, p: [f64; 3]) -> bool {
        let Some(pos) = self.seen_at(p) else { return false };
        if self.covering(pos).is_some() {
            return false;
        }
        let (tol, depth) = self.pixel_and_depth(p);
        qymcad_pick::face_under_cursor(&self.app.painting(), self.app.viewing.view_rect, pos).is_none_or(|(hit, _, _)| hit >= depth - 4.0 * tol)
    }

    /// WHERE THE GIZMO OF WHAT IS CHOSEN STANDS: the arms to carry it and the rings to turn it. `None` when what
    /// is chosen carries no gizmo, or the canvas is the flat sheet.
    pub fn gizmo(&mut self) -> Option<Gizmo> {
        self.settle();
        if !self.app.viewing.mode_3d {
            return None;
        }
        let app = &self.app;
        let (o, l) = match app.chosen.sel {
            qymcad_ui_state::Sel::Component(i) => {
                let comp = app.project.components.get(i)?.id;
                qymcad_ui_state::gizmo_geometry(app.viewing.cam, app.dragged.comp_giz, &app.project, comp)
            }
            sel => {
                let (_, mi) = qymcad_ui_state::body_gizmo_target(qymcad_ui_state::body_view_of!(app), sel)?;
                qymcad_ui_state::body_gizmo_geometry(&app.dragged.body_giz, app.viewing.cam, &app.project, &app.set, mi)
            }
        };
        let basis = app.viewing.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis };
        // halfway along the arm, and the ring a quarter of the way round from its first axis - the place on it
        // furthest from both arms it lies between
        let arm = |ax: usize| {
            let mut tip = o;
            tip[ax] += l / 2.0;
            scr.at(tip).0
        };
        let ring = |ax: u8| {
            let (u, v) = qymcad_ui_state::ring_axes(ax);
            let (c, s) = (std::f64::consts::FRAC_PI_4.cos(), std::f64::consts::FRAC_PI_4.sin());
            scr.at([o[0] + l * (u[0] * c + v[0] * s), o[1] + l * (u[1] * c + v[1] * s), o[2] + l * (u[2] * c + v[2] * s)]).0
        };
        Some(Gizmo { arms: [arm(0), arm(1), arm(2)], rings: [ring(0), ring(1), ring(2)] })
    }

    /// IS THE CANVAS THE FLAT SHEET? - the sketch drawn flat before the eye, rather than the model in space.
    pub fn flat(&mut self) -> bool {
        self.settle();
        !self.app.viewing.mode_3d
    }

    /// WHERE A FACE THROUGH `p` IS CLICKED: the point in view, and a click there takes a face through `p`.
    ///
    /// # Panics
    /// As [`Session::in_space`] does, and when the surface under the pointer does not pass through `p`.
    pub fn face_at(&mut self, p: [f64; 3]) -> Pos2 {
        let pos = self.in_space(p);
        let (tol, depth) = self.pixel_and_depth(p);
        let hit = qymcad_pick::face_under_cursor(&self.app.painting(), self.app.viewing.view_rect, pos).map(|(d, _, _)| d);
        assert!(hit.is_some_and(|d| (d - depth).abs() < 4.0 * tol), "under the pointer at {pos:?} there is no face through {p:?}: the face there stands at depth {hit:?}, the point at {depth}");
        pos
    }

    /// WHERE AN EDGE THROUGH `p` IS CLICKED: the point in view, and the edge the pointer takes passes through `p`.
    ///
    /// # Panics
    /// As [`Session::in_space`] does, and when the edge under the pointer does not pass through `p`.
    pub fn edge_at(&mut self, p: [f64; 3]) -> Pos2 {
        let pos = self.in_space(p);
        let (tol, _) = self.pixel_and_depth(p);
        let hit = qymcad_pick::pick_edge_point(&self.app.painting(), self.app.viewing.view_rect, pos);
        assert!(hit.is_some_and(|h| dist(h, p) < 4.0 * tol), "under the pointer at {pos:?} there is no edge through {p:?}: the nearest edge point is {hit:?}");
        pos
    }

    /// WHERE THE VERTEX `p` IS CLICKED: the point in view, and the vertex the pointer takes is `p`.
    ///
    /// # Panics
    /// As [`Session::in_space`] does, and when the vertex under the pointer is not `p`.
    pub fn vertex_at(&mut self, p: [f64; 3]) -> Pos2 {
        let pos = self.in_space(p);
        let (tol, _) = self.pixel_and_depth(p);
        let hit = qymcad_pick::pick_vertex_pos(&self.app.painting(), self.app.viewing.view_rect, pos);
        assert!(hit.is_some_and(|h| dist(h, p) < 4.0 * tol), "under the pointer at {pos:?} there is no vertex at {p:?}: the vertex there is {hit:?}");
        pos
    }

    /// `f` DONE ASIDE: what is chosen and the status line are as they were when it returns - for a check that makes
    /// steps of its own between the steps of a person (taking a step back and putting it again, rebuilding
    /// everything) and must leave the next step of the person where the last one left it.
    pub fn aside<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let (sel, multi, status) = (self.app.chosen.sel, self.app.chosen.tree_sel.multi.clone(), self.app.status.clone());
        let out = f(self);
        (self.app.chosen.sel, self.app.chosen.tree_sel.multi, self.app.status) = (sel, multi, status);
        out
    }

    /// HOW MANY THINGS OF THE SKETCH STAND LIT - the count a person would count by eye.
    pub fn lit_in_sketch(&self) -> usize {
        self.app.tools.sel_sk.items.len()
    }

    /// WHICH EDITING BUTTON IS IN HAND, by its own number: 4 the corner fillet, 5 the chamfer, 0 none of them.
    pub fn editing_tool(&self) -> u8 {
        self.app.tools.armed.click_op()
    }

    /// WHICH DRAWING TOOL OF A SKETCH IS IN HAND, by its own number: 1 the line, 3 the circle, 9 the spline, 0 none.
    pub fn drawing_tool(&self) -> u8 {
        self.app.tools.armed.draw_kind()
    }

    /// Is a text field taking the keys - a person in the middle of typing, where Ctrl+Z is the field's own?
    pub fn typing(&self) -> bool {
        self.win.ctx.egui_wants_keyboard_input()
    }

    /// Is the program asking for a file - a chooser up and waiting for its answer?
    pub fn asking_for_a_file(&self) -> bool {
        self.app.asking_for_a_file()
    }

    /// A COPY OF THE DOCUMENT WRITTEN TO `path`, the document keeping its own place and name - what a check of saving and
    /// opening needs in the middle of a session without changing the session it checks.
    pub fn save_copy(&mut self, path: &str) -> Result<(), String> {
        self.settle();
        // the live bodies go with it, as File -> Save writes them
        let live = &self.app.live;
        let breps: Vec<(qymcad_core::model::Id, Vec<u8>)> = live.shapes.iter().filter_map(|(id, sh)| live.blobs.get(id).cloned().or_else(|| sh.to_brep_bytes()).map(|b| (*id, b))).collect();
        qymcad_io::save_project_guarded_with_brep(&self.app.project, path, &breps).map_err(|e| e.to_string())
    }

    /// A DOCUMENT OF AN OLD VERSION made of the file at `path`: written to a scratch file of this process with its
    /// meshes holding no normals of the surface, as the window saved a document before it kept them. A check of what
    /// the program does with an old document makes its own, and the file it is made of stays free to be saved by any
    /// build. Returns where it was written.
    pub fn old_document_of(path: &str) -> Result<String, String> {
        let mut p = qymcad_io::load_project(path)?;
        for b in p.bodies.iter_mut() {
            b.mesh.norms.clear();
        }
        let out = std::env::temp_dir().join(format!("qymcad-old-document-{}.qcad", std::process::id())).to_string_lossy().into_owned();
        qymcad_io::save_project(&p, &out)?;
        Ok(out)
    }
}
