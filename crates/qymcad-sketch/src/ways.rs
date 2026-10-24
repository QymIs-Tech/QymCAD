//! THE WAYS OF A DRAWING TOOL: a tool taken starts at its first way, its key pressed again takes the next.
//!
//! What ways a shape has, and which is in force, is read from its bar (`options::bar_ways`); a way is changed only
//! through the door a click on a word of the bar goes through (`options::opt_took`). The key and the bar can never
//! answer apart. A key pressed again while its tool is in hand takes the next way here (Settings -> Keyboard); a shape
//! with one way is put down, as the key always did.

use qymcad_ui_state::SketchShape;

/// TAKE DRAWING TOOL `shape` IN HAND, as its button, its key, its row of "More" and the search take it: every other tool
/// put down (`take_drawing_tool`), the flat view, and the hint of the tool in the status line. Under `Again::Cycles` the
/// tool starts at its first way, whatever way it was put down in; under `Again::PutsDown` it keeps the way left on its
/// bar.
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

/// THE KEY OF DRAWING TOOL `shape` PRESSED AGAIN under `Again::Cycles` (`BarAsk::NextWay`): the next word of the ways on
/// its bar picked, after the last the first, through the door a click on that word goes through (`opt_took`) - the
/// rectangle and the arc drop the points clicked, the circle drops them and the curves a tangent circle picked, the
/// polygon keeps its centre, which every way of it reads alike. The way taken is named in the status line. A shape with
/// one way is put down, as the key puts it down under `Again::PutsDown`.
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

    /// The place of the way of `shape` in force on its bar; 0 for a shape with one way.
    fn now(sk: &qymcad_ui_state::SketchCtx, shape: SketchShape) -> usize {
        crate::options::bar_ways(sk.tool_prefs, shape).map_or(0, |w| w.now)
    }

    /// EVERY SHAPE TAKEN, ITS KEY PRESSED AGAIN AS MANY TIMES AS IT HAS WAYS, COMES BACK TO ITS FIRST, passing every way
    /// once in the order of the bar, the tool in hand all along and the status line naming each way - through
    /// `take_next_way`, the path of the key. A shape with one way is put down by the first press.
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

    /// THE SHAPES WITH WAYS ARE THE FOUR THE BAR OFFERS WAYS FOR, each with more than one, and no other.
    #[test]
    fn the_shapes_with_ways_are_the_circle_the_rectangle_the_arc_and_the_polygon() {
        let prefs = qymcad_ui_state::SketchToolPrefs::default();
        let many: Vec<SketchShape> = SketchShape::ALL.into_iter().filter(|s| crate::options::bar_ways(&prefs, *s).is_some_and(|w| w.words.len() > 1)).collect();
        let any: Vec<SketchShape> = SketchShape::ALL.into_iter().filter(|s| crate::options::bar_ways(&prefs, *s).is_some()).collect();
        assert_eq!(many, [SketchShape::Rect, SketchShape::Circle, SketchShape::Arc, SketchShape::Polygon]);
        assert_eq!(any, many, "a shape with one way has ways on its bar");
    }
}
