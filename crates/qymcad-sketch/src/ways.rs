//! THE WAYS OF A DRAWING TOOL: how many a shape has, the next one, the first one.
//!
//! The only place that ties a `SketchShape` to the field of `SketchToolPrefs` holding its way. A key pressed again
//! while its tool is in hand takes the next way here (Settings -> Keyboard), and a tool taken anew starts here at its
//! first way; a shape with one way answers `Cycled::Single` and the press puts the tool down as it always did.

use qymcad_ui_state::{ArcWay, CircleWay, PolygonWay, RectWay, SketchShape, SketchToolPrefs, Way};

/// WHAT A PRESS ASKING FOR THE NEXT WAY OF A SHAPE DID.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cycled {
    /// The next way is in force; after the last, the first.
    Next,
    /// The shape has one way: nothing changed.
    Single,
}

/// HOW MANY WAYS `shape` IS BUILT IN: the ways of its options bar, one for a shape that offers none.
pub fn ways(shape: SketchShape) -> usize {
    match shape {
        SketchShape::Circle => CircleWay::ALL.len(),
        SketchShape::Rect => RectWay::ALL.len(),
        SketchShape::Arc => ArcWay::ALL.len(),
        SketchShape::Polygon => PolygonWay::ALL.len(),
        SketchShape::Line | SketchShape::Point | SketchShape::Slot | SketchShape::Ellipse | SketchShape::Spline | SketchShape::Text => 1,
    }
}

/// THE WAY AFTER `way` on the bar; after the last, the first.
fn following<W: Way>(way: W) -> W {
    W::at((way.index() + 1) % W::WAYS.len())
}

/// TAKE THE NEXT WAY OF `shape`: after the last, the first. A shape with one way is left as it is.
pub fn next_way(prefs: &mut SketchToolPrefs, shape: SketchShape) -> Cycled {
    match shape {
        SketchShape::Circle => prefs.circ_mode = following(prefs.circ_mode),
        SketchShape::Rect => prefs.rect_mode = following(prefs.rect_mode),
        SketchShape::Arc => prefs.arc_mode = following(prefs.arc_mode),
        SketchShape::Polygon => prefs.poly_mode = following(prefs.poly_mode),
        SketchShape::Line | SketchShape::Point | SketchShape::Slot | SketchShape::Ellipse | SketchShape::Spline | SketchShape::Text => return Cycled::Single,
    }
    Cycled::Next
}

/// PUT `shape` BACK TO ITS FIRST WAY, the first of its bar.
pub fn first_way(prefs: &mut SketchToolPrefs, shape: SketchShape) {
    match shape {
        SketchShape::Circle => prefs.circ_mode = CircleWay::ALL[0],
        SketchShape::Rect => prefs.rect_mode = RectWay::ALL[0],
        SketchShape::Arc => prefs.arc_mode = ArcWay::ALL[0],
        SketchShape::Polygon => prefs.poly_mode = PolygonWay::ALL[0],
        SketchShape::Line | SketchShape::Point | SketchShape::Slot | SketchShape::Ellipse | SketchShape::Spline | SketchShape::Text => {}
    }
}

/// THE PLACE OF THE WAY OF `shape` IN FORCE on its bar; 0 for a shape with one way.
pub fn way_index(prefs: &SketchToolPrefs, shape: SketchShape) -> usize {
    match shape {
        SketchShape::Circle => prefs.circ_mode.index(),
        SketchShape::Rect => prefs.rect_mode.index(),
        SketchShape::Arc => prefs.arc_mode.index(),
        SketchShape::Polygon => prefs.poly_mode.index(),
        SketchShape::Line | SketchShape::Point | SketchShape::Slot | SketchShape::Ellipse | SketchShape::Spline | SketchShape::Text => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EVERY SHAPE, CYCLED AS MANY TIMES AS IT HAS WAYS, COMES BACK TO ITS FIRST, passing every way once on the way in
    /// the order of the bar. A shape with one way answers `Single` and is left as it was.
    #[test]
    fn cycling_every_shape_comes_back_to_its_first_way() {
        let mut wrong = Vec::new();
        for shape in SketchShape::ALL {
            let mut prefs = SketchToolPrefs::default();
            let n = ways(shape);
            let mut seen = vec![way_index(&prefs, shape)];
            for _ in 0..n {
                let said = next_way(&mut prefs, shape);
                let want = if n > 1 { Cycled::Next } else { Cycled::Single };
                if said != want {
                    wrong.push(format!("{shape:?}: answered {said:?}, want {want:?}"));
                }
                seen.push(way_index(&prefs, shape));
            }
            let order: Vec<usize> = (0..n).chain([0]).collect();
            if seen != order {
                wrong.push(format!("{shape:?}: the ways went {seen:?}, want {order:?}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// THE FIRST WAY IS THE FIRST OF THE BAR, from any way in force.
    #[test]
    fn the_first_way_is_the_first_of_the_bar() {
        let mut wrong = Vec::new();
        for shape in SketchShape::ALL {
            let mut prefs = SketchToolPrefs::default();
            for _ in 0..ways(shape) {
                next_way(&mut prefs, shape);
                let mut back = prefs.clone();
                first_way(&mut back, shape);
                if way_index(&back, shape) != 0 {
                    wrong.push(format!("{shape:?}: from way {} the first way is {}", way_index(&prefs, shape), way_index(&back, shape)));
                }
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// THE SHAPES WITH WAYS ARE THE FOUR THE BAR OFFERS WAYS FOR, and no other.
    #[test]
    fn the_shapes_with_ways_are_the_circle_the_rectangle_the_arc_and_the_polygon() {
        let many: Vec<SketchShape> = SketchShape::ALL.into_iter().filter(|s| ways(*s) > 1).collect();
        assert_eq!(many, [SketchShape::Rect, SketchShape::Circle, SketchShape::Arc, SketchShape::Polygon]);
    }
}
