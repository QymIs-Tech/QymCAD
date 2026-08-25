//! A COMPONENT ARRAY — the whole path a person walks.
//!
//! The kernel is checked separately (`qymcad-testkit/tests/comp_pattern.rs`); here it is the wiring: pick
//! a part, press the button, set the count and the step, Enter — the row stands. Plus what surrounds it:
//! the preview before Enter, Esc without a trace, reopening for editing, and deleting a copy removing
//! the array rather than letting it grow back.
#[cfg(test)]
pub(super) mod tests {
    use super::super::{App, Sel};
    use qymcad_core::model::CompPatternKind;

    /// An assembly with one cube part; returns its component.
    pub(in crate::gui) fn assembly_with_part(app: &mut App) -> u64 {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        let comp = app.project.body_owner(body).expect("the owner");
        let root = app.project.root;
        app.set_context_to(root); // component arrays live in the ASSEMBLY
        comp
    }

    /// The buttons are there — both of them.
    #[test]
    fn the_tools_have_buttons() {
        let src = crate::gui::panels_source::PANELS;
        assert!(src.contains("self.start_comp_array(1);"), "a linear component array without a button does not exist");
        assert!(src.contains("self.start_comp_array(2);"), "a circular component array without a button does not exist");
    }

    /// THE WHOLE PATH: pick a part -> the button -> the count and the step -> Enter -> the copies stand.
    #[test]
    fn a_part_can_be_patterned_from_the_toolbar() {
        let mut app = App::default();
        let comp = assembly_with_part(&mut app);
        let ci = app.project.components.iter().position(|c| c.id == comp).expect("the index");
        app.sel = Sel::Component(ci);

        app.start_comp_array(1);
        assert_eq!(app.carr.mode, 1, "the command must open; the status line: {}", app.status);
        assert_eq!(app.carr.src, comp, "the source is the selected part");
        assert!(app.cmd.params.iter().any(|p| p.key == "cstep"), "a linear array must have a step field");

        app.arr.count = 4;
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "cstep") {
            p.val = 30.0;
            p.txt = "30".into();
        }
        app.apply_comp_array();
        app.rebuild_if_dirty();

