//! THE VIEW CUBE — the whole path a person walks.
//!
//! The main thing the cube lacked: a click on a CORNER (the isometric view) and on an EDGE (a view at 45
//! degrees). It is for their sake that the cube is truncated — a chamfer is a zone, not decoration. What
//! is checked here is that the zones exist, that they are hit where they should be and that they give a
//! correct camera.
#[cfg(test)]
mod tests {
    use super::super::viewcube::{dir_to_angles, zones, ZoneKind};
    use super::super::App;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    fn ready() -> App {
        let mut app = App::default();
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        app
    }

    /// 26 ZONES: 6 faces plus 12 edges plus 8 corners. Exactly what the former cube lacked.
    #[test]
    fn the_cube_has_all_twenty_six_zones() {
        let z = zones();
        assert_eq!(z.len(), 26, "the cube must give 26 zones, and it gives {}", z.len());
        assert_eq!(z.iter().filter(|x| x.kind == ZoneKind::Face).count(), 6, "there are six faces");
        assert_eq!(z.iter().filter(|x| x.kind == ZoneKind::Edge).count(), 12, "there are twelve edges");
        assert_eq!(z.iter().filter(|x| x.kind == ZoneKind::Corner).count(), 8, "there are eight corners");
        // the faces have captions and the edges and corners do not: captioning 26 zones means burying the
        // cube in text
        assert_eq!(z.iter().filter(|x| x.label.is_some()).count(), 6, "exactly the faces are captioned");
    }

