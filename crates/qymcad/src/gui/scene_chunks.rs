//! A SCENE TOO BIG FOR ONE BUFFER IS DRAWN IN PIECES.
//!
//! Reported behaviour, on the heaviest reference file (a V8 engine in STEP): the program died on opening it
//! with `Buffer size 739358112 is greater than the maximum buffer size (268435456)`. Every vertex of the
//! scene went into ONE vertex buffer, and a graphics device has a limit on how big one buffer may be - 256 MB
//! is the ordinary one. The engine holds 23 million vertices at 32 bytes each.
//!
//! The cure is not a bigger buffer (the limit belongs to the device) but several of them, drawn one after
//! another. The arithmetic of "where to cut" lives here, apart from the graphics, so it can be measured
//! without a GPU.
//!
//! THE UNIT OF CUTTING IS A BLOCK, not a vertex. While the scene was drawn unindexed, a body could be
//! written across the seam of two buffers in two goes; an indexed draw reads its vertices from ONE buffer, so
//! a body has to lie inside one piece entire.

/// HOW MANY VERTICES AND INDICES ONE PIECE MAY HOLD, by the device's own limit on a single buffer.
///
/// Two numbers, not one: a closed mesh has about twice as many triangles as vertices, that is six indices per
/// vertex at 4 bytes each - 24 bytes of indices against 20 bytes of vertex. The index buffer reaches the
/// limit FIRST, and a piece cut by vertices alone would overflow it.
pub fn room_in_a_chunk(max_bytes: u64, vertex_bytes: u64, index_bytes: u64) -> (u32, u32) {
    let cap = |per: u64| (max_bytes / per.max(1)).max(3).min(u32::MAX as u64) as u32;
    (cap(vertex_bytes), cap(index_bytes))
}

/// WHICH PIECE EACH BLOCK GOES INTO, and where inside it.
///
/// An indexed draw reads from ONE vertex buffer, so a block may not lie across the seam of two (unindexed, it
/// simply got written twice). The blocks are laid into a piece one after another, and the piece is closed as
/// soon as the next block would not fit - by vertices OR by indices. The offset of the block inside the piece
/// becomes the `base_vertex` of the draw, so the indices stay local to the block.
///
/// A block that does not fit an EMPTY piece is placed in one of its own: it was split at build time, and this
/// is the last resort rather than a silent loss of geometry.
pub fn pack_blocks(blocks: &[(u32, u32)], room: (u32, u32)) -> Vec<Placed> {
    let (mut out, mut at, mut used) = (Vec::with_capacity(blocks.len()), 0u32, (0u32, 0u32));
    for &(verts, idx) in blocks {
        let fits = used.0.saturating_add(verts) <= room.0 && used.1.saturating_add(idx) <= room.1;
        if !fits && used != (0, 0) {
            at += 1;
            used = (0, 0);
        }
        out.push(Placed { chunk: at, first_vertex: used.0, first_index: used.1 });
        used = (used.0 + verts, used.1 + idx);
    }
    out
}

