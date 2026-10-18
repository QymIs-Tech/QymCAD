//! egui's red frame of a widget that changed its id, told by its colour: the one place of the window the checks drive
//! that names a colour, and it names egui's, not the colour of a message.

/// AND THE RED FRAME WITH NO WORDS: egui's mark of a widget whose rectangle took another id under the same parent
/// between two frames - a red stroke of 2 points over everything; where each stands
pub(super) fn red_frames(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::Rect> {
    let mut out = Vec::new();
    for shape in shapes {
        if let egui::epaint::Shape::Rect(r) = &shape.shape {
            if shape.clip_rect == egui::Rect::EVERYTHING && r.stroke.color == egui::Color32::RED && r.fill == egui::Color32::TRANSPARENT {
                out.push(r.rect);
            }
        }
    }
    out
}