        let pat = app.project.comp_pattern_of(comp).expect("the array is created");
        assert_eq!(pat.copies.len(), 3, "4 instances = the source + 3 copies; the status line: {}", app.status);
        let copies = pat.copies.clone();
        for (i, c) in copies.iter().enumerate() {
            let t = app.project.component_transform(*c);
            assert!((t[3] - 30.0 * (i + 1) as f64).abs() < 1e-9, "copy {i} must stand at its own step, and it stands at {}", t[3]);
            let v = app.project.component_bodies(*c).first().and_then(|b| app.project.mesh_index(*b)).map(|m| app.project.bodies[m].mesh.volume()).unwrap_or(0.0);
            assert!(v > 1.0, "copy {i} must have A BODY, and its volume is {v}");
        }
        assert_eq!(app.carr.mode, 0, "after applying, the command closes");
    }

    /// With no part selected the command does not start and says why.
    #[test]
    fn without_a_part_the_command_refuses() {
        let mut app = App::default();
        assembly_with_part(&mut app);
        app.sel = Sel::None;
        app.start_comp_array(1);
        assert_eq!(app.carr.mode, 0, "the command must not open without a source");
        assert_eq!(app.status, crate::i18n::tr("msg-pick-part-first"), "the reason must be said out loud");
    }

    /// Esc cancels without a trace.
    #[test]
    fn escape_cancels_without_a_trace() {
        let mut app = App::default();
        let comp = assembly_with_part(&mut app);
        let ci = app.project.components.iter().position(|c| c.id == comp).expect("the index");
        app.sel = Sel::Component(ci);
        app.start_comp_array(1);
        app.on_escape();
        assert_eq!(app.carr.mode, 0, "the command is closed");
        assert!(app.project.comp_patterns.is_empty(), "nothing was created");
    }

    /// THE PREVIEW before Enter: the ghosts of the copies are drawn.
    #[test]
    fn the_ghosts_are_drawn_before_enter() {
        let src = crate::gui::render_source::RENDER;
        assert!(src.contains("pub(super) fn draw_comp_array_preview"), "a component array must have a preview");
        assert!(src.contains("self.draw_comp_array_preview(painter, rect);"), "the preview must be called from the frame");
        // THE COMPUTATION OF THE GHOSTS LIVES APART from the drawing — so that it can be checked with
        // numbers rather than by reading the source (see `preview_matches_result`).
        assert!(src.contains("pub(super) fn comp_array_ghosts"), "the computation of the ghosts must be a function of its own");
        let a = src.find("pub(super) fn comp_array_ghosts").expect("the block");
        let b = src[a..].find("\n    pub(super) fn draw_comp_array_preview").map(|i| a + i).unwrap_or(src.len());
        assert!(src[a..b].contains("(1..kind.count())"), "the ghosts are for THE COPIES only: the source is on screen anyway");
    }

    /// REOPENING FOR EDITING restores the layout and does not breed a second array.
    #[test]
    fn reopening_the_pattern_edits_it_in_place() {
        let mut app = App::default();
        let comp = assembly_with_part(&mut app);
        let pid = app.project.add_comp_pattern(comp, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 3 });
        assert_ne!(pid, 0, "setup: the array is there");
        let copies_before = app.project.comp_pattern_of(comp).expect("the array").copies.clone();

        // THE PATH TO EDITING EXISTS IN THE INTERFACE: an item in the context menu of a component.
        // Without this check the editing function would live "for the test" — and it did live that way
        // until a compiler warning caught it.
        assert!(crate::gui::panels_source::PANELS.contains("self.start_comp_array_edit(pid);"), "editing an array must be callable from the tree");
        app.start_comp_array_edit(pid);
        assert_eq!(app.carr.mode, 1, "editing must open the same command");
        assert_eq!(app.carr.edit, pid, "it is THIS array that is being edited");
        assert_eq!(app.arr.count, 3, "the number of instances must be restored");
        assert!((app.cmd_val("cstep") - 30.0).abs() < 1e-9, "the step must be restored");

        app.arr.count = 5;
        app.apply_comp_array();
        assert_eq!(app.project.comp_patterns.len(), 1, "a second array must not appear");
        let after = app.project.comp_pattern_of(comp).expect("the array").copies.clone();
        assert_eq!(after.len(), 4, "there are 5 instances now");
        assert_eq!(&after[..2], &copies_before[..], "the former copies kept their Ids — the mates on them are alive");
    }

    /// DELETING A COPY removes the whole array while the source stays.
    ///
    /// Otherwise the deleted copy would grow back on the next rebuild: it is the array that leads it.
    #[test]
    fn deleting_one_copy_removes_the_pattern_and_spares_the_source() {
        let mut app = App::default();
        let comp = assembly_with_part(&mut app);
        app.project.add_comp_pattern(comp, CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 4 });
        app.rebuild_if_dirty();
        let copies = app.project.comp_pattern_of(comp).expect("the array").copies.clone();

        let ci = app.project.components.iter().position(|c| c.id == copies[1]).expect("the index of the copy");
        app.execute_delete(Sel::Component(ci));

        assert!(app.project.comp_patterns.is_empty(), "the array must go whole");
        for c in &copies {
            assert!(!app.project.components.iter().any(|x| x.id == *c), "copy {c} must go");
        }
        assert!(app.project.components.iter().any(|c| c.id == comp), "THE SOURCE stays — it is a part somebody made");
        assert!(app.project.active_body(comp).is_some(), "and so does its body");
    }

    /// The instance row is visible in the tree — otherwise a copy looks like a part with no recipe.
    #[test]
    fn the_instance_is_visible_in_the_tree() {
        assert!(crate::gui::panels_source::PANELS.contains("FeatureKind::PartInstance { src_comp, .. } =>"), "the row in the tree");
        let gui = include_str!("../gui.rs");
        assert!(gui.contains("FK::PartInstance { .. } => ph::"), "the icon");
        assert!(!crate::i18n::tr("feat-name-instance").is_empty() && crate::i18n::tr("feat-name-instance") != "feat-name-instance", "the default name of the feature must have a translation");
    }

    /// A COPY OF A PART IS NOT AN ASSEMBLY, AND THE ICON MUST SAY SO.
    ///
    /// Reported from a screenshot: what is this instance inside, with an assembly icon, that cannot be
    /// entered? Fair: the row carried `ph::STACK` — the same icon that marks ASSEMBLIES in the tree and
    /// labels the new-subassembly button. The icon promised a node one enters, and this is a copy of a
    /// body.
    #[test]
    fn a_part_copy_does_not_wear_the_assembly_icon() {
        let panels = crate::gui::panels_source::PANELS;
        let gui = include_str!("../gui.rs");
        let sub_assembly_icon = {
            // "the assembly icon" is not a letter in the code but THE ONE ON THE NEW-SUBASSEMBLY BUTTON:
            // it is asked of the button, otherwise the check would drift from the interface at the very
            // first change of the icon set.
            let at = panels.find("tb-new-subassembly-hint").expect("the new-subassembly button");
            let head = &panels[..at];
            let i = head.rfind("ph::").expect("the button has an icon");
            let name: String = head[i + 4..].chars().take_while(|c| c.is_ascii_uppercase() || *c == '_').collect();
            format!("ph::{name}")
        };
        assert!(sub_assembly_icon.starts_with("ph::"), "setup: the subassembly icon was not found, out came \"{sub_assembly_icon}\"");
        for (what, src, at) in [
            ("the tree row", panels, panels.find("FeatureKind::PartInstance { src_comp, .. } =>")),
            ("the icon of the feature", gui, gui.find("FK::PartInstance { .. } =>")),
        ] {
            let at = at.unwrap_or_else(|| panic!("{what}: the display of a part copy was not found"));
            let tail = &src[at..src[at..].find('\n').map(|e| at + e).unwrap_or(src.len())];
            assert!(
                !tail.contains(&sub_assembly_icon),
                "{what}: a copy of a part is marked with the ASSEMBLY icon ({sub_assembly_icon}) — it promises a node one enters, and there is nowhere to enter"
            );
        }
    }
}