/// Where a block lies: which piece, and at what offset in its vertices and in its indices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Placed {
    pub chunk: u32,
    pub first_vertex: u32,
    pub first_index: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PIECE IS CLOSED BY WHICHEVER LIMIT COMES FIRST - and on a closed mesh that is the indices.
    #[test]
    fn a_piece_is_measured_by_vertices_and_by_indices() {
        let (v, i) = room_in_a_chunk(268435456, 20, 4);
        assert_eq!(v, 13421772, "the room for vertices is not the device limit divided by the vertex");
        assert_eq!(i, 67108864, "the room for indices is not the device limit divided by the index");
        // a closed mesh: about six indices per vertex, so the indices run out on the smaller number of bodies
        assert!(i < v * 6, "with six indices per vertex the index buffer must be the first to fill up");
    }

    /// THE BLOCKS ARE LAID OUT WITHOUT CROSSING A SEAM, and each knows its offset inside its piece.
    #[test]
    fn no_block_lies_across_the_seam_of_two_pieces() {
        let blocks = [(30u32, 90u32), (40, 120), (50, 150), (10, 30)];
        let placed = pack_blocks(&blocks, (100, 300));
        // the first three fill a piece exactly (120 vertices would not fit, so the third opens a new one)
        assert_eq!(placed[0], Placed { chunk: 0, first_vertex: 0, first_index: 0 });
        assert_eq!(placed[1], Placed { chunk: 0, first_vertex: 30, first_index: 90 });
        assert_eq!(placed[2], Placed { chunk: 1, first_vertex: 0, first_index: 0 }, "70 + 50 does not fit in 100");
        assert_eq!(placed[3], Placed { chunk: 1, first_vertex: 50, first_index: 150 });
        for (b, p) in blocks.iter().zip(&placed) {
            let room = pack_blocks(&blocks, (100, 300)).iter().filter(|q| q.chunk == p.chunk).count();
            assert!(room > 0, "a block was placed nowhere");
            assert!(p.first_vertex + b.0 <= 100, "a block runs past the end of its piece");
            assert!(p.first_index + b.1 <= 300, "the indices of a block run past the end of their piece");
        }
    }

    /// THE INDEX LIMIT CLOSES A PIECE ON ITS OWN, even when there is room for the vertices.
    #[test]
    fn the_indices_can_close_a_piece_while_the_vertices_still_fit() {
        let placed = pack_blocks(&[(10, 200), (10, 200)], (1000, 300));
        assert_eq!(placed[1].chunk, 1, "the piece must be closed by the indices: 400 of them against a room of 300");
    }

    /// A BLOCK BIGGER THAN A WHOLE PIECE gets a piece to itself rather than disappearing.
    #[test]
    fn a_block_too_big_for_any_piece_still_gets_a_place() {
        let placed = pack_blocks(&[(10, 30), (500, 1500), (10, 30)], (100, 300));
        assert_eq!(placed[1].chunk, 1, "an oversized block must not be squeezed in beside another");
        assert_eq!(placed[1].first_vertex, 0, "and it starts its piece from the beginning");
        assert_eq!(placed[2].chunk, 2, "and nothing is laid on top of it");
    }
}

/// THE SHADER OF THE VIEWPORT IS CHECKED HERE, not when a window opens.
///
/// Nothing in Rust looks inside a shader: it is a string handed to the driver, and a mistake in it shows up
/// as a blank viewport on a machine with a graphics card - or, worse, as geometry drawn wrong. Parsing and
/// validating it in a check costs milliseconds and catches the mistake at the same moment as a typo in Rust.
#[cfg(test)]
mod shader {
    #[test]
    fn the_viewport_shader_parses_and_validates() {
        let src = crate::viewport_gpu::SHADER;
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("the viewport shader does not parse:\n{}", e.emit_to_string(src)));
        let mut v = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all());
        v.validate(&module).unwrap_or_else(|e| panic!("the viewport shader does not validate: {e:?}"));
    }
}

/// HIGHLIGHTING A BODY DOES NOT TOUCH THE GEOMETRY.
///
/// Reported behaviour on the reference engine (23 million vertices): the pointer over a part, or stepping
/// into a subassembly, froze the program. The colour, the highlight and the ghosting were baked into every
/// vertex, so a change of look changed the vertices - and the whole scene, 739 MB of it, went to the card
/// again.
#[cfg(test)]
mod look_apart {
    use crate::gui::App;
    use qymcad_ui_state::Sel;

