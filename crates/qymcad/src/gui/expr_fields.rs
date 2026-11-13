//! A FORMULA IS ACCEPTED EVERYWHERE A NUMBER IS ENTERED.
//!
//! Reported behaviour: parameters cannot be used in the top bar, nor in many other places — the
//! sketcher too has tools where no parameter or formula can be entered. And so it was: the
//! application had exactly one field that accepted TEXT — the parameter popup at the geometry.
//! Everything else was a `DragValue`, where a formula cannot be entered at all: the copy count of a
//! pattern, the number of sides of a polygon, the radius of a sketch fillet, the offset distance, the
//! parameters of sketch patterns, the height of text.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// No field is left in the tool bars where an expression cannot be entered.
    #[test]
    fn the_tool_bars_have_no_number_only_fields() {
        let src = crate::gui::panels_source::PANELS;
        for fname in ["fn drawing_opts(", "fn describe(pc: &mut qymcad_ui_state::PartCtx"] {
            let a = src.find(fname).expect("the bar is there");
            // The bar's body ends at the NEXT function of any shape. Ending it only at `    pub(super) fn `
            // was right while every function in the file was a method; once helpers were lifted out to the
            // top level the body ran on past its own closing brace and counted their fields as its own.
            let rest = &src[a..];
            // `\npub fn ` is how a free function looks in a crate; without it the body ran past its own
            // end into the next one and counted its fields.
            let b = ["\n    pub(super) fn ", "\n    pub(crate) fn ", "\n    fn ", "\npub(crate) fn ", "\npub fn ", "\nfn "].iter().filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
            let n = rest[..b].matches("DragValue").count();
            assert_eq!(
                n, 0,
                "{n} DragValue fields are left in `{fname}` — neither a formula nor a global variable can be \
                 entered there, and these are the working dimensions of a part"
            );
        }
    }

    /// The dimension fields AFTER DRAWING accept an expression, not a bare number alone.
    ///
    /// Entering the width and height of a rectangle and an ellipse, the radius and angle of a polygon,
    /// the corner fillet and the rotation all existed, but was parsed through `parse::<f64>()` —
    /// neither a formula nor a global variable could be entered. These are the working dimensions of a
    /// part: "width = housing - 2*wall" is an everyday thing.
    #[test]
    fn sketch_size_fields_accept_expressions() {
        let mut app = App::default();
        app.project.parameters.push(qymcad_core::model::Param { name: "korpus".into(), expr: "60".into(), value: 60.0 });
        app.project.parameters.push(qymcad_core::model::Param { name: "stenka".into(), expr: "2.5".into(), value: 2.5 });

        assert_eq!(crate::gui::sketching::parse_num(&app.project, "40"), Some(40.0), "a plain number must keep working as before");
        assert_eq!(crate::gui::sketching::parse_num(&app.project, " 40,5 "), Some(40.5), "a comma as the decimal separator");
        assert_eq!(crate::gui::sketching::parse_num(&app.project, "korpus - 2*stenka"), Some(55.0), "an expression over global variables");
        assert_eq!(crate::gui::sketching::parse_num(&app.project, ""), None, "empty: do not change the value");
        assert_eq!(crate::gui::sketching::parse_num(&app.project, "1/0"), None, "a broken expression: do not change the value rather than substitute infinity");
        assert_eq!(crate::gui::sketching::parse_num(&app.project, "no_such_name"), None, "an unknown name: do not change the value");
    }

    /// Exactly one bare-number parse is left in the sketcher — and it is legitimate.
    ///
    /// It is the value field of a DIMENSION: there `parse::<f64>()` asks "is this a number or an
    /// expression" in order to decide whether to store the formula in the constraint. Every other
    /// field must go through `parse_num`. Comments do not count — the code is what is counted.
    #[test]
    fn the_sketcher_parses_values_through_one_door() {
        let src = crate::gui::sketch_source::SKETCH;
        let n = src.lines().filter(|l| !l.trim_start().starts_with("//")).filter(|l| l.contains("parse::<f64>()")).count();
        assert_eq!(
            n, 1,
            "parsing a value must go through `parse_num` everywhere except the dimension field (where it is \
             decided whether it is a number or a formula). Places with a bare parse: {n}"
        );
    }

    /// A PLATE'S SKETCH WITH THE EXTRUSION IN HAND, the hand at the window: its box of fields stands by the geometry.
    fn extrusion_in_hand(app: &mut App) -> super::super::hand::Hand<'_> {
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = super::super::Sel::Sketch(si);
        let mut hand = super::super::hand::Hand::new(app);
        hand.tool(qymcad_ui_state::PartTool::Extrude);
        hand.frame(Vec::new());
        hand.frame(Vec::new());
        hand
    }

    /// Where the box of fields of the command in hand stands: the one beside the arrow or the one of the rest.
    fn the_box(hand: &super::super::hand::Hand) -> egui::Rect {
        let id = hand.tool_box_id().expect("the box of fields of the extrusion is drawn");
        let at_arrow = (0..4).find_map(|i| hand.area_rect(id.with(("at", i))));
        at_arrow.or_else(|| hand.area_rect(id)).expect("the box of fields of the extrusion is drawn")
    }

    /// A window opened from the menu: the heading of the menu, the item in it, the id of the window.
    struct Opened {
        menu: &'static str,
        item: &'static str,
        id: &'static str,
    }

    #[test]
    fn a_window_over_the_box_at_the_geometry_covers_it() {
        let tr = crate::i18n::tr;
        let mut app = App::default();
        let mut hand = extrusion_in_hand(&mut app);
        let origin = egui::pos2(0.0, 0.0);
        let windows = [
            Opened { menu: "menu-windows", item: "win-settings", id: "win_settings" },
            Opened { menu: "menu-windows", item: "win-parts-library", id: "win_parts_library" },
            Opened { menu: "menu-help", item: "help-title", id: "win_help" },
        ];
        let mut under = Vec::new();
        for Opened { menu, item, id } in windows {
            hand.press_word(&tr(menu), origin);
            // the item under the heading of the menu: "Help" names both
            let heading = hand.written_at(&tr(menu)).map_or(origin, |r| r.center());
            hand.press_word(&tr(item), heading + egui::vec2(20.0, 40.0));
            hand.frame(Vec::new());
            let window = hand.area_rect(egui::Id::new(id)).unwrap_or_else(|| panic!("{item}: the window did not open"));
            let target = the_box(&hand).center();
            // the window carried by its title bar, its middle onto the box
            let title = egui::pos2(window.center().x, window.min.y + 10.0);
            hand.drag_screen(title, title + (target - window.center()));
            hand.frame(Vec::new());
            let top = hand.top_layer_at(target);
            if top.map(|l| l.id) != Some(egui::Id::new(id)) {
                under.push(format!("{item}: on top at the box is {top:?}"));
            }
        }
        assert!(under.is_empty(), "the box of fields at the geometry stands over a window:\n{}", under.join("\n"));
    }

    /// THE NAMES OF THE FIELDS IN A BOX AT THE GEOMETRY ARE NOT TEXT TO SELECT: a drag across the name of a field of the
    /// extrusion in hand leaves nothing selected.
    #[test]
    fn the_names_in_a_box_at_the_geometry_are_not_selected() {
        let mut app = App::default();
        let mut hand = extrusion_in_hand(&mut app);
        let the_box = the_box(&hand);
        let name = hand
            .words_drawn()
            .iter()
            .find(|(t, r)| the_box.contains_rect(*r) && t.chars().any(char::is_alphabetic) && t.parse::<f64>().is_err())
            .map(|(_, r)| *r)
            .expect("a name of a field stands in the box");
        hand.drag_screen(name.left_center() + egui::vec2(1.0, 0.0), name.right_center() - egui::vec2(1.0, 0.0));
        hand.frame(Vec::new());
        assert!(!hand.text_selected(), "a drag across the name of a field at {name:?} selected its text");
    }

    /// NO BOX OF FIELDS OF A WORKBENCH IS LAID OVER THE WINDOWS: each takes the layer `AT_GEOMETRY`, the layer of the
    /// windows; one written on the layer above them covers any window dragged over the view.
    #[test]
    fn no_box_of_a_workbench_is_laid_over_the_windows() {
        let root = qymcad_i18n::ratchet::crates_root();
        let mut over = Vec::new();
        for krate in ["qymcad-part", "qymcad-sketch", "qymcad-assembly"] {
            let mut stack = vec![root.join(krate).join("src")];
            while let Some(dir) = stack.pop() {
                for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().is_some_and(|x| x == "rs") {
                        let text = std::fs::read_to_string(&p).unwrap_or_default();
                        over.extend(text.lines().enumerate().filter(|(_, l)| l.contains("Order::Foreground")).map(|(i, _)| format!("{}:{}", p.display(), i + 1)));
                    }
                }
            }
        }
        assert!(over.is_empty(), "a box of fields is laid over the windows:\n{}", over.join("\n"));
    }
}
