//! THE GUI TESTS, lifted out of `gui.rs`.
//!
//! This is a CHILD module of `gui` rather than a neighbouring file: `App`'s private fields are visible to the
//! owning module and its descendants, so the headless flows keep working with the internal state directly.
// DO NOT REMOVE: rustc calls this import unused, yet without it the file does not compile - it is what lets the
// nested test modules see the `gui` names by the `super::` path (a private import is available to descendants).
#[allow(unused_imports)]
use super::*;

#[cfg(test)]
mod command_flow_tests;

#[cfg(test)]
mod section_clip_tests;

#[cfg(test)]
mod gizmo_ring_sign_tests;

#[cfg(test)]
mod section_drag_tests;

#[cfg(test)]
mod pick_depth_tests;

#[cfg(test)]
mod gizmo_math_tests;

#[cfg(test)]
mod sketch_conflict_ui_tests;

#[cfg(test)]
mod brep_warmup_tests;

#[cfg(test)]
mod sketch_plane_pick_frames_tests;

#[cfg(test)]
mod faces_part_in_light;

#[cfg(test)]
mod the_box_of_a_joint_is_laid_in_sections;

#[cfg(test)]
mod a_box_follows_what_it_sizes;

#[cfg(test)]
mod a_tool_with_nothing_to_apply_closes_plainly;

#[cfg(test)]
mod a_sketch_tool_outside_a_sketch_says_so;

#[cfg(test)]
mod the_doors_of_a_tool;

#[cfg(test)]
mod a_box_in_the_corner_stands_clear_of_a_bar;

#[cfg(test)]
mod the_floor_fades_round_the_view;

#[cfg(test)]
mod parts_far_apart_draw_clean;

#[cfg(test)]
mod the_camera_holds_after_a_far_view;

#[cfg(test)]
pub(in crate::gui) mod a_quick_rebuild_does_not_blink;

#[cfg(test)]
mod nothing_changed_rebuilds_nothing;

#[cfg(test)]
mod a_hinge_follows_its_parameter;

#[cfg(test)]
mod a_cut_through_both_ways_reopens_both_ways;

#[cfg(test)]
mod a_cut_on_a_datum_follows_its_parameter_in_the_window;

#[cfg(test)]
mod a_reference_found_as_the_other_piece_is_shown;

#[cfg(test)]
mod a_reference_lost_by_an_edit_is_shown;

#[cfg(test)]
mod every_tool_takes_every_curve;

#[cfg(test)]
mod two_fillets_meet_on_a_side;

#[cfg(test)]
mod a_figure_is_carried_whole;

#[cfg(test)]
mod a_figure_opens_its_size;

#[cfg(test)]
mod a_figure_shows_no_badge_of_its_own;

#[cfg(test)]
mod a_projected_ellipse_follows_its_body;

#[cfg(test)]
mod a_spline_survives_a_circle_deleted_by_hand;

#[cfg(test)]
mod a_reference_size_reads_the_curve_as_it_stands;

#[cfg(test)]
mod a_corner_at_a_crossing_keeps_its_two_sides;

#[cfg(test)]
mod a_fillet_of_a_spline_holds_when_dragged;

#[cfg(test)]
mod an_offset_follows_its_source;

#[cfg(test)]
mod the_offset_of_a_closed_spline_is_light;

#[cfg(test)]
mod a_point_snapped_to_a_centre_line_is_held_on_it;

#[cfg(test)]
mod a_circle_shows_its_centre_lines;

#[cfg(test)]
mod a_trim_preview_shows_any_curve;

#[cfg(test)]
mod a_curve_under_the_pointer_is_the_one_cut;

#[cfg(test)]
mod a_point_drawn_on_a_curve_is_held_on_it;

#[cfg(test)]
mod a_picked_curve_of_any_kind_is_lit;

#[cfg(test)]
mod a_corner_of_two_crossing_curves_is_taken_like_one_of_lines;

#[cfg(test)]
mod a_circle_is_drawn_in_each_of_its_modes;

#[cfg(test)]
mod a_spline_closes_on_its_first_node;

#[cfg(test)]
mod a_filleted_spline_drags_lightly;

#[cfg(test)]
mod a_ring_of_a_large_spline_is_picked_lightly;

#[cfg(test)]
mod the_copy_of_an_offset_shows_no_nodes;

#[cfg(test)]
mod a_line_drawn_by_a_curve_touches_it;

#[cfg(test)]
mod a_tangent_arc_goes_on_from_any_curve;

#[cfg(test)]
mod a_crossing_of_any_curve_is_snapped_to;

#[cfg(test)]
mod a_whole_ellipse_is_cut_as_any_curve;

#[cfg(test)]
mod a_blend_built_short_advises_what_builds;

#[cfg(test)]
mod the_box_of_a_rectangle_goes_with_its_tool;

#[cfg(test)]
mod a_feature_value_is_named_in_its_box;

#[cfg(test)]
mod a_number_of_the_bar_is_a_field_of_the_box;

#[cfg(test)]
mod the_boxes_look;

#[cfg(test)]
mod the_tool_in_hand_after_its_work;

#[cfg(test)]
mod the_key_of_the_tool_in_hand_puts_it_down;

#[cfg(test)]
mod finish_climbs_the_tree;

#[cfg(test)]
mod profiles_are_taken_by_a_box;

#[cfg(test)]
mod the_author_is_supported;

#[cfg(test)]
mod an_old_formula_is_named_on_opening;