    #[test]
    fn highlighting_a_body_changes_the_look_and_not_the_scene() {
        let mut app = App::default();
        crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let mi = app.project.mesh_id(0).and_then(|b| app.project.mesh_index(b)).expect("setup: a body was built");

        let before_key = qymcad_ui_state::gpu_scene_key(&app.painting());
        let (before, looks_before) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        assert!(!before.is_empty(), "setup: the scene is empty");
        assert_eq!(looks_before[0].state, 0, "setup: nothing is highlighted yet");

        // the body is selected: it lights up
        app.chosen.sel = Sel::Mesh(mi);
        let after_key = qymcad_ui_state::gpu_scene_key(&app.painting());
        let (after, looks_after) = crate::gui::render_scene::gpu_scene_flat(&app.painting());

        assert_eq!(before_key, after_key, "the geometry key changed from a highlight: the whole scene would go to the card again");
        let (bb, ab) = (bytemuck::cast_slice::<_, u8>(&before), bytemuck::cast_slice::<_, u8>(&after));
        let differ = bb.len() != ab.len() || bb.iter().zip(ab).position(|(x, y)| x != y).is_some();
        assert!(!differ, "the vertices changed from a highlight: {} bytes against {}", bb.len(), ab.len());
        assert_eq!(looks_after[0].state, qymcad_ui_state::LOOK_HOT, "the highlight did not reach the look table at all");
    }
}

/// STEPPING INTO A CONTEXT DOES NOT SEND THE SCENE TO THE CARD AGAIN.
///
/// Reported behaviour: "the file opens, but I cannot enter the engine assembly - it hangs". Entering changes
/// what the bodies LOOK like: the ones outside the context become ghosts (with neighbours shown) or leave the
/// picture altogether. The look used to be baked into the vertices, so a step down the tree rebuilt every
/// block of the scene and sent the whole buffer to the card again - 739 MB on the reference engine.
#[cfg(test)]
pub(crate) mod entering_a_context {
    use crate::gui::App;
    use qymcad_core::model::Id;

    /// A part with a body of its own inside the given parent component; the context is left afterwards.
    pub(crate) fn part_inside(app: &mut App, parent: Id, x: f64) -> Id {
        app.project.set_active_component(Some(parent));
        let part = app.project.add_part(format!("Part {x}"));
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, x, 0.0, x + 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
        app.start_feat_cmd(qymcad_ui_state::PartTool::Extrude);
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        app.exit_context();
        part
    }

    /// WITH NEIGHBOURS SHOWN, entering a part turns them into ghosts and moves nothing: not one vertex may
    /// change, and the key of the buffer may not either.
    #[test]
    fn entering_a_part_only_changes_the_look_table() {
        let mut app = App::default();
        let root = app.project.root;
        let first = part_inside(&mut app, root, 0.0);
        part_inside(&mut app, root, 40.0);
        app.win.context = true; // "in context": the neighbouring parts stay on screen as ghosts
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        assert_eq!(app.project.bodies.len(), 2, "setup: two parts side by side are needed");

        let before_key = qymcad_ui_state::gpu_scene_key(&app.painting());
        let (before, looks_before) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        assert!(!before.is_empty(), "setup: the scene is empty");
        assert!(looks_before.iter().all(|l| l.state == 0), "setup: in the root nothing is a ghost yet");

        // the step a person takes: into the part
        app.enter_component(first);
        let after_key = qymcad_ui_state::gpu_scene_key(&app.painting());
        let (after, looks_after) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        let rebuilt = app.cache.scene_stats.get()[0];

        assert_eq!(before_key, after_key, "the geometry key changed on entering: the whole scene would go to the card again");
        let (bb, ab) = (bytemuck::cast_slice::<_, u8>(&before), bytemuck::cast_slice::<_, u8>(&after));
        assert!(bb == ab, "the vertices changed on entering a part: {} bytes against {}", bb.len(), ab.len());
        assert_eq!(rebuilt, 0, "the blocks were built anew although not a single body moved");
        assert!(looks_after.iter().any(|l| l.state == qymcad_ui_state::LOOK_GHOST), "the neighbouring part did not turn into a ghost");
    }

