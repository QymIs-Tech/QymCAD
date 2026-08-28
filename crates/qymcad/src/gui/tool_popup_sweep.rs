//! A POPUP DOES NOT COVER WHAT IS BEING AIMED AT — A SWEEP OVER EVERY PART TOOL.
//!
//! Reported behaviour: the popups sit in the centre and cover part of the geometry being pulled, which
//! makes picking faces awkward.
//!
//! A field at the geometry is the right idea, but for tools that PICK geometry by clicking, "at the
//! geometry" does not mean "on top of it": while the popup lies on the part, one can neither aim at it
//! nor see what is coming out. The rule is simple and is checked here for every tool at once rather
//! than for the one that was complained about: the anchor stands BESIDE the target body.
//!
//! The second rule of the same sweep: if a command has a numeric field, it must have an anchor.
//! Otherwise there is nowhere to show the field — that is exactly how the thickness of the thicken
//! tool went missing, and exactly how both splits stayed silent.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use egui::{Pos2, Rect};

    /// A cube in a part; returns (mesh index, body id).
    fn part_with_cube(app: &mut App) -> (usize, u64) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.sel = Sel::Mesh(mi);
        (mi, body)
    }

    fn viewport() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// The screen box of the body — what the popup has no right to cover.
    fn body_box(app: &App, mi: usize, rect: Rect) -> Rect {
        let basis = app.cam.basis();
        let mut bb: Option<Rect> = None;
        for v in &app.project.bodies[mi].mesh.verts {
            let p = app.project3([v.x, v.y, v.z], rect, &basis).0;
            bb = Some(bb.map_or(Rect::from_min_max(p, p), |r| r.union(Rect::from_min_max(p, p))));
        }
        bb.expect("the body is visible on screen")
    }

    /// Give the command the least selection without which it cannot have an anchor.
    fn arm(app: &mut App, kind: u8, mi: usize, body: u64) {
        let top = app.project.bodies[mi].faces.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()).cloned();
        match kind {
            6 | 23 | 25 | 26 | 28 => {
                if let Some(f) = top {
                    app.gsel.faces.insert(f.id);
                    app.gsel.faces_body = Some(body);
                    if kind == 23 {
                        // a draft also needs a neutral face — any other one
                        if let Some(side) = app.project.bodies[mi].faces.iter().find(|s| s.id != f.id) {
                            app.draft.neutral = side.id;
                        }
                    }
                }
            }
            7 => {
                if let Some(f) = top {
                    let fi = app.project.bodies[mi].faces.iter().position(|x| x.id == f.id).expect("the index of the face");
                    app.sel = Sel::Face(mi, fi);
                }
            }
            27 | 29 => {
                use qymcad_core::feature::{BasePlane, SketchPlane};
                app.split.plane = Some(SketchPlane::World(BasePlane::XY));
            }
            _ => {}
        }
    }

    /// The Part tools that aim by clicking the body. Each has a command number of its own.
    const AIMED_AT_THE_BODY: [u8; 11] = [4, 5, 6, 7, 23, 25, 26, 27, 28, 29, 24];

    /// A COMMAND WITH A NUMERIC FIELD MUST HAVE AN ANCHOR.
    ///
    /// No anchor means no popup, which means there is nowhere to see the field and nowhere to type into
    /// it. The parameter exists all the while and the command looks like it works: that is exactly how
    /// the thickness of the thicken tool went missing.
    #[test]
    fn every_tool_with_a_number_has_a_place_to_show_it() {
        let rect = viewport();
        let mut missing: Vec<String> = Vec::new();
        for kind in AIMED_AT_THE_BODY {
            let mut app = App::default();
            let (mi, body) = part_with_cube(&mut app);
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 9.0;
            app.cam.target = [10.0, 10.0, 5.0];
            app.start_feat_cmd(kind);
            arm(&mut app, kind, mi, body);
            if app.cmd.params.is_empty() {
                continue; // there is no number, so there is nothing to show
            }
            if app.cmd_anchor_screen(rect).is_none() {
                missing.push(format!("{kind} ({} fields)", app.cmd.params.len()));
            }
        }
        assert!(missing.is_empty(), "these commands have a numeric field and no anchor — there is nowhere to show it: {}", missing.join(", "));
    }

    /// THE ANCHOR DOES NOT LIE ON THE BODY BEING AIMED AT.
    #[test]
    fn no_tool_puts_its_popup_over_the_part() {
        let rect = viewport();
        let mut over: Vec<String> = Vec::new();
        for kind in AIMED_AT_THE_BODY {
            let mut app = App::default();
            let (mi, body) = part_with_cube(&mut app);
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 9.0;
            app.cam.target = [10.0, 10.0, 5.0];
            app.start_feat_cmd(kind);
            arm(&mut app, kind, mi, body);
            let Some(anchor) = app.cmd_anchor_screen(rect) else { continue };
            let bb = body_box(&app, mi, rect);
            if bb.contains(anchor) {
                over.push(format!("{kind} (anchor {:.0},{:.0} inside {:.0},{:.0}..{:.0},{:.0})", anchor.x, anchor.y, bb.min.x, bb.min.y, bb.max.x, bb.max.y));
            }
            assert!(rect.contains(anchor), "command {kind}: the anchor went outside the viewport — the popup cannot be seen");
        }
        assert!(over.is_empty(), "the popup lay on the part — and that is what is aimed at next: {}", over.join("; "));
    }

    /// WHERE A HANDLE BELONGS, IT IS THERE.
    ///
    /// It belongs where the number has a DIRECTION in space: the offset of a face, a thickness, the
    /// offset of a split, the radius of a fillet. Without a handle such a tool is half-made — it exists
    /// and yet cannot be worked the way CAD is worked: all that is left is typing a number.
    ///
    /// Two sorts of tool are NOT included here, and both for good reason. The first: fillet, chamfer,
    /// splitting faces — there a batch of elements facing different ways is picked at once, and an
    /// arrow on one of them would show a direction the operation does not have. The second: angles
    /// (draft, revolve, circular pattern) and the steps of patterns — they need a ring rather than an
    /// arrow, which is a different mechanic.
    #[test]
    fn every_tool_with_a_direction_has_a_handle() {
        let rect = viewport();
        let _ = rect;
        let mut missing: Vec<u8> = Vec::new();
        for kind in [6u8, 25, 27, 28] {
            let mut app = App::default();
            let (mi, body) = part_with_cube(&mut app);
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 9.0;
            app.cam.target = [10.0, 10.0, 5.0];
            app.start_feat_cmd(kind);
            arm(&mut app, kind, mi, body);
            if app.face_arrow_geometry().is_none() {
                missing.push(kind);
            }
        }
        assert!(missing.is_empty(), "the number of these tools has a direction and there is nothing to pull it with: {missing:?}");
    }

    /// A TOOL MUST SHOW WHAT IS PICKED.
    ///
    /// Reported about the new face-copy tool: no face can be selected at all. The face WAS being picked
    /// — everything in the model was right — but nothing changed on screen: no preview had been written
    /// for the command. A person cannot tell "it was not picked" from "it was picked and not drawn",
    /// and is not obliged to: to them it is one and the same — the tool does not work.
    ///
    /// So the rule is checked for every tool at once: every command that collects faces by clicking
    /// must have a branch of its own in the drawing of the scene. The check goes over the source —
    /// what is drawn cannot be caught in a headless run — but it catches exactly what was missed.
    #[test]
    fn every_face_tool_shows_what_is_picked() {
        let render = crate::gui::render_source::RENDER;
        // every place where the drawing of the scene looks at the kind of command
        let mut covered: Vec<u8> = Vec::new();
        for line in render.lines() {
            let Some(pos) = line.find("self.cmd.kind") else { continue };
            let tail = &line[pos..];
            for tok in tail.split(|c: char| !c.is_ascii_digit()) {
                if let Ok(k) = tok.parse::<u8>() {
                    covered.push(k);
                }
            }
        }
        let mut blind: Vec<u8> = Vec::new();
        for kind in [6u8, 23, 25, 26, 28, 30, 31] {
            if !covered.contains(&kind) {
                blind.push(kind);
            }
        }
        assert!(blind.is_empty(), "these tools collect faces and show NOTHING — to a person that means \"it does not work\": {blind:?}");
    }

    /// THE DRIVER LIST IS IN THE POPUP OF EVERY PART TOOL.
    ///
    /// Reported: there is no dropdown with a search over parameters and drivers in the popups of the
    /// sketcher and part tools; and separately, that features must have all of this too, not sketches
    /// alone.
    ///
    /// WHY THE CHECK GOES OVER EVERY ONE RATHER THAN THROUGH A SINGLE ONE. It had already been decided
    /// once that "the field in the tool bars is one for all of them, so it is wired everywhere", and
    /// the item was recorded as closed. It turned out that the popup AT THE GEOMETRY draws ITS OWN
    /// fields through its own `focus_edit` — there was no list there at all. That was found only when
    /// the check went over the frame of every tool.
    #[test]
    fn every_part_tool_popup_offers_the_drivers() {
        let rect = viewport();
        let mut without: Vec<String> = Vec::new();
        for kind in AIMED_AT_THE_BODY {
            let mut app = App::default();
            let (mi, body) = part_with_cube(&mut app);
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 9.0;
            app.cam.target = [10.0, 10.0, 5.0];
            // THE DRIVER THAT MUST BE OFFERED.
            app.project.parameters.push(qymcad_core::model::Param { name: "vysota_korpusa".into(), expr: "25".into(), value: 25.0 });
            app.start_feat_cmd(kind);
            arm(&mut app, kind, mi, body);
            if app.cmd.params.is_empty() || app.cmd_anchor_screen(rect).is_none() {
                continue; // there is no number, or nowhere to show it — the neighbouring checks catch that
            }

            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let mut texts: Vec<String> = Vec::new();
            // SEVERAL FRAMES: the area of the list settles into place on the second pass — measured.
            for pass in 0..4 {
                let mut input = egui::RawInput { screen_rect: Some(rect), ..Default::default() };
                if pass == 1 {
                    // TYPE THE NAME LETTER BY LETTER, as a person does: the list opens on typing. The
                    // field of the command takes the focus itself on the first frame and SELECTS the
                    // previous value — the first edition of the check did not account for that, nudged
                    // the text and simply wiped that selection.
                    for c in "vys".chars() {
                        input.events.push(egui::Event::Text(c.to_string()));
                    }
                }
                let out = ctx.run_ui(input, |c| {
                    egui::CentralPanel::default().show(c, |_ui| {});
                    app.feat_cmd_popup(c, rect);
                });
                texts.clear();
                for cs in &out.shapes {
                    super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
                }
            }
            if !texts.iter().any(|t| t.contains("vysota_korpusa")) {
                without.push(format!("{kind}"));
            }
        }
        assert!(
            without.is_empty(),
            "the driver list did not appear in the popups of these part tools: {}",
            without.join(", ")
        );
    }
}
