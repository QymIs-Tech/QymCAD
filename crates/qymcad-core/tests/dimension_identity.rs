//! Named dimensions stay attached to their selected records through edits and saving.

use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, DimensionId, DimTarget, EdgeDimensionEnd, EntityKind, Id, NamedDim, Param, PatternKind, Project, SketchDimensionSelection};

struct Scene {
    project: Project,
    si: usize,
    points: Vec<Id>,
}

struct LineScene {
    scene: Scene,
    entity: Id,
}

enum Axis {
    Horizontal,
    Vertical,
}

impl Scene {
    fn empty() -> Self {
        let mut project = Project::default();
        project.new_document();
        let si = project.new_sketch("Sketch2");
        project.add_sketch_node(project.sketches[si].id, "Sketch2");
        let points = [(0.0, 0.0), (20.0, 15.0), (20.0, 0.0), (0.0, 15.0)].into_iter().map(|(x, y)| project.sketch_point_at(si, x, y, 1e-6)).collect();
        Self { project, si, points }
    }

    fn pair() -> Self {
        let mut scene = Self::empty();
        scene.project.sketches[scene.si].constraints = vec![scene.distance(Axis::Horizontal, 20.0), scene.distance(Axis::Vertical, 15.0)];
        scene
    }

    fn line() -> LineScene {
        let mut scene = Self::empty();
        let entity = scene.project.add_line_entity(scene.si, 5.0, 5.0, 25.0, 5.0, Purpose::Real);
        let EntityKind::Line { a, b } = scene.project.sketches[scene.si].entities.iter().find(|e| e.id == entity).expect("the line exists").kind else {
            panic!("the selected entity is a line");
        };
        scene.project.sketches[scene.si].constraints.push(Constraint::Distance { dimension: DimensionId::default(), a, b, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 1, at: None });
        LineScene { scene, entity }
    }

    fn distance(&self, axis: Axis, d: f64) -> Constraint {
        Constraint::Distance {
            dimension: DimensionId::default(),
            a: self.points[1],
            b: self.points[0],
            d,
            off: 0.0,
            expr: String::new(),
            driven: false,
            axis: match axis {
                Axis::Horizontal => 1,
                Axis::Vertical => 2,
            },
            at: None,
        }
    }

    fn selection(&self, ci: usize) -> SketchDimensionSelection {
        SketchDimensionSelection { sketch: self.project.sketches[self.si].id, index: ci }
    }

    fn name_selected(&mut self, ci: usize, name: &str) -> bool {
        self.project.name_sketch_dimension(name.into(), self.selection(ci))
    }

    fn selected_target(&self, ci: usize) -> DimTarget {
        self.project.sketch_dimension_target(self.selection(ci)).expect("the selected dimension has a unique bound identity")
    }

    fn identity_at(&self, ci: usize) -> DimensionId {
        self.project.sketches[self.si].constraints[ci].dimension_id().expect("the selected constraint is a dimension")
    }

    fn target_named(&self, name: &str) -> DimTarget {
        self.project.named_dims.iter().find(|nd| nd.name == name).expect("the dimension name must remain registered").target.clone()
    }

    fn value_at(&self, ci: usize) -> f64 {
        Project::dim_value_of(&self.project.sketches[self.si].constraints[ci]).expect("the selected constraint has a value")
    }
}

#[test]
fn x_and_y_targets_are_distinct() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(1, "d6"));
    assert_ne!(scene.selected_target(0), scene.selected_target(1), "the two selected dimensions retain distinct identities");
}

#[test]
fn both_dimension_names_survive() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(1, "d6"));
    assert_eq!(scene.project.named_dims.len(), 2, "X and Y are different dimensions");
    assert_eq!(scene.project.param_map().get("d5"), Some(&20.0));
    assert_eq!(scene.project.param_map().get("d6"), Some(&15.0));
}

#[test]
fn naming_only_y_reads_y_not_the_first_constraint() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(1, "d6"));
    assert_eq!(scene.project.param_map().get("d6"), Some(&15.0));
}

#[test]
fn editing_y_does_not_change_x() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(1, "d6"));
    let target = scene.target_named("d6");
    assert!(scene.project.set_dim_target_value(&target, 17.0));
    assert_eq!(scene.value_at(0), 20.0, "editing Y must leave X alone");
    assert_eq!(scene.value_at(1), 17.0);
}

#[test]
fn both_targets_can_be_edited_independently() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(1, "d6"));
    let x = scene.target_named("d5");
    let y = scene.target_named("d6");
    assert!(scene.project.set_dim_target_value(&x, 22.0));
    assert!(scene.project.set_dim_target_value(&y, 17.0));
    assert_eq!(scene.value_at(0), 22.0);
    assert_eq!(scene.value_at(1), 17.0);
}

#[test]
fn reordering_constraints_does_not_retarget_x() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert_eq!(scene.project.param_map().get("d5"), Some(&20.0));
    scene.project.sketches[scene.si].constraints.swap(0, 1);
    assert_eq!(scene.project.param_map().get("d5"), Some(&20.0));
}

#[test]
fn ron_roundtrip_keeps_y_identity_and_edit() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(1, "d6"));
    let text = qymcad_core::model::to_ron(&scene.project).unwrap();
    scene.project = qymcad_core::model::from_ron(&text).unwrap();
    assert_eq!(scene.project.param_map().get("d6"), Some(&15.0));
    let target = scene.target_named("d6");
    assert!(scene.project.set_dim_target_value(&target, 17.0));
    assert_eq!(scene.value_at(0), 20.0);
    assert_eq!(scene.value_at(1), 17.0);
}