    /// ENTERING A SUBASSEMBLY takes the bodies outside it out of the picture - the buffer does change. What
    /// may NOT happen is building the blocks anew: the bodies that stay have neither moved nor changed shape.
    #[test]
    fn entering_a_subassembly_builds_no_block_anew() {
        let mut app = App::default();
        let root = app.project.root;
        part_inside(&mut app, root, 0.0);
        let sub = app.project.add_assembly("Subassembly");
        part_inside(&mut app, sub, 40.0);
        app.set_context_to(root); // back at the root, as "make active" in the tree puts one there
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let (before, _) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        assert!(!before.is_empty(), "setup: the scene is empty");

        app.enter_component(sub);
        let (after, looks) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
        let stats = app.cache.scene_stats.get();

        assert_eq!(looks.len(), 1, "inside the subassembly only its own body is drawn");
        assert!(after.len() < before.len(), "setup: the body outside the context must leave the picture");
        assert_eq!(stats[0], 0, "the block of the body that stayed was built anew although it neither moved nor changed");
        assert_eq!(stats[2], 1, "the block that stayed was not taken ready-made");
    }
}

/// THE SCENE IS INDEXED: a vertex is written once, and the triangles point at it.
///
/// Measured on a closed mesh: a vertex belongs to about six triangles, so unrolled it was written out three
/// times over. On the reference engine that is 23 million vertices at 20 bytes against 8 million plus 23
/// million indices at 4 - 460 MB against 250.
#[cfg(test)]
mod indexed {
    use crate::gui::App;

    /// The scene of one part, and the mesh it was built from.
    fn one_part() -> (App, Vec<qymcad_ui_state::ScenePiece>) {
        let mut app = App::default();
        crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let pieces = crate::gui::render_scene::gpu_scene(&app.painting()).pieces;
        assert!(!pieces.is_empty(), "setup: the scene is empty");
        (app, pieces)
    }

    /// FEWER VERTICES, THE SAME TRIANGLES.
    ///
    /// How much fewer depends on the body. A BOX shares the least there is to share: its mesh keeps a corner
    /// three times over, once per face it belongs to, because the faces meet at an edge and not smoothly - 24
    /// vertices for 12 triangles, against 36 written out unrolled. A CURVED face shares within itself, and
    /// there the count falls by about three: a grid of V vertices carries about 2V triangles, that is 6V
    /// vertices unrolled against V here.
    #[test]
    fn a_shared_vertex_is_written_once() {
        let (app, pieces) = one_part();
        let verts: usize = pieces.iter().map(|p| p.verts.len()).sum();
        let idx: usize = pieces.iter().map(|p| p.idx.len()).sum();
        let mesh = &app.project.bodies.iter().find(|b| b.visible).expect("a visible body").mesh;

        assert_eq!(idx % 3, 0, "the indices do not make whole triangles");
        assert_eq!(idx / 3, mesh.tris.len(), "the scene holds {} triangles against {} in the mesh", idx / 3, mesh.tris.len());
        assert!(verts < idx, "the vertices are not shared at all: {verts} of them against {idx} indices");
        assert!(verts <= mesh.verts.len(), "the scene holds MORE vertices than the mesh: {verts} against {}", mesh.verts.len());
    }

    /// EVERY INDEX POINTS INSIDE ITS OWN PIECE. A draw reads the vertices of one buffer only, so an index
    /// past the end of its piece is geometry read out of another body - or out of nothing at all.
    #[test]
    fn no_index_points_past_its_own_vertices() {
        let (_app, pieces) = one_part();
        for (n, p) in pieces.iter().enumerate() {
            let past = p.idx.iter().filter(|&&i| i as usize >= p.verts.len()).count();
            assert_eq!(past, 0, "piece {n}: {past} indices point past its {} vertices", p.verts.len());
        }
    }

