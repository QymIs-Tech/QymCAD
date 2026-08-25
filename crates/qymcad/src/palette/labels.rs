//! COLOUR GROUPS AND THEIR ORDER for the custom-scheme screen. THERE ARE NO TEXTS HERE.
//!
//! The labels live in the language catalogue (`scheme-group-*`, `scheme-color-*`), like everything a
//! person reads. Keeping them in the code would create a second home for interface strings, and the
//! scheme screen would stay in one language forever.
//!
//! THIS IS THE THIRD LIST OF FIELDS (after the structure and the readers), and it is written by hand,
//! so it will drift unless it is guarded. A test cross-checks it against `entries()` in both
//! directions AND demands a translation for EVERY key in EVERY language.

/// Editor groups: the group key and the colour keys within it, in the order the scheme declares them.
/// A group label is `scheme-group-<key>`, a colour label is `scheme-color-<key>`.
pub fn groups() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("window", vec![
            "viewport_bg", "toolbar_bg", "panel_bg", "panel_border", "splash_bg", "thumbnail_bg", "scrim", "text_strong", "text_dim", "text_faint", "emphasis", "glyph_text", "glyph_backing",
        ]),
        ("grid", vec![
            "grid", "grid_minor", "axis_x", "axis_y", "axis_z", "grid_axis_x", "grid_axis_y", "grid_axis_z", "sketch_axis_x", "sketch_axis_y", "sketch_axis_idle",
        ]),
        ("sketch", vec![
            "sketch_line", "sketch_construction", "sketch_driven", "sketch_face_edge", "sketch_edge_3d",
        ]),
        ("dims", vec![
            "dimension", "dimension_driven", "dim_helper", "dim_helper_ring",
        ]),
        ("select", vec![
            "selected", "highlight", "active", "handle", "handle_face", "snap_point", "axis_pick_idle", "clip", "plate_text", "pattern_center", "sketch_point", "annotation", "note", "hint", "hint_action", "tree_selected", "connector", "rollback", "select_window", "select_cross", "rubber_band",
        ]),
        ("snap", vec![
            "snap_marker", "snap_intersection", "snap_edge", "snap_axis", "snap_grid",
        ]),
        ("body", vec![
            "body_face", "body_ghost", "body_clash", "edge_idle", "ghost_target",
        ]),
        ("faces", vec![
            "add", "remove", "modify", "reference", "offset_in",
        ]),
        ("planes", vec![
            "plane_face", "plane_fill", "plane_idle", "plane_normal", "datum_point", "datum_axis",
        ]),
        ("preview", vec![
            "preview", "preview_prim", "preview_array", "preview_axis", "preview_datum", "measure", "cut_line", "gizmo_label",
        ]),
        ("constraints", vec![
            "constraint_ok", "constraint_selected", "constraint_hover",
        ]),
        ("assembly", vec![
            "joint_idle", "joint_hover", "joint_pick_a", "joint_pick_b", "grounded",
        ]),
        ("contours", vec![
            "contour_idle", "contour_hover", "contour_profile",
        ]),
        ("states", vec![
            "ok", "ok_soft", "underdefined", "warning", "error", "error_mild",
        ]),
        ("viewcube", vec![
            "viewcube_face", "viewcube_edge",
        ]),
        // THE INTERFACE COMES LAST: it only takes effect when "colour the interface" is ticked, and
        // a person simply picking a sketch line colour does not need it.
        ("ui", vec![
            "ui_window", "ui_field", "ui_stripe", "ui_outline", "ui_control", "ui_control_hover", "ui_control_active", "ui_accent", "ui_text", "ui_text_dim", "ui_text_strong", "ui_link",
        ]),
        ("cam", vec![
            "cam_plunge", "cam_rapid", "cam_table", "cam_table_grid", "cam_stock", "cam_stock_idle", "cam_op1", "cam_op2", "cam_op3", "cam_op4", "cam_op5", "cam_op6",
        ]),
    ]
}