#[test]
fn deleting_named_x_does_not_retarget_its_name_to_y() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    let target = scene.target_named("d5");
    assert!(scene.project.delete_sketch_constraint(scene.si, 0));
    assert_eq!(scene.project.dim_target_value(&target), None);
    assert!(!scene.project.set_dim_target_value(&target, 22.0));
    assert_eq!(scene.value_at(0), 15.0);
    assert_eq!(scene.project.named_dims.len(), 1, "a missing driver remains diagnosable");
}

#[test]
fn another_axis_cannot_take_the_same_name() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(!scene.name_selected(1, "d5"), "another dimension cannot use the same driver name");
}

#[test]
fn name_validation_distinguishes_an_unbound_axis_from_its_named_neighbor() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(!scene.project.sketch_dimension_name_taken("d5", scene.selection(0)));
    assert!(scene.project.sketch_dimension_name_taken("d5", scene.selection(1)));
    assert!(!scene.project.sketch_dimension_name_taken("d6", scene.selection(1)));
    assert!(scene.name_selected(1, "d6"));
    assert!(scene.project.sketch_dimension_name_taken("d6", scene.selection(0)));
    assert!(!scene.project.sketch_dimension_name_taken("d6", scene.selection(1)));
}

#[test]
fn global_parameters_still_block_driver_names() {
    let mut scene = Scene::pair();
    scene.project.parameters.push(Param { name: "W1".into(), expr: "40".into(), value: 40.0 });
    assert!(!scene.name_selected(0, "W1"));
    assert!(scene.project.sketch_dimension_name_taken("W1", scene.selection(0)));
}

#[test]
fn renaming_the_same_dimension_remains_allowed() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(0, "Width"));
    assert_eq!(scene.project.named_dims.len(), 1);
    assert_eq!(scene.project.param_map().get("Width"), Some(&20.0));
    assert!(!scene.project.param_map().contains_key("d5"));
}

#[test]
fn reversing_distance_endpoints_keeps_the_name() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    if let Constraint::Distance { a, b, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        std::mem::swap(a, b);
    }
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "d5");
    assert_eq!(scene.project.param_map().get("d5"), Some(&20.0));
}

#[test]
fn reference_flag_changes_do_not_lose_names() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    scene.project.toggle_driven(scene.si, 0);
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "d5");
    assert_eq!(scene.project.param_map().get("d5"), Some(&20.0));
}

#[test]
fn diameter_radius_mode_change_keeps_its_name() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints.push(Constraint::Diameter {
        dimension: DimensionId::default(),
        c: scene.points[1],
        d: 10.0,
        off: 0.0,
        expr: String::new(),
        driven: false,
        diam: true,
        at: None,
    });
    assert!(scene.name_selected(0, "diam"));
    if let Constraint::Diameter { d, diam, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        *d *= 0.5;
        *diam = false;
    }
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "diam");
    assert_eq!(scene.project.param_map().get("diam"), Some(&5.0));
}

#[test]
fn changing_label_offset_does_not_lose_the_name() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    if let Constraint::Distance { off, at, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        *off = 3.0;
        *at = Some(0.7);
    }
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "d5");
}

#[test]
fn rotation_that_changes_axis_keeps_its_name() {
    let mut scene = Scene::empty();
    let eid = scene.project.add_line_entity(scene.si, 5.0, 5.0, 25.0, 5.0, Purpose::Real);
    let ends = scene.project.sketches[scene.si].entities.iter().find(|e| e.id == eid).unwrap().kind;
    let qymcad_core::model::EntityKind::Line { a, b } = ends else { panic!("the entity is a line") };
    scene.project.sketches[scene.si].constraints.push(Constraint::Distance { dimension: DimensionId::default(), a, b, d: 20.0, off: 3.0, expr: String::new(), driven: false, axis: 1, at: None });
    assert!(scene.name_selected(0, "side"));
    scene.project.rotate_entities(scene.si, &[eid], 15.0, 5.0, 30.0);
    assert!(matches!(scene.project.sketches[scene.si].constraints[0], Constraint::Distance { axis: 0, .. }));
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "side");
    assert_eq!(scene.project.param_map().get("side"), Some(&20.0));
}

#[test]
fn angle_and_point_line_distance_are_different_targets() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::Angle { dimension: DimensionId::default(), a: scene.points[0], b: scene.points[1], c: scene.points[2], deg: 37.0, expr: String::new(), driven: false, off: 0.0, at: None },
        Constraint::DistancePL { dimension: DimensionId::default(), p: scene.points[0], a: scene.points[1], b: scene.points[2], d: 7.0, off: 0.0, expr: String::new(), driven: false, at: None },
    ];
    assert!(scene.name_selected(0, "angle"));
    assert!(scene.name_selected(1, "gap"));
    assert_eq!(scene.project.named_dims.len(), 2, "different dimensions may use the same points");
    assert_eq!(scene.project.param_map().get("angle"), Some(&37.0));
    assert_eq!(scene.project.param_map().get("gap"), Some(&7.0));
}

#[test]
fn angle_vertex_role_is_not_an_unordered_set() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::Angle { dimension: DimensionId::default(), a: scene.points[0], b: scene.points[1], c: scene.points[2], deg: 37.0, expr: String::new(), driven: false, off: 0.0, at: None },
        Constraint::Angle { dimension: DimensionId::default(), a: scene.points[1], b: scene.points[0], c: scene.points[2], deg: 53.0, expr: String::new(), driven: false, off: 0.0, at: None },
    ];
    assert!(scene.name_selected(0, "first"));
    assert!(scene.name_selected(1, "second"));
    assert_eq!(scene.project.named_dims.len(), 2, "an angle retains its vertex role");
}

