use qymcad_ui_state::SketchShape;

pub fn take_drawing_tool(sk: &mut qymcad_ui_state::SketchCtx, shape: SketchShape) {
    qymcad_ui_state::take_drawing_tool(&mut qymcad_ui_state::tools_in!(sk), Some(shape));
    if sk.set.again == qymcad_ui_state::Again::Cycles {
        if let Some(w) = crate::options::bar_ways(sk.tool_prefs, shape) {
            crate::options::opt_took(sk, qymcad_ui_state::Did::Chose { key: qymcad_ui_state::OptKey::of(w.key), index: 0 });
        }
    }
    *sk.mode_3d = false;
    *sk.status = crate::drawing_hint(sk.armed.drawing());
}

pub fn take_next_way(sk: &mut qymcad_ui_state::SketchCtx, shape: SketchShape) {
    let Some(w) = crate::options::bar_ways(sk.tool_prefs, shape) else { return crate::sketch_select_mode(sk) };
    let index = (w.now + 1) % w.words.len();
    crate::options::opt_took(sk, qymcad_ui_state::Did::Chose { key: qymcad_ui_state::OptKey::of(w.key), index });
    if let Some(said) = crate::options::way_said(sk.tool_prefs, shape) {
        *sk.status = qymcad_ui_state::Notice::info(said);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now(sk: &qymcad_ui_state::SketchCtx, shape: SketchShape) -> usize {
        crate::options::bar_ways(sk.tool_prefs, shape).map_or(0, |w| w.now)
    }

    #[test]
    fn the_key_pressed_again_walks_every_shape_through_its_ways_and_back() {
        let mut wrong = Vec::new();
        for shape in SketchShape::ALL {
            let mut bench = qymcad_ui_state::Bench::default();
            bench.set.again = qymcad_ui_state::Again::Cycles;
            let mut sk = bench.sketch_ctx();
            take_drawing_tool(&mut sk, shape);
            let Some(n) = crate::options::bar_ways(sk.tool_prefs, shape).map(|w| w.words.len()) else {
                take_next_way(&mut sk, shape);
                if !qymcad_ui_state::in_select_mode(sk.armed) {
                    wrong.push(format!("{shape:?}: a shape with one way was left in hand by its key pressed again: {:?}", sk.armed));
                }
                continue;
            };
            let mut seen = vec![now(&sk, shape)];
            for _ in 0..n {
                take_next_way(&mut sk, shape);
                seen.push(now(&sk, shape));
                let said = crate::options::way_said(sk.tool_prefs, shape).unwrap_or_default();
                if sk.armed.drawing() != Some(shape) || sk.status.words != said {
                    wrong.push(format!("{shape:?}: in hand {:?}, the status line says {:?}, want {said:?}", sk.armed.drawing(), sk.status.words));
                }
            }
            let order: Vec<usize> = (0..n).chain([0]).collect();
            if seen != order {
                wrong.push(format!("{shape:?}: the ways went {seen:?}, want {order:?}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn the_shapes_with_ways_are_the_circle_the_rectangle_the_arc_and_the_polygon() {
        let prefs = qymcad_ui_state::SketchToolPrefs::default();
        let many: Vec<SketchShape> = SketchShape::ALL.into_iter().filter(|s| crate::options::bar_ways(&prefs, *s).is_some_and(|w| w.words.len() > 1)).collect();
        let any: Vec<SketchShape> = SketchShape::ALL.into_iter().filter(|s| crate::options::bar_ways(&prefs, *s).is_some()).collect();
        assert_eq!(many, [SketchShape::Rect, SketchShape::Circle, SketchShape::Arc, SketchShape::Polygon]);
        assert_eq!(any, many, "a shape with one way has ways on its bar");
    }
}