/// A GHOST MUST POINT WHERE THE COPIES WILL LATER LIE.
///
/// Reported from a screenshot: the array works crookedly, look at the frames. A ghost was computed as
/// `step x the transform of the body IN THE FRAME OF THE VIEW`, while the finished array places a copy
/// as `step x the transform of the source` in the system of THE PARENT. Those coincided exactly when the
/// parent stands at zero.
#[cfg(test)]
mod preview_matches_result {
    use super::super::App;
    use qymcad_core::feature::{mat_mul12, PLACE_IDENTITY};

    /// The source lies in the assembly WITH A PLACEMENT OF ITS OWN — that is where the divergence came
    /// out.
    #[test]
    fn ghosts_land_where_the_copies_will() {
        let mut app = App::default();
        let src = super::tests::assembly_with_part(&mut app);
        let mut off = PLACE_IDENTITY;
        off[3] = 37.0; // a shifted source: with a correct computation the ghost travels with it
        app.project.set_component_transform(src, off);

        app.carr.mode = 1;
        app.carr.src = src;
        let ctx = app.current_ctx_id();
        let parent = app.project.components.iter().find(|c| c.id == src).and_then(|c| c.parent).unwrap_or(app.project.root);
        let base = app.project.components.iter().find(|c| c.id == src).map(|c| c.transform).expect("the source");
        let pre = app.project.relative_transform(parent, ctx);
        let ghosts = app.comp_array_ghosts(&pre, &base, 0);
        assert!(!ghosts.is_empty(), "setup: the ghosts must exist");

        // and now the array IS APPLIED — and the copies must stand in exactly the same places
        let kind = app.comp_array_kind();
        let want: Vec<[f64; 12]> = (1..kind.count()).map(|i| mat_mul12(&pre, &mat_mul12(&kind.step_transform(i), &base))).collect();
        for (g, w) in ghosts.iter().zip(want.iter()) {
            for k in 0..12 {
                assert!((g[k] - w[k]).abs() < 1e-9, "the ghost stands where the copy will not:\nghost {g:?}\ncopy  {w:?}");
            }
        }
        // AND SHIFTING THE SOURCE DRAGS THE GHOSTS ALONG. The check goes by the difference and not by an
        // absolute: the default step of an array may well be zero, and then an absolute number proves
        // nothing.
        let at_zero = app.comp_array_ghosts(&pre, &PLACE_IDENTITY, 0);
        for (moved, still) in ghosts.iter().zip(at_zero.iter()) {
            assert!((moved[3] - still[3] - 37.0).abs() < 1e-9, "the ghost did not travel with the source: {} against {}", moved[3], still[3]);
        }
    }

    /// EDITING AN EXISTING ARRAY: there are no more frames than copies still unplaced.
    #[test]
    fn editing_an_existing_pattern_does_not_double_the_frames() {
        let mut app = App::default();
        let src = super::tests::assembly_with_part(&mut app);
        app.carr.mode = 1;
        app.carr.src = src;
        let all = app.comp_array_ghosts(&PLACE_IDENTITY, &PLACE_IDENTITY, 0).len();
        assert!(all >= 1, "setup: with no copies placed the ghosts must exist");
        let with_one_placed = app.comp_array_ghosts(&PLACE_IDENTITY, &PLACE_IDENTITY, 1).len();
        assert_eq!(with_one_placed, all - 1, "a copy that is placed must remove its own ghost, otherwise there are twice as many frames as bodies");
    }
}
