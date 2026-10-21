//! A PNG OUT OF A PICTURE the program drew: the thumbnail kept in a `.qpart`, the pictures the checks look at.

/// A PNG out of a `ColorImage` (RGBA8) — for the `thumb.png` inside a `.qpart` and a `.qcad`. Uses `image` with its png feature.
///
/// A PNG holds STRAIGHT alpha while a `Color32` holds premultiplied: the channels are unmultiplied on the way out,
/// or a pixel at half opacity reads back at half its brightness and the antialiased rim of the badge on a
/// transparent preview is a dark fringe.
pub(crate) fn color_image_to_png(img: &egui::ColorImage) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let (w, h) = (img.size[0] as u32, img.size[1] as u32);
    let mut rgba: Vec<u8> = Vec::with_capacity(img.pixels.len() * 4);
    for p in &img.pixels {
        rgba.extend_from_slice(&p.to_srgba_unmultiplied());
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(&rgba, w, h, image::ExtendedColorType::Rgba8).ok()?;
    Some(png)
}