#[test]
fn reversing_angle_rays_keeps_the_name() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints.push(Constraint::Angle {
        dimension: DimensionId::default(),
        a: scene.points[0],
        b: scene.points[1],
        c: scene.points[2],
        deg: 37.0,
        expr: String::new(),
        driven: false,
        off: 0.0,
        at: None,
    });
    assert!(scene.name_selected(0, "angle"));
    if let Constraint::Angle { a, c, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        std::mem::swap(a, c);
    }
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "angle");
}

#[test]
fn point_line_distance_retains_the_point_role() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::DistancePL { dimension: DimensionId::default(), p: scene.points[0], a: scene.points[1], b: scene.points[2], d: 7.0, off: 0.0, expr: String::new(), driven: false, at: None },
        Constraint::DistancePL { dimension: DimensionId::default(), p: scene.points[1], a: scene.points[0], b: scene.points[2], d: 9.0, off: 0.0, expr: String::new(), driven: false, at: None },
    ];
    assert!(scene.name_selected(0, "first"));
    assert!(scene.name_selected(1, "second"));
    assert_eq!(scene.project.named_dims.len(), 2);
}

#[test]
fn line_angle_retains_segment_pair_grouping() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::AngleLines {
            dimension: DimensionId::default(),
            a: scene.points[0],
            b: scene.points[1],
            c: scene.points[2],
            d: scene.points[3],
            deg: 37.0,
            expr: String::new(),
            driven: false,
            off: 0.0,
            at: None,
        },
        Constraint::AngleLines {
            dimension: DimensionId::default(),
            a: scene.points[0],
            b: scene.points[2],
            c: scene.points[1],
            d: scene.points[3],
            deg: 53.0,
            expr: String::new(),
            driven: false,
            off: 0.0,
            at: None,
        },
    ];
    assert!(scene.name_selected(0, "first"));
    assert!(scene.name_selected(1, "second"));
    assert_eq!(scene.project.named_dims.len(), 2);
}

#[test]
fn reversing_one_line_retains_supplementary_angle_identity() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::AngleLines {
            dimension: DimensionId::default(),
            a: scene.points[0],
            b: scene.points[1],
            c: scene.points[2],
            d: scene.points[3],
            deg: 37.0,
            expr: String::new(),
            driven: false,
            off: 0.0,
            at: None,
        },
        Constraint::AngleLines {
            dimension: DimensionId::default(),
            a: scene.points[1],
            b: scene.points[0],
            c: scene.points[2],
            d: scene.points[3],
            deg: 143.0,
            expr: String::new(),
            driven: false,
            off: 0.0,
            at: None,
        },
    ];
    assert!(scene.name_selected(0, "first"));
    assert!(scene.name_selected(1, "second"));
    assert_eq!(scene.project.named_dims.len(), 2, "reversing one directed segment changes the angle");
}

#[test]
fn complementary_arcs_do_not_share_a_driver() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::ArcLength { dimension: DimensionId::default(), c: scene.points[0], a: scene.points[1], b: scene.points[2], ccw: true, len: 10.0, off: 0.0, expr: String::new(), driven: false },
        Constraint::ArcLength { dimension: DimensionId::default(), c: scene.points[0], a: scene.points[1], b: scene.points[2], ccw: false, len: 30.0, off: 0.0, expr: String::new(), driven: false },
    ];
    assert!(scene.name_selected(0, "minor"));
    assert!(scene.name_selected(1, "major"));
    assert_eq!(scene.project.named_dims.len(), 2);
}

#[test]
fn reversing_arc_ends_and_direction_keeps_the_name() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints.push(Constraint::ArcLength {
        dimension: DimensionId::default(),
        c: scene.points[0],
        a: scene.points[1],
        b: scene.points[2],
        ccw: true,
        len: 10.0,
        off: 0.0,
        expr: String::new(),
        driven: false,
    });
    assert!(scene.name_selected(0, "arc"));
    if let Constraint::ArcLength { a, b, ccw, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        std::mem::swap(a, b);
        *ccw = !*ccw;
    }
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "arc");
}

#[test]
fn participant_merge_keeps_the_dimension_name() {
    let mut scene = Scene::pair();
    let duplicate = scene.project.sketch_point_at(scene.si, 20.0001, 15.0, 1e-6);
    if let Constraint::Distance { a, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        *a = duplicate;
    }
    assert!(scene.name_selected(0, "d5"));
    assert_eq!(scene.project.merge_close_points(scene.si, 0.001), 1);
    assert_eq!(scene.project.param_map().get("d5"), Some(&20.0));
}

#[test]
fn aligned_distance_is_not_an_axis_projection() {
    let mut scene = Scene::pair();
    let mut aligned = scene.distance(Axis::Horizontal, 25.0);
    if let Constraint::Distance { axis, .. } = &mut aligned {
        *axis = 0;
    }
    scene.project.sketches[scene.si].constraints.push(aligned);
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(2, "aligned"));
    assert_ne!(scene.selected_target(0), scene.selected_target(2));
}

#[test]
fn numeric_edit_clears_only_the_selected_expression() {
    let mut scene = Scene::pair();
    if let Constraint::Distance { expr, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        *expr = "W1/2".into();
    }
    if let Constraint::Distance { expr, .. } = &mut scene.project.sketches[scene.si].constraints[1] {
        *expr = "H1/2".into();
    }
    assert!(scene.name_selected(1, "d6"));
    let target = scene.target_named("d6");
    assert!(scene.project.set_dim_target_value(&target, 17.0));
    assert_eq!(scene.project.sketches[scene.si].constraints[0].dim_expr(), Some("W1/2"));
    assert_eq!(scene.project.sketches[scene.si].constraints[1].dim_expr(), Some(""));
}