    /// A CORNER GIVES THE ISOMETRIC VIEW: all three axes are visible and none is degenerate.
    ///
    /// This is the commonest click on the cube, and it used not to exist.
    #[test]
    fn a_corner_gives_a_proper_isometric_view() {
        for z in zones().iter().filter(|z| z.kind == ZoneKind::Corner) {
            let (yaw, pitch) = dir_to_angles(z.dir);
            let mut app = ready();
            app.cam.yaw = yaw;
            app.cam.pitch = pitch;
            let (right, up, fwd) = app.cam.basis();
            // the basis is orthonormal — the camera is not degenerate
            for (n, v) in [("right", right), ("up", up), ("fwd", fwd)] {
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                assert!((l - 1.0).abs() < 1e-9, "{n} must be a unit vector, and it is {l}");
            }
            // ALL THREE AXES ARE VISIBLE: none looks exactly along the view (otherwise it is not isometric)
            for (i, ax) in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].iter().enumerate() {
                let along = (ax[0] * fwd[0] + ax[1] * fwd[1] + ax[2] * fwd[2]).abs();
                assert!(along < 0.95, "axis {i} is degenerate in the view from a corner: the absolute dot product is {along}");
            }
        }
    }

    /// THE POLES DO NOT BREAK THE CAMERA. The top view gives a pitch of 90 degrees, and a naive basis
    /// degenerates there.
    ///
    /// The degeneracy is already handled in `Cam::basis` (the reference "up" switches to world Y). The
    /// test pins that down: an edit to the basis must not break the poles silently.
    #[test]
    fn the_poles_keep_an_orthonormal_basis() {
        for z in zones().iter().filter(|z| z.kind == ZoneKind::Face && z.dir[2].abs() > 0.9) {
            let (yaw, pitch) = dir_to_angles(z.dir);
            let mut app = ready();
            app.cam.yaw = yaw;
            app.cam.pitch = pitch;
            let (right, up, fwd) = app.cam.basis();
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            assert!(dot(right, up).abs() < 1e-9 && dot(right, fwd).abs() < 1e-9 && dot(up, fwd).abs() < 1e-9, "the basis at a pole must be orthogonal");
            assert!((dot(right, right) - 1.0).abs() < 1e-9, "and normalised");
            assert!(fwd[2].abs() > 0.999, "the view at a pole runs along Z");
        }
    }

    /// A CLICK ON A ZONE TURNS THE VIEW TO THAT VERY ZONE — through a real pick rather than by assigning
    /// angles.
    #[test]
    fn clicking_a_zone_turns_the_view_to_it() {
        let mut app = ready();
        // the view starts isometric so that there are plenty of front zones
        app.cam.yaw = -0.7;
        app.cam.pitch = 0.6;
        let idx = app.viewcube_zone_at_pub(rect(), app.viewcube_zone_center_pub(rect(), 0)).expect("the centre of zone 0 must land inside it");
        let want = zones()[idx].dir;

        let at = app.viewcube_zone_center_pub(rect(), idx);
        assert!(app.viewcube_click_pub(rect(), at), "a click on the cube must be accepted");
        // the turn is animated — the animation is driven to its end
        app.finish_view_anim_pub();
        let (yw, pt) = dir_to_angles(want);
        assert!((app.cam.yaw - yw).abs() < 1e-6, "the yaw must arrive at the zone: {} against {yw}", app.cam.yaw);
        assert!((app.cam.pitch - pt).abs() < 1e-6, "the pitch must arrive at the zone: {} against {pt}", app.cam.pitch);
    }

    /// THE FAR SIDE IS NOT CLICKABLE: a click on a visible zone must not go to the one behind it.
    #[test]
    fn the_far_side_of_the_cube_is_not_clickable() {
        let mut app = ready();
        app.cam.yaw = -0.7;
        app.cam.pitch = 0.6;
        let (_, _, fwd) = app.cam.basis();
        for i in 0..zones().len() {
            let at = app.viewcube_zone_center_pub(rect(), i);
            if let Some(hit) = app.viewcube_zone_at_pub(rect(), at) {
                let d = zones()[hit].dir;
                let toward = -(d[0] * fwd[0] + d[1] * fwd[1] + d[2] * fwd[2]);
                assert!(toward > 0.0, "the pick returned a zone from the FAR side of the cube (toward={toward})");
            }
        }
    }

    /// A MISS OF THE CUBE does not swallow the click: it must reach the scene.
    #[test]
    fn a_click_away_from_the_cube_is_not_swallowed() {
        let mut app = ready();
        assert!(!app.viewcube_click_pub(rect(), egui::pos2(20.0, 650.0)), "a click in another corner of the screen does not belong to the cube");
    }

    /// THE CAPTIONS OF THE FACES COME FROM THE LOCALISATION: a build in one language with a cube in
    /// another is unfinished work.
    #[test]
    fn face_labels_come_from_the_catalogue() {
        crate::i18n::set_language("en");
        assert_eq!(crate::i18n::tr("view-front"), "FRONT");
        // THE OTHER LANGUAGE IS ASKED FOR A WORD OF ITS OWN rather than compared with a literal: a literal
        // would pin the check to one particular translation and would have to be edited with it.
        crate::i18n::set_language("ru");
        let other = crate::i18n::tr("view-front");
        assert_ne!(other, "FRONT", "the other language must carry a word of its own, not the English one");
        assert_ne!(other, "view-front", "and not the key itself");
        for z in zones().iter().filter(|z| z.kind == ZoneKind::Face) {
            let key = z.label.expect("a face must have a caption");
            assert_ne!(crate::i18n::tr(key), key, "the caption {key} must be translated");
        }
    }

    /// THE SIZE OF THE CUBE IS A SETTING OF THREE VALUES, and by default it is readable.
    ///
    /// The first edition was too big: on a 2K screen the middle one was reported as already large and the
    /// large one as taking a noticeable part of the viewport. All three were reduced by 30%. What is
    /// checked is not "bigger than before at any cost" but what matters: readable by default, three
    /// distinguishable sizes, and a large one that does not claim the viewport (checked by a test of its
    /// own).
    #[test]
    fn the_cube_size_is_a_setting_with_three_readable_steps() {
        let mut app = ready();
        app.set.viewcube_size = 1;
        let medium = app.viewcube_size_pub();
        assert!(medium > 36.0, "the middle cube must be more readable than the former 36 px, and it is {medium}");
        app.set.viewcube_size = 0;
        let small = app.viewcube_size_pub();
        app.set.viewcube_size = 2;
        let large = app.viewcube_size_pub();
        assert!(small < medium && medium < large, "the three sizes must differ: {small} / {medium} / {large}");
        assert!(large < medium * 1.6, "the spread of the sizes is sensible rather than twofold: {medium} -> {large}");
    }

    /// THE TURN IS ANIMATED rather than instant: a jump is disorienting on a complex assembly.
    #[test]
    fn the_turn_is_animated_not_instant() {
        let mut app = ready();
        app.cam.yaw = 0.0;
        app.cam.pitch = 0.0;
        app.animate_view_to(1.5, 0.5);
        assert!((app.cam.yaw - 0.0).abs() < 1e-9, "right after the request the view is NOT there yet — it is travelling");
        app.finish_view_anim_pub();
        assert!((app.cam.yaw - 1.5).abs() < 1e-6, "on completion it must arrive exactly");
    }

    /// THE SHORTEST WAY ROUND IN YAW: from -170 to +170 degrees that is 20 degrees, not 340 all the way
    /// around.
    #[test]
    fn the_turn_takes_the_short_way_round() {
        let mut app = ready();
        app.cam.yaw = -170f64.to_radians();
        app.cam.pitch = 0.0;
        app.animate_view_to(170f64.to_radians(), 0.0);
        let (from, to) = app.view_anim_endpoints_pub().expect("the animation is running");
        assert!((to.0 - from.0).abs() < std::f64::consts::PI, "the turn must take the short way round, and it takes {:.0} degrees", (to.0 - from.0).to_degrees().abs());
    }
    /// A CORNER IS A HEXAGON and not a triangle: otherwise HOLES are left at the corners.
    ///
    /// At a corner of a truncated cube three faces and three edges meet, and all six permutations of
    /// (H,T,S) lie next to each other. A triangle through three of them left three gaps — visible by eye
    /// at every corner.
    #[test]
    fn corners_are_hexagons_so_there_are_no_gaps() {
        for z in zones().iter().filter(|z| z.kind == ZoneKind::Corner) {
            assert_eq!(z.poly.len(), 6, "a corner must be a hexagon, and it has {} vertices", z.poly.len());
        }
    }

    /// A SURFACE WITH NO HOLES: every edge of a polygon belongs to EXACTLY TWO zones.
    ///
    /// This is the check for holes on the merits rather than by eye: on a closed surface every edge is
    /// shared by exactly two faces. An edge met once is an open boundary, that is, a hole.
    #[test]
    fn the_surface_is_closed_every_edge_shared_by_two_zones() {
        use std::collections::HashMap;
        let key = |p: &[f64; 3]| (((p[0] * 1000.0).round()) as i64, ((p[1] * 1000.0).round()) as i64, ((p[2] * 1000.0).round()) as i64);
        let mut count: HashMap<((i64, i64, i64), (i64, i64, i64)), usize> = HashMap::new();
        for z in zones() {
            for i in 0..z.poly.len() {
                let (a, b) = (key(&z.poly[i]), key(&z.poly[(i + 1) % z.poly.len()]));
                let e = if a <= b { (a, b) } else { (b, a) };
                *count.entry(e).or_insert(0) += 1;
            }
        }
        let open: Vec<_> = count.iter().filter(|(_, n)| **n != 2).collect();
        assert!(open.is_empty(), "the surface of the cube has holes: edges met other than twice: {open:?}");
    }

    /// THE VERTICES OF A CORNER ARE ORDERED AROUND IT: otherwise the hexagon is drawn as a star and
    /// picked wrongly.
    #[test]
    fn corner_vertices_are_ordered_around_the_normal() {
        for z in zones().iter().filter(|z| z.kind == ZoneKind::Corner) {
            // the sum of the turns while walking a convex polygon is one full circle
            let c = z.poly.iter().fold([0.0f64; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
            let k = z.poly.len() as f64;
            let c = [c[0] / k, c[1] / k, c[2] / k];
            let mut prev: Option<f64> = None;
            let mut total = 0.0;
            let seed = if z.dir[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
            let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let nrm = |a: [f64; 3]| {
                let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
                [a[0] / l, a[1] / l, a[2] / l]
            };
            let u = nrm(cross(z.dir, seed));
            let v = cross(z.dir, u);
            for p in &z.poly {
                let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
                let ang = (d[0] * v[0] + d[1] * v[1] + d[2] * v[2]).atan2(d[0] * u[0] + d[1] * u[1] + d[2] * u[2]);
                if let Some(pv) = prev {
                    let mut step = ang - pv;
                    if step < 0.0 {
                        step += std::f64::consts::TAU;
                    }
                    total += step;
                }
                prev = Some(ang);
            }
            assert!(total < std::f64::consts::TAU, "the vertices of a corner must run around without doubling back, and the sum of the steps is {total:.2}");
        }
    }

    /// THE CAPTION LIES ON THE FACE LIKE A DECAL — it turns and is distorted together with it.
    ///
    /// Three approaches before it would not do, and the report had to be repeated three times. Horizontal
    /// text in the centre: the text and the faces live separately. Text rotated by the angle of the face:
    /// it turns but is not distorted, so the face is squeezed into a strip while the letters keep their
    /// normal width. A visibility threshold: it simply vanished. The right answer: BAKE IT INTO A TEXTURE
    /// and stretch it over a quadrilateral in the plane of the face — then the caption is deformed exactly
    /// as the face is, because it is a drawing on it.
    #[test]
    fn the_label_lies_on_the_face_like_a_decal() {
        let src = include_str!("viewcube.rs");
        let a = src.find("if let (Some(key), true) = (z.label").expect("the caption is drawn");
        let b = src[a..].find("self.draw_axis_triad").map(|i| a + i).unwrap_or(src.len());
        let blk = &src[a..b];
        assert!(!blk.contains("toward > 0.55"), "there must be no threshold at which the caption disappears");
        assert!(!blk.contains("painter.text("), "the caption is NOT drawn as screen text — it is a texture on the face");
        assert!(blk.contains("Mesh::with_texture"), "the caption must be a texture stretched over the face");
        assert!(blk.contains("label_frame(z.dir)"), "the quadrilateral of the caption must lie IN THE PLANE of the face");
    }

    /// THE QUADRILATERAL OF THE CAPTION IS DEFORMED TOGETHER WITH THE FACE — that is what "lies on it"
    /// means.
    ///
    /// The check goes by the geometry rather than the picture: the camera is turned so that the face
    /// stands at a grazing angle, and the quadrilateral of the caption must be squeezed just as the face
    /// is. Screen text would not be squeezed at all in this test.
    #[test]
    fn the_label_quad_squashes_together_with_its_face() {
        let mut app = ready();
        // the +X face, looked at head on
        app.cam.yaw = 0.0;
        app.cam.pitch = 0.0;
        let face = zones().iter().position(|z| z.kind == ZoneKind::Face && z.dir[0] > 0.9).expect("the +X face");
        let wide = app.label_quad_width_pub(rect(), face);
        let face_wide = app.zone_screen_width_pub(rect(), face);

        // and now at a grazing angle
        app.cam.yaw = 1.2;
        let narrow = app.label_quad_width_pub(rect(), face);
        let face_narrow = app.zone_screen_width_pub(rect(), face);

        assert!(narrow < wide * 0.8, "the caption must be squeezed together with the face: {wide:.1} -> {narrow:.1}");
        let k_label = narrow / wide;
        let k_face = face_narrow / face_wide;
        assert!(
            (k_label - k_face).abs() < 0.12,
            "the caption must be squeezed TO THE SAME degree as the face: the face {k_face:.2}, the caption {k_label:.2}"
        );
    }

    /// A SMALL CUBE HAS NO CAPTIONS: 32 px per face give letters of 6 px — mush instead of text.
    ///
    /// The small size is chosen precisely so that the cube does not get in the way; one then navigates by
    /// its shape and by the triad of axes.
    #[test]
    fn the_small_cube_has_no_labels() {
        let src = include_str!("viewcube.rs");
        assert!(
            src.contains("self.set.viewcube_size > 0"),
            "the captions must be shown only at the middle and large sizes"
        );
    }

    /// THE BUILD SHIPS A REAL BOLD FONT. Faking boldness by drawing repeatedly cost five calls per
    /// caption and turned the letters into mush at a small size.
    #[test]
    fn the_build_ships_a_real_bold_font() {
        let gui = include_str!("../gui.rs");
        assert!(gui.contains("LiberationSans-Bold.ttf"), "the bold font must be baked into the binary");
        assert!(gui.contains("egui::FontFamily::Name(BOLD_FONT"), "and be registered as a family of its own");
        // the licence must lie beside it: the OFL requires distributing it together with the font
        assert!(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/LiberationSans-LICENSE.txt")).exists(), "the licence of the font must lie beside the font");
    }

    /// THE CUBE DOES NOT TAKE UP HALF THE VIEWPORT: on a 2K screen the large size was too big, and all
    /// three were reduced.
    #[test]
    fn the_cube_stays_a_pointer_not_a_centrepiece() {
        let mut app = ready();
        app.set.viewcube_size = 2;
        let large = app.viewcube_size_pub();
        // 700 px is the height of the viewport in the tests; the cube (two half-sizes) must take up
        // noticeably less than a fifth of it
        assert!(large * 2.0 < 700.0 * 0.22, "a large cube must stay a pointer, and it is {} px", large * 2.0);
        app.set.viewcube_size = 1;
        assert!(app.viewcube_size_pub() < large * 0.8, "the middle one is noticeably smaller than the large one");
    }

    /// THE OLD PLATE OF VIEW BUTTONS IS GONE — the cube does its work, and does it more fully.
    ///
    /// THE CHECK GOES BY THE MECHANISM, NOT BY A FORMER CAPTION. It used to look for the exact literals
    /// the plate was written with, and a guard like that stops meaning anything the moment somebody writes
    /// the plate back with different words. Turning the view is `animate_view_to`, and the only place that
    /// may call it is the cube.
    #[test]
    fn the_old_view_buttons_are_gone() {
        let panels = crate::gui::panels_source::PANELS;
        assert!(
            !panels.contains("animate_view_to("),
            "a panel turns the view itself again — that work belongs to the cube, and two ways of doing it will diverge"
        );
    }

    /// THE CUBE IS REALLY DRAWN — not "the geometry adds up" but a frame that builds without a panic.
    ///
    /// This test was written AFTER A CRASH. The earlier checks looked at the geometry and at the source,
    /// and NOT ONE of them performed the drawing — and the application crashed on the very first frame: on
    /// a mesh with a texture the vertices cannot be pushed through `colored_vertex`, which checks that
    /// with an assertion. The tests were green and the program would not start.
    ///
    /// The rule that follows: a widget that draws something must have a test that DRAWS it. A check of the
    /// shape and a check of the source are no substitute.
    #[test]
    fn the_cube_paints_a_frame_without_panicking() {
        for size in [0u8, 1, 2] {
            let mut app = ready();
            app.set.viewcube_size = size;
            app.cam.yaw = -0.7;
            app.cam.pitch = 0.6;
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx); // the same set of fonts as in a real window
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let painter = ui.painter().clone();
                    // the same call a real frame makes
                    app.draw_viewcube_pub(&painter, rect());
                });
            });
        }
    }

    /// AND FROM DIFFERENT ANGLES: the poles and grazing viewpoints are branches of their own (degenerate
    /// axes, inverted polygons), and each can crash in its own way.
    #[test]
    fn the_cube_paints_from_every_angle() {
        let angles = [(0.0, 0.0), (0.0, std::f64::consts::FRAC_PI_2), (0.0, -std::f64::consts::FRAC_PI_2), (2.4, 0.9), (-1.9, -0.7), (3.1, 0.0)];
        for (yaw, pitch) in angles {
            let mut app = ready();
            app.cam.yaw = yaw;
            app.cam.pitch = pitch;
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx); // the same set of fonts as in a real window
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let painter = ui.painter().clone();
                    app.draw_viewcube_pub(&painter, rect());
                });
            });
        }
    }

    /// THE CAPTION READS ON EVERY FACE: horizontally and not upside down.
    ///
    /// The test was written from a report: screenshots arrived where the left caption was upside down
    /// while the front and back ones stood vertically. The cause: the axes of a face were derived by the
    /// formula "the next ones in a cycle" — tidy to look at and wrong on half the faces. There are exactly
    /// six right answers, and each is a convention about how a person looks at that face.
    ///
    /// The check goes by THE GEOMETRY rather than the picture: the face is looked at head on and "right"
    /// of the caption is required to run right across the screen and "up" to run up. The formula failed
    /// that on three faces out of six.
    #[test]
    fn every_face_label_reads_horizontally_and_upright() {
        for (i, z) in zones().iter().enumerate() {
            if z.kind != ZoneKind::Face {
                continue;
            }
            let mut app = ready();
            // this face is looked at HEAD ON — just as after a click on it
            let (yaw, pitch) = dir_to_angles(z.dir);
            app.cam.yaw = yaw;
            app.cam.pitch = pitch;
            let (right, up) = app.label_screen_dirs_pub(rect(), i);
            let name = z.label.unwrap_or("?");

            // HORIZONTAL: "right" of the text runs along the screen X rather than up and down
            assert!(
                right.x.abs() > right.y.abs() * 3.0,
                "{name}: the caption stood up VERTICALLY — \"right\" of the text went to {right:?}"
            );
            // NOT MIRRORED: "right" points right indeed (the screen X grows)
            assert!(right.x > 0.0, "{name}: the caption is mirrored — \"right\" of the text went left ({right:?})");
            // NOT UPSIDE DOWN: "up" of the text runs up (the screen Y decreases)
            assert!(up.y < 0.0, "{name}: the caption is UPSIDE DOWN — \"up\" of the text went down ({up:?})");
            assert!(up.y.abs() > up.x.abs() * 3.0, "{name}: \"up\" of the text is tipped sideways ({up:?})");
        }
    }

}