    /// THE TRIANGLES ARE THE ONES THE MESH HAS - the same three points, in the same order.
    ///
    /// This is what "the picture has not changed" means where it can be checked without a graphics card: the
    /// card draws exactly these triples, and the shading is computed from them.
    #[test]
    fn the_triangles_that_come_out_are_the_ones_that_went_in() {
        let (app, pieces) = one_part();
        let mesh = &app.project.bodies.iter().find(|b| b.visible).expect("a visible body").mesh;
        let want: Vec<[[f32; 3]; 3]> = (0..mesh.tris.len()).map(|t| mesh.triangle(t).map(|p| [p.x as f32, p.y as f32, p.z as f32])).collect();
        let got: Vec<[[f32; 3]; 3]> = pieces.iter().flat_map(|p| p.idx.chunks(3).map(|c| [p.verts[c[0] as usize].pos, p.verts[c[1] as usize].pos, p.verts[c[2] as usize].pos])).collect();
        assert_eq!(got.len(), want.len(), "the number of triangles changed");
        for (n, (g, w)) in got.iter().zip(&want).enumerate() {
            assert_eq!(g, w, "triangle {n} came out of the indices as another triangle");
        }
    }
}

/// THE FAST PATH OF THE SCENE MUST ANSWER AS THE SLOW ONE DOES.
///
/// The scene asks three things of every visible body: whose it is, where it stands and whether it belongs to
/// the context being looked at. Each has an answer of its own in the model (`body_owner`,
/// `body_display_transform`, `body_is_ghost`), and each costs a pass over the timeline or up the component
/// tree - on a document of 1296 bodies that came to 175 ms per frame, felt as the whole program going
/// sluggish. The scene now answers them from one pass of its own, and a second answer that drifts from the
/// first is not a slow picture but a wrong one: reported as everything turning see-through after the speed-up.
#[cfg(test)]
mod fast_path {
    use crate::gui::App;

    #[test]
    fn the_scene_agrees_with_the_model_about_every_body() {
        let mut app = App::default();
        let root = app.project.root;
        // a part in the root, a subassembly with a part of its own, and a body that went through a second
        // operation - the case where "the first node that owns it" and "the last" part company
        super::entering_a_context::part_inside(&mut app, root, 0.0);
        let sub = app.project.add_assembly("Subassembly");
        let inner = super::entering_a_context::part_inside(&mut app, sub, 40.0);
        app.enter_component(inner);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 42.0, 2.0, 50.0, 10.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        let sid = app.project.sketches[si].id;
        let profiles: Vec<qymcad_core::model::Id> = app.project.sketches[si].contour_ids.clone();
        app.project.add_extrude_multi(sid, profiles, 4.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        app.exit_context();
        app.win.context = true; // neighbours on screen, so ghosts are possible at all

        // THE ORDER OF THE ANSWER MATTERS: `body_owner` takes the FIRST node that names the body, and the pass
        // here must take the same one. Measured on the reference engine (1296 bodies): first and last agree
        // everywhere, so the difference is not visible on a document like that - it would show as bodies of a
        // foreign component, drawn see-through and without depth.

        // from the root, from inside the subassembly and from inside a part: the contexts differ, and so do
        // the answers
        for step in [None, Some(sub), Some(inner)] {
            if let Some(c) = step {
                app.enter_component(c);
            }
            let pn = app.painting();
            let ctx = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
            let items = qymcad_ui_state::visible_mesh_items(&pn);
            assert!(!items.is_empty(), "setup: nothing is drawn in context {ctx:?}");
            for m in &items {
                let id = app.project.mesh_id(m.index).expect("the body has an id");
                let dc = app.draw_ctx();
                assert_eq!(m.ghost, qymcad_ui_state::body_is_ghost(&dc, m.index), "body {id:?} in context {ctx:?}: the scene and the model disagree about the foreign context");
                let want = app.project.body_display_transform(id, ctx);
                assert_eq!(m.world, want, "body {id:?} in context {ctx:?}: the scene placed it by another frame");
            }
        }
    }
}