#[test]
fn arc_center_is_not_an_endpoint() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::ArcLength { dimension: DimensionId::default(), c: scene.points[0], a: scene.points[1], b: scene.points[2], ccw: true, len: 10.0, off: 0.0, expr: String::new(), driven: false },
        Constraint::ArcLength { dimension: DimensionId::default(), c: scene.points[1], a: scene.points[0], b: scene.points[2], ccw: true, len: 30.0, off: 0.0, expr: String::new(), driven: false },
    ];
    assert!(scene.name_selected(0, "first"));
    assert!(scene.name_selected(1, "second"));
    assert_eq!(scene.project.named_dims.len(), 2);
}

#[test]
fn different_edge_modes_do_not_share_a_driver() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints = vec![
        Constraint::EdgeDistance { dimension: DimensionId::default(), c1: scene.points[0], c2: scene.points[1], d: 3.0, m1: 1, m2: -1, off: 0.0, expr: String::new(), driven: false, at: None },
        Constraint::EdgeDistance { dimension: DimensionId::default(), c1: scene.points[0], c2: scene.points[1], d: 27.0, m1: -1, m2: 1, off: 0.0, expr: String::new(), driven: false, at: None },
    ];
    assert!(scene.name_selected(0, "near"));
    assert!(scene.name_selected(1, "far"));
    assert_eq!(scene.project.named_dims.len(), 2);
}

#[test]
fn swapping_line_pairs_and_reversing_both_keeps_the_name() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints.push(Constraint::AngleLines {
        dimension: DimensionId::default(),
        a: scene.points[0],
        b: scene.points[1],
        c: scene.points[2],
        d: scene.points[3],
        deg: 37.0,
        expr: String::new(),
        driven: false,
        off: 0.0,
        at: None,
    });
    assert!(scene.name_selected(0, "angle"));
    if let Constraint::AngleLines { a, b, c, d, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        std::mem::swap(a, c);
        std::mem::swap(b, d);
        std::mem::swap(a, b);
        std::mem::swap(c, d);
    }
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "angle");
}

#[test]
fn distinct_reference_record_does_not_rename_the_driving_record() {
    let mut scene = Scene::pair();
    let mut reference = scene.distance(Axis::Horizontal, 20.0);
    if let Constraint::Distance { driven, .. } = &mut reference {
        *driven = true;
    }
    scene.project.sketches[scene.si].constraints.push(reference);
    assert!(scene.name_selected(0, "driver"));
    assert!(scene.name_selected(2, "measure"));
    assert_eq!(scene.project.named_dims.len(), 2, "two selected records retain separate identities");
}

#[test]
fn a_new_equal_dimension_does_not_reanimate_a_deleted_driver() {
    let mut scene = Scene::pair();
    scene.project.sketches[scene.si].constraints.remove(1);
    assert!(scene.name_selected(0, "d5"));
    let old_target = scene.target_named("d5");
    assert!(scene.project.delete_sketch_constraint(scene.si, 0));
    let replacement = scene.distance(Axis::Horizontal, 20.0);
    scene.project.sketches[scene.si].constraints.push(replacement);
    assert_eq!(scene.project.dim_target_value(&old_target), None, "a newly created record is not the deleted record");
}

#[test]
fn unnamed_dimensions_have_no_bound_target() {
    let scene = Scene::pair();
    assert_eq!(scene.identity_at(0), DimensionId::default());
    assert_eq!(scene.identity_at(1), DimensionId::default());
    assert_eq!(scene.project.sketch_dimension_target(scene.selection(0)), None);
    assert_eq!(scene.project.sketch_dimension_target(scene.selection(1)), None);
}

#[test]
fn rejected_and_empty_names_do_not_allocate_identity() {
    let mut scene = Scene::pair();
    scene.project.parameters.push(Param { name: "W1".into(), expr: "40".into(), value: 40.0 });
    let before = qymcad_core::model::to_ron(&scene.project).unwrap();
    assert!(scene.project.sketch_dimension_name_taken("W1", scene.selection(0)));
    for name in ["W1", "", "  ", "1width", "two words", "width.height", "width+height"] {
        assert!(!scene.name_selected(0, name), "the rejected name must not bind the dimension: {name:?}");
        assert_eq!(qymcad_core::model::to_ron(&scene.project).unwrap(), before, "rejecting the name leaves the document unchanged: {name:?}");
    }
    assert_eq!(scene.identity_at(0), DimensionId::default());
}

#[test]
fn plain_and_missing_constraints_cannot_be_named() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints.push(Constraint::Fixed { p: scene.points[0] });
    let plain = scene.selection(0);
    let missing = scene.selection(7);
    assert_eq!(scene.project.sketches[scene.si].constraints[0].dimension_id(), None);
    assert_eq!(scene.project.sketch_dimension_target(plain), None);
    assert_eq!(scene.project.sketch_dimension_target(missing), None);
    assert!(!scene.project.name_sketch_dimension("plain".into(), plain));
    assert!(!scene.project.name_sketch_dimension("missing".into(), missing));
    assert!(scene.project.named_dims.is_empty());
}

#[test]
fn clearing_a_name_keeps_record_identity_for_later_naming() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    let dimension = scene.identity_at(0);
    assert_ne!(dimension, DimensionId::default());
    assert!(!scene.name_selected(0, ""));
    assert!(scene.project.named_dims.is_empty());
    assert_eq!(scene.identity_at(0), dimension);
    assert!(scene.name_selected(0, "Width"));
    assert_eq!(scene.identity_at(0), dimension);
    assert_eq!(scene.project.param_map().get("Width"), Some(&20.0));
}

