//! WHAT A SESSION READS: the document, the drawing, the status, the windows, the words and the language.

use super::*;

impl Session {
    /// THE DOCUMENT once the program is at rest.
    pub fn document(&mut self) -> Document {
        self.settle();
        let app = &self.app;
        let p = &app.project;
        let named = |stored: &str| crate::i18n::name(stored);
        let comp_name = |id: Option<u64>| id.and_then(|id| p.components.iter().find(|c| c.id == id)).map(|c| named(&c.name));
        let parts = p
            .components
            .iter()
            .filter(|c| c.id != p.root)
            .map(|c| Part {
                name: named(&c.name),
                key: c.id,
                assembly: c.kind == qymcad_core::feature::ComponentKind::Assembly,
                parent: c.parent.filter(|id| *id != p.root).and_then(|id| comp_name(Some(id))),
                visible: c.visible,
                grounded: c.grounded,
                clone_of: Some(p.instance_origin(c.id)).filter(|o| *o != c.id).and_then(|o| p.components.iter().find(|q| q.id == o)).map(|q| named(&q.name)),
                at: {
                    let t = p.component_transform(c.id);
                    [t[3], t[7], t[11]]
                },
                axes: {
                    let t = p.component_transform(c.id);
                    [[t[0], t[4], t[8]], [t[1], t[5], t[9]], [t[2], t[6], t[10]]]
                },
            })
            .collect();
        use qymcad_core::model::EntityKind;
        let sketches = p
            .sketches
            .iter()
            .enumerate()
            .map(|(si, sk)| {
                let count = |f: fn(&EntityKind) -> bool| sk.entities.iter().filter(|e| f(&e.kind)).count();
                // counted as the sketch's own panel counts them: the origin, the axes and what holds them are not a
                // person's geometry
                let system: std::collections::HashSet<u64> = sk.system_ids().into_iter().collect();
                let own = |c: &qymcad_core::model::Constraint| !matches!(c, qymcad_core::model::Constraint::Fixed { p } if system.contains(p));
                let (dof, redundant) = p.sketch_dof(si);
                SketchInfo {
                    name: named(&sk.name),
                    part: comp_name(p.sketch_owner(sk.id)),
                    points: sk.points.iter().filter(|q| !system.contains(&q.id)).count(),
                    places: sk.points.iter().filter(|q| !system.contains(&q.id)).map(|q| [q.x, q.y]).collect(),
                    picked: if qymcad_ui_state::edit_si(p, &self.app.sketch_ses) == Some(si) { self.app.tools.sel_sk.items.len() } else { 0 },
                    seat: {
                        use qymcad_core::feature::{BasePlane, SketchPlane};
                        match sk.plane {
                            SketchPlane::World(BasePlane::XY) => "XY".to_string(),
                            SketchPlane::World(BasePlane::XZ) => "XZ".to_string(),
                            SketchPlane::World(BasePlane::YZ) => "YZ".to_string(),
                            SketchPlane::Datum(_) => "datum".to_string(),
                            SketchPlane::Face(body, _) => format!("face of {}", comp_name(p.body_owner(body)).unwrap_or_default()),
                        }
                    },
                    lines: count(|k| matches!(k, EntityKind::Line { .. })),
                    construction: sk.entities.iter().filter(|e| e.construction && !system.contains(&e.id)).count(),
                    arcs: count(|k| matches!(k, EntityKind::Arc { .. })),
                    circles: count(|k| matches!(k, EntityKind::Circle { .. })),
                    ellipses: count(|k| matches!(k, EntityKind::Ellipse { .. })),
                    splines: sk.splines.len(),
                    texts: sk.texts.len(),
                    notes: sk.notes.len(),
                    text_fonts: sk.texts.iter().map(|t| qymcad_ui_state::font_label(&t.font, "opt-font")).collect(),
                    min: box_of_sketch(sk, &system).0,
                    max: box_of_sketch(sk, &system).1,
                    constraints: sk.constraints.iter().filter(|c| own(c)).count(),
                    constraint_kinds: sk.constraints.iter().filter(|c| own(c)).map(|c| format!("{c:?}").split([' ', '{', '(']).next().unwrap_or_default().to_string()).collect(),
                    dof,
                    redundant,
                }
            })
            .collect();
        let features = p
            .timeline
            .iter()
            .map(|n| Feature {
                name: named(&n.name),
                kind: format!("{:?}", n.kind).split([' ', '{', '(']).next().unwrap_or_default().to_string(),
                part: comp_name(n.parent),
                suppressed: n.suppressed,
                error: p.regen_errors.get(&n.id).map(super::super::error_words::error_text),
                warning: p.regen_warnings.get(&n.id).map(super::super::error_words::error_text),
                key: n.id,
                bodies: n.kind.bodies().len(),
            })
            .collect();
        let consumed = p.consumed_bodies();
        let bodies = p
            .bodies
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let m = &b.mesh;
                let shape = app.live.shapes.get(&b.id);
                let area = (0..m.tris.len())
                    .map(|ti| {
                        let t = m.triangle(ti);
                        let (u, v) = ([t[1].x - t[0].x, t[1].y - t[0].y, t[1].z - t[0].z], [t[2].x - t[0].x, t[2].y - t[0].y, t[2].z - t[0].z]);
                        0.5 * ((u[1] * v[2] - u[2] * v[1]).powi(2) + (u[2] * v[0] - u[0] * v[2]).powi(2) + (u[0] * v[1] - u[1] * v[0]).powi(2)).sqrt()
                    })
                    .sum();
                let bounds = m.bounds();
                Solid {
                    name: named(&b.name),
                    part: comp_name(p.body_owner(b.id)),
                    part_key: p.body_owner(b.id),
                    colour: p.mesh_color(i),
                    volume: shape.map(|s| s.volume()).unwrap_or_else(|| m.volume()),
                    area,
                    faces: b.faces.len(),
                    face_names: b.faces.iter().map(|f| f.id).collect(),
                    face_centres: b.faces.iter().map(|f| [f.centroid.x, f.centroid.y, f.centroid.z]).collect(),
                    edges: shape.map(|s| s.edges().len()),
                    min: bounds.as_ref().map(|bb| [bb.min.x, bb.min.y, bb.min.z]).unwrap_or([f64::NAN; 3]),
                    max: bounds.as_ref().map(|bb| [bb.max.x, bb.max.y, bb.max.z]).unwrap_or([f64::NAN; 3]),
                    visible: b.visible,
                    consumed: consumed.contains(&b.id),
                    lit_by_the_surface: m.norms.iter().filter(|&&n| n != 0).count(),
                    sheet: b.sheet,
                }
            })
            .collect();
        Document {
            path: app.disk.project_path.clone(),
            unsaved: qymcad_ui_state::edit_key(&app.draw_ctx()) != app.disk.edits.saved_key,
            parts,
            sketches,
            editing: qymcad_ui_state::edit_si(p, &app.sketch_ses).map(|si| named(&p.sketches[si].name)),
            context: app.active_path.last().and_then(|id| p.components.iter().find(|c| c.id == *id)).map(|c| named(&c.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-assembly")),
            datums: p
                .planes
                .iter()
                .map(|q| Datum { name: named(&q.name), kind: "plane".into(), at: q.origin, dir: q.normal })
                .chain(p.datum_axes.iter().map(|q| Datum { name: named(&q.name), kind: "axis".into(), at: q.origin(), dir: q.dir() }))
                .chain(p.datum_points.iter().map(|q| Datum { name: named(&q.name), kind: "point".into(), at: q.at, dir: [0.0; 3] }))
                .collect(),
            features,
            bodies,
            parameters: p.parameters.iter().map(|q| Parameter { name: q.name.clone(), expr: q.expr.clone(), value: q.value }).collect(),
            joints: p.joints.iter().map(|j| JointInfo { name: named(&j.name), kind: format!("{:?}", j.kind), violated: p.mates_violated.contains(&j.id) }).collect(),
            mates: p.mate_constraints.iter().map(|c| format!("{:?}", c.kind)).chain(p.relations.iter().map(|r| format!("{:?}", r.kind))).collect(),
            section: self.app.side.section.plane.is_some(),
            undo: app.disk.edits.undo.iter().map(|s| s.name.clone()).collect(),
            redo: app.disk.edits.redo.iter().map(|s| s.name.clone()).collect(),
        }
    }

    /// WHAT THE EXACT BODY NUMBERED `body` IN [`Document::bodies`] IS MADE OF; `None` while the program holds no exact
    /// body for it (a mesh brought in as it is) or the kernel cannot read it.
    pub fn inspect(&mut self, body: usize) -> Option<Inspection> {
        self.settle();
        let id = self.app.project.bodies.get(body)?.id;
        let shape = self.app.live.shapes.get(&id)?;
        let k = shape.face_kinds()?;
        Some(Inspection {
            valid: shape.is_valid(),
            solids: shape.solid_count(),
            shells: shape.shell_count(),
            kinds: FaceKinds { plane: k[0], cylinder: k[1], cone: k[2], sphere: k[3], torus: k[4], free: k[5], other: k[6] },
        })
    }

    /// HOW THE 3D VIEW DRAWS EACH PIECE OF WHAT IT SHOWS on a screen with a graphics card: the colour of the row of
    /// the table of looks its vertices name, and whether that row is painted as selected. It is what a person sees -
    /// the picture of [`Session::snapshot`] draws the space another way and cannot show what the card is handed.
    pub fn drawn(&mut self) -> Vec<([u8; 3], bool)> {
        self.settle();
        let scene = super::super::render_scene::gpu_scene(&self.app.painting());
        scene
            .pieces
            .iter()
            .filter_map(|p| p.verts.first().map(|v| v.body))
            .map(|row| {
                let look = scene.looks.get(row as usize).copied().unwrap_or(qymcad_ui_state::BodyLook { tint: 0, state: 0 });
                let [r, g, b, _] = look.tint.to_le_bytes();
                ([r, g, b], look.state & qymcad_ui_state::LOOK_HOT != 0)
            })
            .collect()
    }

    /// THE STATUS LINE once the program is at rest.
    pub fn status(&mut self) -> String {
        self.settle();
        // the icons of the interface are letters of a private range of the icon font, not words
        self.app.status.words.chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect::<String>().trim().to_string()
    }

    /// THE WINDOWS OPEN INSIDE THE PROGRAM, by their titles.
    pub fn windows(&mut self) -> Vec<String> {
        self.settle();
        self.win.widgets.iter().filter(|w| w.kind == Kind::Window).map(|w| w.label.clone()).collect()
    }

    /// WHAT IS IN HAND, as the bars of options across the top say it: the words written in every bar a tool or a
    /// command has put up. Empty when nothing is.
    pub fn in_hand(&mut self) -> Vec<String> {
        use qymcad_shell::Fills;
        self.settle();
        let shell = super::super::shell(&self.app.set);
        let bars: Vec<Rect> = shell
            .keys()
            .into_iter()
            .filter(|k| shell.slot_of(k) == Some(qymcad_shell::Slot::Top) && *k != "toolbar" && self.app.live(k))
            .filter_map(|k| egui::containers::panel::PanelState::load(&self.win.ctx, egui::Id::new(k)).map(|s| s.outer_rect))
            .collect();
        self.win.drawn.iter().filter(|(t, r)| !t.is_empty() && bars.iter().any(|b| b.contains(r.center()))).map(|(t, _)| t.clone()).collect()
    }

    /// EVERY WORD ON SCREEN AND WHERE IT STANDS, once the screen is at rest.
    pub fn words_at(&mut self) -> Vec<(String, Rect)> {
        self.settle();
        self.win.drawn.iter().filter(|(t, _)| !t.is_empty()).cloned().collect()
    }

    /// A PICTURE OF THE WINDOW once it is at rest, drawn on the processor from what the last frame drew.
    pub fn snapshot(&mut self) -> Picture {
        self.settle();
        let ppp = self.win.ctx.pixels_per_point();
        let size = [(self.win.screen.x * ppp).round() as usize, (self.win.screen.y * ppp).round() as usize];
        let prims = self.win.ctx.tessellate(self.win.shapes.clone(), ppp);
        let img = super::super::help_raster::paint(&prims, &self.win.textures, size, egui::Color32::BLACK, ppp);
        Picture { width: size[0], height: size[1], rgba: img.pixels.iter().flat_map(|c| c.to_array()).collect() }
    }

    /// CLOSE THE WINDOW TITLED `title` INSIDE THE PROGRAM with the cross on its title bar.
    ///
    /// # Panics
    /// When no such window is open, or it has no cross.
    pub fn close_window(&mut self, title: &str) -> &mut Self {
        self.settle();
        let Some(window) = self.win.widgets.iter().find(|w| w.kind == Kind::Window && w.label == title).map(|w| w.rect) else {
            panic!("no window titled {title:?} is open; open: {:?}", self.windows());
        };
        // the cross is the framework's own button, and it tells a screen reader what it does in these words
        let cross = self
            .win
            .widgets
            .iter()
            .find(|w| w.kind == Kind::Button && w.label == "Close window" && window.contains(w.rect.center()) && w.rect.center().y < window.min.y + 40.0)
            .map(|w| w.rect.center());
        match cross {
            Some(at) => self.click(at),
            None => panic!("the window {title:?} has no cross to close it with"),
        }
    }

    /// THE LONGEST A FRAME TOOK SINCE THE LAST STEP WAS WATCHED, in wall time; asking starts the count again.
    pub fn step_frame(&mut self) -> Duration {
        std::mem::take(&mut self.win.worst_step_frame)
    }

    /// THE WORDS ON SCREEN THAT ARE KEYS OF THE CATALOGUE, not words: a key is what the program shows when it has no
    /// string for it, and a person reads `feat-name-extrude` where a name should be.
    pub fn words_that_are_keys(&mut self) -> Vec<String> {
        let keys: std::collections::HashSet<String> = crate::i18n::reference_keys().into_iter().collect();
        self.words().into_iter().filter(|w| keys.contains(w.split('#').next().unwrap_or(w))).collect()
    }

    /// THE WORDS DRAWN WITH A LETTER THE FONT DOES NOT HAVE - a person sees a box in its place - once the screen is at
    /// rest.
    pub fn words_without_glyphs(&mut self) -> Vec<String> {
        self.settle();
        let mut out = Vec::new();
        let mut texts = Vec::new();
        fn gather(s: &egui::epaint::Shape, out: &mut Vec<std::sync::Arc<egui::Galley>>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push(t.galley.clone()),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| gather(s, out)),
                _ => {}
            }
        }
        for shape in &self.win.shapes {
            gather(&shape.shape, &mut texts);
        }
        // A LETTER THE FONT LACKS IS DRAWN AS THE REPLACEMENT GLYPH, and so takes that glyph's cell of the atlas: the
        // cell is what tells it, not the font's own answer - which says "missing" for every letter of a family whose
        // first font also holds the replacement square.
        let mut replacement: std::collections::HashMap<String, ([u16; 2], [u16; 2])> = std::collections::HashMap::new();
        for g in texts {
            let job = &g.job;
            let mut cells = Vec::new();
            for section in &job.sections {
                let font = section.format.font_id.clone();
                let key = format!("{font:?}");
                let cell = *replacement.entry(key).or_insert_with(|| {
                    let square = self.win.ctx.fonts_mut(|f| f.layout_no_wrap("\u{25fb}".to_string(), font, egui::Color32::WHITE));
                    square.rows.first().and_then(|r| r.row.glyphs.first()).map(|g| (g.uv_rect.min, g.uv_rect.max)).unwrap_or_default()
                });
                cells.push(cell);
            }
            let lost = g
                .rows
                .iter()
                .flat_map(|r| r.row.glyphs.iter())
                .any(|glyph| !glyph.chr.is_whitespace() && !matches!(glyph.chr, '\u{25fb}' | '?') && !glyph.uv_rect.is_nothing() && cells.contains(&(glyph.uv_rect.min, glyph.uv_rect.max)));
            if lost {
                out.push(job.text.clone());
            }
        }
        out
    }

    /// WHAT WAITS FOR A CLICK WITH NO BAR OF OPTIONS SAYING SO: a tool in hand, or a pick a button started, while no
    /// bar a tool or a command puts up is on screen. `None` when there is nothing, or its bar is up.
    pub fn waiting_without_bar(&mut self) -> Option<String> {
        let bar = self.in_hand();
        let waiting = self.waiting_for_a_click();
        (bar.is_empty() && !waiting.is_empty()).then(|| waiting.join(", "))
    }

    /// A BAR OF OPTIONS WITH NOTHING IN HAND: its words, while no tool, no pick, no sketch open, no section, no pattern
    /// of components and no joint being edited stands behind it. `None` when there is no such bar.
    pub fn bar_with_nothing_in_hand(&mut self) -> Option<Vec<String>> {
        let bar = self.in_hand();
        if bar.is_empty() || !self.waiting_for_a_click().is_empty() {
            return None;
        }
        let app = &self.app;
        let behind = qymcad_ui_state::edit_si(&app.project, &app.sketch_ses).is_some() || app.side.section.plane.is_some() || app.side.carr.mode != 0 || app.side.joint.edit.is_some();
        (!behind).then_some(bar)
    }

    /// Everything that waits for a click - a tool in hand, a pick a button started - in words.
    pub(crate) fn waiting_for_a_click(&self) -> Vec<String> {
        let app = &self.app;
        let mut waiting = Vec::new();
        if app.tools.armed != qymcad_ui_state::Armed::None {
            waiting.push(format!("the tool {:?}", app.tools.armed));
        }
        let joint = &app.side.joint;
        for (on, what) in [
            (app.params.mirror.in_hand(), "the plane of a mirrored copy"),
            (app.side.section.pick, "the plane of a section"),
            (app.tools.picking.sketch_for().is_some(), "a sketch for a command"),
            (app.tools.picking.is_sketch_plane(), "the plane of a new sketch"),
            (app.side.m3.on, "the elements to measure"),
            (app.tools.picking.fillet_all(), "the shape whose corners are rounded"),
            (app.side.clip.geom_pending.is_some() || app.side.clip.geom_place, "the base point or the place of a copy"),
            (app.params.boolean.pick.is_some(), "the second body of a boolean"),
            (app.params.boolean.edit.is_some(), "the kind of a boolean being edited"),
            (joint.pick_faces || joint.conn_pick, "an anchor of a joint"),
            (joint.ground_pick, "a part to ground"),
            (joint.group_pick.is_some(), "the parts of a group"),
            (joint.width_pick.is_some(), "the walls of a width"),
            (joint.tangent_pick.is_some(), "the surfaces of a tangent"),
            (joint.axis_pick.is_some(), "the axis of an anchor"),
            (joint.edit_repick.is_some(), "a new anchor of a joint"),
            (joint.relation_pick.is_some(), "the joints of a relation"),
        ] {
            if on {
                waiting.push(what.to_string());
            }
        }
        waiting
    }

    /// THE LONGEST A FRAME TOOK since this was last asked, in wall time; asking starts the count again.
    pub fn worst_frame(&mut self) -> Duration {
        std::mem::take(&mut self.win.worst_frame)
    }

    /// THE TITLE OF THE WINDOW, as the program last set it.
    pub fn title(&mut self) -> String {
        self.settle();
        self.win.title.clone().unwrap_or_default()
    }

    /// The words a person reads in the program's language for the catalogue entry `key`.
    pub fn word(&self, key: &str) -> String {
        self.speak();
        crate::i18n::tr(key)
    }

    /// THE CODE OF THE LANGUAGE the program speaks (`en`, `ru`).
    pub fn language(&self) -> String {
        self.speak();
        crate::i18n::language()
    }

    /// THIS SESSION'S LANGUAGE ON THIS THREAD again: the machine's locale and the program's own setting.
    pub(crate) fn speak(&self) {
        qymcad_i18n::stand_for_system_locale(&self.locale);
        super::super::apply_language(&self.app.set);
    }

    /// THE WORDS ON SCREEN once it is at rest.
    pub fn words(&mut self) -> Vec<String> {
        self.settle();
        self.win.drawn.iter().map(|(t, _)| t.clone()).filter(|t| !t.is_empty()).collect()
    }

    /// Is `text` written anywhere on screen, whole, once it is at rest?
    pub fn shows(&mut self, text: &str) -> bool {
        self.settle();
        self.win.drawn.iter().any(|(t, _)| t == text)
    }
}