#[test]
fn an_unbound_target_cannot_read_or_edit_an_unnamed_dimension() {
    let mut scene = Scene::pair();
    let target = DimTarget::Sketch { sketch: scene.project.sketches[scene.si].id, dimension: DimensionId::default() };
    assert_eq!(scene.project.dim_target_value(&target), None);
    assert!(!scene.project.set_dim_target_value(&target, 22.0));
    assert_eq!(scene.value_at(0), 20.0);
    assert_eq!(scene.value_at(1), 15.0);
}

#[test]
fn duplicate_ids_are_refused_without_retargeting_or_mutation() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    let target = scene.target_named("d5");
    let duplicate = scene.identity_at(0);
    if let Constraint::Distance { dimension, .. } = &mut scene.project.sketches[scene.si].constraints[1] {
        *dimension = duplicate;
    }
    let before = qymcad_core::model::to_ron(&scene.project).unwrap();
    assert_eq!(scene.project.sketch_dimension_target(scene.selection(0)), None);
    assert_eq!(scene.project.sketch_dimension_target(scene.selection(1)), None);
    assert_eq!(scene.project.dim_target_value(&target), None);
    assert!(!scene.project.set_dim_target_value(&target, 22.0));
    assert!(!scene.name_selected(1, "duplicate"));
    assert!(!scene.project.param_map().contains_key("d5"));
    assert_eq!(scene.value_at(0), 20.0);
    assert_eq!(scene.value_at(1), 15.0);
    assert_eq!(scene.project.named_dims.len(), 1);
    assert_eq!(scene.project.named_dims[0].name, "d5");
    assert_eq!(qymcad_core::model::to_ron(&scene.project).unwrap(), before, "refusing duplicate identities leaves the whole document unchanged");
}

#[test]
fn naming_with_a_colliding_next_id_is_refused_without_mutation() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    let dimension = scene.identity_at(0);
    scene.project.next_id = dimension.0 - 1;
    let before = qymcad_core::model::to_ron(&scene.project).unwrap();
    assert!(!scene.name_selected(1, "d6"), "the candidate identity already belongs to the first dimension");
    assert_eq!(qymcad_core::model::to_ron(&scene.project).unwrap(), before, "refusing a colliding candidate leaves the whole document unchanged");
    assert_eq!(scene.identity_at(1), DimensionId::default());
    assert_eq!(scene.project.param_map().get("d5"), Some(&20.0));
    assert_eq!(scene.project.named_dims.len(), 1);
}

#[test]
fn naming_cannot_reuse_the_identity_of_a_deleted_driver() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    let dimension = scene.identity_at(0);
    let deleted = scene.target_named("d5");
    assert!(scene.project.delete_sketch_constraint(scene.si, 0));
    scene.project.next_id = dimension.0 - 1;
    let before = qymcad_core::model::to_ron(&scene.project).unwrap();
    assert!(!scene.name_selected(0, "d6"), "the candidate identity is still reserved by the deleted driver's target");
    assert_eq!(qymcad_core::model::to_ron(&scene.project).unwrap(), before, "a deleted driver's identity cannot be assigned to another record");
    assert_eq!(scene.identity_at(0), DimensionId::default());
    assert_eq!(scene.project.dim_target_value(&deleted), None);
    assert_eq!(scene.target_named("d5"), deleted);
    assert_eq!(scene.value_at(0), 15.0);
}

#[test]
fn both_bound_ids_survive_serialization_and_independent_edits() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(1, "d6"));
    let x = scene.identity_at(0);
    let y = scene.identity_at(1);
    assert_ne!(x, y);
    assert_ne!(x, DimensionId::default());
    assert_ne!(y, DimensionId::default());
    scene.project = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&scene.project).unwrap()).unwrap();
    assert_eq!(scene.identity_at(0), x);
    assert_eq!(scene.identity_at(1), y);
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "d5");
    assert_eq!(scene.project.name_of_target(&scene.selected_target(1)), "d6");
    assert!(scene.project.set_dim_target_value(&scene.selected_target(0), 22.0));
    assert!(scene.project.set_dim_target_value(&scene.selected_target(1), 17.0));
    assert_eq!(scene.value_at(0), 22.0);
    assert_eq!(scene.value_at(1), 17.0);
}

#[test]
fn copying_geometry_resets_only_the_copied_dimension_identity() {
    let mut line = Scene::line();
    assert!(line.scene.name_selected(0, "source"));
    let source = line.scene.identity_at(0);
    let copied = line.scene.project.copy_entities(line.scene.si, &[line.entity], 40.0, 0.0);
    assert_eq!(copied.len(), 1);
    assert_eq!(line.scene.project.sketches[line.scene.si].constraints.len(), 2);
    assert_eq!(line.scene.identity_at(0), source);
    assert_eq!(line.scene.identity_at(1), DimensionId::default());
    assert!(line.scene.name_selected(1, "copy"));
    assert_ne!(line.scene.identity_at(1), source);
    assert!(line.scene.project.set_dim_target_value(&line.scene.selected_target(1), 24.0));
    assert_eq!(line.scene.project.param_map().get("source"), Some(&20.0));
    assert_eq!(line.scene.project.param_map().get("copy"), Some(&24.0));
}

#[test]
fn pasting_the_same_clip_twice_does_not_reuse_bound_ids() {
    let mut line = Scene::line();
    assert!(line.scene.name_selected(0, "source"));
    let source = line.scene.identity_at(0);
    let clip = line.scene.project.copy_sketch_geometry(line.scene.si, &[line.entity], 5.0, 5.0);
    assert_eq!(line.scene.project.paste_sketch_geometry(line.scene.si, &clip, 45.0, 5.0).len(), 1);
    assert_eq!(line.scene.project.paste_sketch_geometry(line.scene.si, &clip, 85.0, 5.0).len(), 1);
    assert_eq!(line.scene.identity_at(0), source);
    assert_eq!(line.scene.identity_at(1), DimensionId::default());
    assert_eq!(line.scene.identity_at(2), DimensionId::default());
    assert!(line.scene.name_selected(1, "first_copy"));
    assert!(line.scene.name_selected(2, "second_copy"));
    assert_ne!(line.scene.identity_at(1), source);
    assert_ne!(line.scene.identity_at(2), source);
    assert_ne!(line.scene.identity_at(1), line.scene.identity_at(2));
    assert_eq!(line.scene.project.named_dims.len(), 3);
}

#[test]
fn mirror_and_pattern_copies_leave_the_source_binding_alone() {
    let mut line = Scene::line();
    assert!(line.scene.name_selected(0, "source"));
    let source = line.scene.identity_at(0);
    line.scene.project.mirror_entities(line.scene.si, &[line.entity], 0.0, 0.0, 0.0, 10.0);
    line.scene.project.add_pattern(line.scene.si, &[line.entity], PatternKind::Linear { dx: 40.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 0.0, count2: 1 });
    assert_eq!(line.scene.project.sketches[line.scene.si].constraints.len(), 1, "these geometry copies do not carry dimensions");
    assert_eq!(line.scene.identity_at(0), source);
    assert_eq!(line.scene.project.named_dims.len(), 1);
    assert_eq!(line.scene.project.param_map().get("source"), Some(&20.0));
}

#[test]
fn a_whole_sketch_clone_has_a_separate_binding_namespace() {
    let mut line = Scene::line();
    assert!(line.scene.name_selected(0, "source"));
    let source = line.scene.identity_at(0);
    let target_parent = line.scene.project.add_part("Target");
    let cloned = line.scene.project.clone_sketch_node(line.scene.project.sketches[line.scene.si].id, target_parent).expect("the sketch can be cloned");
    let clone_index = line.scene.project.sketch_index(cloned).unwrap();
    let selection = SketchDimensionSelection { sketch: cloned, index: 0 };
    assert_eq!(line.scene.project.sketches[clone_index].constraints[0].dimension_id(), Some(source));
    let clone_target = line.scene.project.sketch_dimension_target(selection).expect("the cloned record has a unique identity in its sketch");
    assert_ne!(clone_target, line.scene.selected_target(0));
    assert_eq!(line.scene.project.name_of_target(&clone_target), "");
    assert!(!line.scene.project.name_sketch_dimension("source".into(), selection));
    assert!(line.scene.project.name_sketch_dimension("clone".into(), selection));
    assert!(line.scene.project.set_dim_target_value(&clone_target, 24.0));
    assert_eq!(line.scene.project.param_map().get("source"), Some(&20.0));
    assert_eq!(line.scene.project.param_map().get("clone"), Some(&24.0));
}

#[test]
fn rotation_keeps_two_names_when_measurement_modes_become_equal() {
    let mut line = Scene::line();
    let mut reference = line.scene.project.sketches[line.scene.si].constraints[0].clone();
    if let Constraint::Distance { axis, driven, .. } = &mut reference {
        *axis = 0;
        *driven = true;
    }
    line.scene.project.sketches[line.scene.si].constraints.push(reference);
    assert!(line.scene.name_selected(0, "horizontal"));
    assert!(line.scene.name_selected(1, "aligned"));
    let first = line.scene.identity_at(0);
    let second = line.scene.identity_at(1);
    line.scene.project.rotate_entities(line.scene.si, &[line.entity], 15.0, 5.0, 30.0);
    assert!(matches!(line.scene.project.sketches[line.scene.si].constraints[0], Constraint::Distance { axis: 0, .. }));
    assert!(matches!(line.scene.project.sketches[line.scene.si].constraints[1], Constraint::Distance { axis: 0, .. }));
    assert_eq!(line.scene.identity_at(0), first);
    assert_eq!(line.scene.identity_at(1), second);
    assert_ne!(line.scene.selected_target(0), line.scene.selected_target(1));
    assert_eq!(line.scene.project.name_of_target(&line.scene.selected_target(0)), "horizontal");
    assert_eq!(line.scene.project.name_of_target(&line.scene.selected_target(1)), "aligned");
}

#[test]
fn changing_near_far_mode_keeps_the_same_record_name() {
    let mut scene = Scene::empty();
    scene.project.sketches[scene.si].constraints.push(Constraint::EdgeDistance {
        dimension: DimensionId::default(),
        c1: scene.points[0],
        c2: scene.points[1],
        d: 3.0,
        m1: 1,
        m2: -1,
        off: 0.0,
        expr: String::new(),
        driven: false,
        at: None,
    });
    assert!(scene.name_selected(0, "edge"));
    let dimension = scene.identity_at(0);
    if let Constraint::EdgeDistance { m1, m2, d, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        *m1 = -1;
        *m2 = 1;
        *d = 27.0;
    }
    assert_eq!(scene.identity_at(0), dimension);
    assert_eq!(scene.project.name_of_target(&scene.selected_target(0)), "edge");
    assert_eq!(scene.project.param_map().get("edge"), Some(&27.0));
}

#[test]
fn detaching_geometry_from_the_origin_keeps_dimension_identity() {
    let mut scene = Scene::empty();
    let entity = scene.project.add_line_entity(scene.si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    let origin = scene.project.ensure_origin(scene.si);
    let EntityKind::Line { b, .. } = scene.project.sketches[scene.si].entities.iter().find(|e| e.id == entity).unwrap().kind else {
        panic!("the selected entity is a line");
    };
    if let EntityKind::Line { a, .. } = &mut scene.project.sketches[scene.si].entities.iter_mut().find(|e| e.id == entity).unwrap().kind {
        *a = origin;
    }
    let ci = scene.project.sketches[scene.si].constraints.len();
    scene.project.sketches[scene.si].constraints.push(Constraint::Distance {
        dimension: DimensionId::default(),
        a: origin,
        b,
        d: 20.0,
        off: 0.0,
        expr: String::new(),
        driven: false,
        axis: 1,
        at: None,
    });
    assert!(scene.name_selected(ci, "side"));
    let dimension = scene.identity_at(ci);
    assert!(scene.project.detach_geometry_from_origin(scene.si));
    assert_eq!(scene.identity_at(ci), dimension);
    assert_eq!(scene.project.name_of_target(&scene.selected_target(ci)), "side");
    assert_eq!(scene.project.param_map().get("side"), Some(&20.0));
    let Constraint::Distance { a, .. } = scene.project.sketches[scene.si].constraints[ci] else {
        panic!("the selected constraint is a distance");
    };
    assert_ne!(a, origin);
}

#[test]
fn renaming_a_driver_preserves_targets_and_updates_its_formula() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(1, "d6"));
    let x = scene.selected_target(0);
    let y = scene.selected_target(1);
    scene.project.parameters.push(Param { name: "sum".into(), expr: "d5 + d6".into(), value: 35.0 });
    assert_eq!(scene.project.rename_driver("d5", "Width").unwrap(), 1);
    assert_eq!(scene.target_named("Width"), x);
    assert_eq!(scene.target_named("d6"), y);
    assert_eq!(scene.project.parameters[0].expr, "Width + d6");
    assert_eq!(scene.project.param_map().get("Width"), Some(&20.0));
    assert_eq!(scene.project.param_map().get("d6"), Some(&15.0));
}

#[test]
fn edge_gap_creation_measures_the_rims_and_binds_lazily() {
    let mut scene = Scene::empty();
    let first_entity = scene.project.add_circle_entity(scene.si, 0.0, 0.0, 3.0, Purpose::Real);
    let second_entity = scene.project.add_circle_entity(scene.si, 20.0, 0.0, 5.0, Purpose::Real);
    let circle_center = |entity| match scene.project.sketches[scene.si].entities.iter().find(|e| e.id == entity).expect("the circle exists").kind {
        EntityKind::Circle { center, .. } => center,
        _ => panic!("the selected entity is a circle"),
    };
    let first = circle_center(first_entity);
    let second = circle_center(second_entity);
    let near = scene.project.add_edge_dimension(scene.si, [EdgeDimensionEnd { center: first, mode: -1 }, EdgeDimensionEnd { center: second, mode: -1 }]);
    let far = scene.project.add_edge_dimension(scene.si, [EdgeDimensionEnd { center: first, mode: -1 }, EdgeDimensionEnd { center: second, mode: 1 }]);
    assert_eq!(scene.value_at(near), 12.0, "the near rims leave 20 - 3 - 5 = 12 mm");
    assert_eq!(scene.value_at(far), 22.0, "the opposite rim measures 20 - 3 + 5 = 22 mm");
    for ci in [near, far] {
        assert_eq!(scene.identity_at(ci), DimensionId::default());
        assert_eq!(scene.project.sketch_dimension_target(scene.selection(ci)), None);
    }
    let Constraint::EdgeDistance { c1, c2, m1, m2, off, expr, driven, at, .. } = &scene.project.sketches[scene.si].constraints[near] else {
        panic!("the created record is an edge-gap dimension");
    };
    assert_eq!(*c1, first);
    assert_eq!(*c2, second);
    assert_eq!(*m1, -1);
    assert_eq!(*m2, -1);
    assert_eq!(*off, 0.0);
    assert!(expr.is_empty());
    assert!(!driven);
    assert_eq!(*at, None);
    assert!(scene.name_selected(near, "near_gap"));
    assert!(scene.name_selected(far, "far_gap"));
    assert_ne!(scene.selected_target(near), scene.selected_target(far));
    assert!(scene.project.set_dim_target_value(&scene.selected_target(near), 14.0));
    assert_eq!(scene.project.param_map().get("near_gap"), Some(&14.0));
    assert_eq!(scene.project.param_map().get("far_gap"), Some(&22.0));
}

#[test]
fn missing_record_and_target_ids_load_as_unbound_without_inference() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    assert!(scene.name_selected(1, "d6"));
    let text = qymcad_core::model::to_ron(&scene.project).unwrap();
    let without_ids = text.lines().filter(|line| !line.trim_start().starts_with("dimension:")).collect::<Vec<_>>().join("\n");
    assert_ne!(text, without_ids, "the fixture must remove the persisted identities");
    scene.project = qymcad_core::model::from_ron(&without_ids).unwrap();
    assert_eq!(scene.identity_at(0), DimensionId::default());
    assert_eq!(scene.identity_at(1), DimensionId::default());
    assert_eq!(scene.project.named_dims.len(), 2, "missing drivers stay visible for diagnosis");
    for name in ["d5", "d6"] {
        let target = scene.target_named(name);
        assert_eq!(target, DimTarget::Sketch { sketch: scene.project.sketches[scene.si].id, dimension: DimensionId::default() });
        assert_eq!(scene.project.dim_target_value(&target), None);
        assert!(!scene.project.set_dim_target_value(&target, 99.0));
        assert!(!scene.project.param_map().contains_key(name));
        let row = scene.project.drivers().into_iter().find(|row| row.name == name).expect("the missing driver has a diagnostic row");
        assert_eq!(row.value, None);
    }
    assert_eq!(scene.value_at(0), 20.0);
    assert_eq!(scene.value_at(1), 15.0);
}

#[test]
fn participant_only_targets_are_unbound_even_when_the_points_match() {
    let mut scene = Scene::pair();
    let text = format!("Sketch(sketch:{},refs:[{},{}])", scene.project.sketches[scene.si].id, scene.points[0], scene.points[1]);
    let target = ron::from_str::<DimTarget>(&text).expect("an absent record identity uses its default");
    assert_eq!(target, DimTarget::Sketch { sketch: scene.project.sketches[scene.si].id, dimension: DimensionId::default() });
    scene.project.named_dims.push(NamedDim { name: "unbound".into(), target: target.clone() });
    assert_eq!(scene.project.dim_target_value(&target), None);
    assert!(!scene.project.set_dim_target_value(&target, 99.0));
    assert!(!scene.project.param_map().contains_key("unbound"));
    assert_eq!(scene.project.named_dims[0].name, "unbound");
    let row = scene.project.drivers().into_iter().find(|row| row.name == "unbound").expect("the missing driver has a diagnostic row");
    assert_eq!(row.value, None);
    assert_eq!(scene.value_at(0), 20.0);
    assert_eq!(scene.value_at(1), 15.0);
}

#[test]
fn every_dimensional_constraint_defaults_an_absent_identity_to_unbound() {
    let records = [
        "Distance(a:1,b:2,d:20.0,off:0.0,expr:\"\",driven:false,axis:1,at:None)",
        "Diameter(c:1,d:20.0,off:0.0,expr:\"\",driven:false,diam:true,at:None)",
        "Angle(a:1,b:2,c:3,deg:20.0,off:0.0,expr:\"\",driven:false,at:None)",
        "AngleLines(a:1,b:2,c:3,d:4,deg:20.0,off:0.0,expr:\"\",driven:false,at:None)",
        "DistancePL(p:1,a:2,b:3,d:20.0,off:0.0,expr:\"\",driven:false,at:None)",
        "EdgeDistance(c1:1,c2:2,d:20.0,m1:-1,m2:1,off:0.0,expr:\"\",driven:false,at:None)",
        "ArcLength(c:1,a:2,b:3,ccw:true,len:20.0,off:0.0,expr:\"\",driven:false)",
    ];
    for text in records {
        let constraint = ron::from_str::<Constraint>(text).expect("an absent record identity uses its default");
        assert_eq!(constraint.dimension_id(), Some(DimensionId::default()), "the absent identity defaults to unbound: {text}");
        assert_eq!(Project::dim_value_of(&constraint), Some(20.0), "the dimension value survives: {text}");
    }
}

#[test]
fn naming_a_dimension_changes_the_document_key() {
    let mut scene = Scene::pair();
    let before = scene.project.state_key();
    assert!(scene.name_selected(0, "d5"));
    assert_ne!(scene.project.state_key(), before, "binding a dimension is a persisted edit");
}

#[test]
fn naming_an_unused_dimension_keeps_the_sketch_geometry_key() {
    let mut scene = Scene::pair();
    let sketch = scene.project.sketches[scene.si].id;
    let geometry_before = scene.project.sketch_key(sketch);
    let document_before = scene.project.state_key();
    assert!(scene.name_selected(0, "unused_driver"));
    assert_ne!(scene.project.state_key(), document_before, "binding a dimension remains a persisted edit");
    assert_eq!(scene.project.sketch_key(sketch), geometry_before, "naming an unused dimension must not rebuild the bodies on its sketch");
}

#[test]
fn changing_only_a_record_identity_changes_the_document_key() {
    let mut scene = Scene::pair();
    assert!(scene.name_selected(0, "d5"));
    let before = scene.project.state_key();
    let target = scene.target_named("d5");
    if let Constraint::Distance { dimension, .. } = &mut scene.project.sketches[scene.si].constraints[0] {
        *dimension = DimensionId(dimension.0 + 100);
    }
    assert_ne!(scene.project.state_key(), before, "a changed record identity changes the saved document");
    assert_eq!(scene.target_named("d5"), target);
    assert_eq!(scene.value_at(0), 20.0);
    assert_eq!(scene.value_at(1), 15.0);
    assert_eq!(scene.project.dim_target_value(&target), None);
}

#[test]
fn retargeting_between_equal_dimensions_changes_the_document_key() {
    let mut scene = Scene::pair();
    if let Constraint::Distance { d, .. } = &mut scene.project.sketches[scene.si].constraints[1] {
        *d = 20.0;
    }
    assert!(scene.name_selected(0, "first"));
    assert!(scene.name_selected(1, "second"));
    let first = scene.selected_target(0);
    let second = scene.selected_target(1);
    scene.project.named_dims.retain(|nd| nd.name == "first");
    let before = scene.project.state_key();
    assert_eq!(scene.project.dim_target_value(&first), Some(20.0));
    assert_eq!(scene.project.dim_target_value(&second), Some(20.0));
    scene.project.named_dims[0].target = second;
    assert_ne!(scene.project.state_key(), before, "the binding changes even when its displayed value does not");
    assert_eq!(scene.project.param_map().get("first"), Some(&20.0));
}
