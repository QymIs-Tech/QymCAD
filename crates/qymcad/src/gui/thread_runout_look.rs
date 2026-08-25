//! LOOKING AT A THREAD'S RUN-OUT WITH ONE'S EYES.
//!
//! Reported behaviour: the run-out at the entry and the exit spoils the turn by thickening it, and the
//! thread will not screw on.
//!
//! A number does not settle this. The volume a pair of parts shares says whether they bind, not WHERE or
//! WHY; and a thickened turn is invisible from outside — the crests look the same, the fault sits in the
//! section. So the frame is drawn and looked at: the whole shaft, and the same shaft cut along its axis.
//!
//! The pictures land in `target/thread-look/` and are named after what they show. This is not an automatic
//! check and does not pretend to be one — it is a way of putting the geometry in front of an eye.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::apply12;
    use qymcad_core::model::Project;
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 900.0))
    }

    /// M`d` with pitch `p`, external or internal.
    fn m(d: f64, pitch: f64, internal: bool, fit: f64) -> ThreadSpec {
        ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch, internal, fit, ..Default::default() }
    }

    /// The round edge of radius about `r` — what a person clicks to place a thread.
    ///
    /// THE HIGHEST one of that radius. A boss standing on a flange has two, and taking whichever came first
    /// put the thread's start inside the flange: it then ran from the bottom upwards through solid metal, and
    /// the picture showed something nobody would ever model. A person clicks the rim they can see, which is
    /// the top one.
    fn rim(app: &mut App, body: qymcad_core::model::Id, r: f64) -> u32 {
        app.rebuild_if_dirty_for_test();
        let e = app.project.regen_edges.get(&body).cloned().unwrap_or_default();
        e.iter()
            .filter(|e| e.radius > 1e-9 && (e.radius - r).abs() < 0.05)
            .max_by(|x, y| x.mid[2].total_cmp(&y.mid[2]))
            .map(|e| e.id)
            .unwrap_or_else(|| panic!("no round edge of radius {r}: {:?}", e.iter().map(|x| x.radius).collect::<Vec<_>>()))
    }

    /// A threaded shaft in a fresh document.
    fn threaded_shaft(d: f64, pitch: f64, len: f64, lead: f64) -> App {
        let mut app = App::default();
        app.project = Project::default();
        app.project.new_document();
        let blank = app.project.add_cylinder(d * 0.5, len);
        let e = rim(&mut app, blank, d * 0.5);
        let t = app.project.add_thread(blank, e, m(d, pitch, false, 0.2), len, lead, lead);
        app.project.finish_base_body(t, 1);
        app.mode_3d = true;
        app.rebuild_if_dirty_for_test();
        app
    }

    /// Fit the camera to everything in the document — the same arithmetic the viewport's "fit" does.
    fn fit(app: &mut App) {
        let basis = app.cam.basis();
        let (right, up) = (basis.0, basis.1);
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        let mut pts: Vec<[f64; 3]> = Vec::new();
        for (i, b) in app.project.bodies.iter().enumerate() {
            let Some(id) = app.project.mesh_id(i) else { continue };
            let wt = app.project.body_world_transform(id);
            for v in &b.mesh.verts {
                let q = apply12(&wt, [v.x as f64, v.y as f64, v.z as f64]);
                for k in 0..3 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
                pts.push(q);
            }
        }
        assert!(!pts.is_empty(), "there is nothing to look at: the document holds no mesh");
        let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let (mut sx, mut sy) = (0.0f64, 0.0f64);
        for q in &pts {
            let rel = [q[0] - mid[0], q[1] - mid[1], q[2] - mid[2]];
            sx = sx.max((rel[0] * right[0] + rel[1] * right[1] + rel[2] * right[2]).abs());
            sy = sy.max((rel[0] * up[0] + rel[1] * up[1] + rel[2] * up[2]).abs());
        }
        app.cam.target = mid;
        let r = rect();
        app.cam.scale = ((r.width() as f64 / 2.0 / sx.max(1e-6)).min(r.height() as f64 / 2.0 / sy.max(1e-6)) * 0.9) as f32;
        app.cam.init = true;
    }

    /// Draw the frame and put it on disk under `name`.
    fn shot(app: &mut App, name: &str) {
        let basis = app.cam.basis();
        let img = app.rasterize_3d(rect(), &basis, 1.0, 1.0).expect("the frame was not drawn");
        let png = App::color_image_to_png(&img).expect("PNG");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/thread-look");
        std::fs::create_dir_all(&dir).expect("the directory for the pictures");
        std::fs::write(dir.join(name), png).expect("writing the picture");
        eprintln!("LOOK {}", dir.join(name).display());
    }

    /// EXACTLY WHAT WAS REPORTED, with the numbers from the person's own screen.
    ///
    /// Reported: the run-out spoils the turn, thickening it and changing its angle, and the thread will not
    /// screw on. The picture shows turns that are flat wide plates instead of a profile with flanks.
    ///
    /// The numbers are theirs: Ø40, pitch 5, length 20, fit 0.2, run-out 2 at each end, a CUSTOM profile with
    /// an included angle of 80 degrees and a depth of 3.6, no rounding. Every measurement before this one was
    /// made on a standard metric M10 or M20 — that is why none of them reproduced anything.
    #[test]
    #[ignore]
    fn look_at_the_reported_thread() {
        let spec = ThreadSpec {
            standard: ThreadStandard::Custom,
            nominal_d: 40.0,
            pitch: 5.0,
            fit: 0.2,
            custom_angle: 80.0,
            custom_depth: 3.6,
            crest_r: Some(0.0),
            root_r: Some(0.0),
            ..Default::default()
        };
        let g = spec.geometry();
        eprintln!("major {:.3}, pitch_d {:.3}, minor {:.3}, depth {:.3}, angle {:.1}, pitch {:.3}", g.major_d, g.pitch_d, g.minor_d, g.depth, g.angle_deg, g.pitch);
        eprintln!("the groove has {} edges", g.groove.len());
        for (i, e) in g.groove.iter().enumerate() {
            eprintln!("  {i}: {e:?}");
        }
        // THE GROOVE'S WIDTH AT THE CREST against the pitch. A groove wider than the pitch cannot fit beside
        // its neighbour: the two overlap and there is no land left between the turns.
        let width = 2.0 * g.depth * (g.angle_deg * 0.5).to_radians().tan();
        eprintln!("the groove is about {width:.2} mm wide at the crest, the pitch is {:.2}", g.pitch);

        let mut app = App::default();
        app.project = qymcad_core::model::Project::default();
        app.project.new_document();
        let blank = app.project.add_cylinder(20.0, 20.0);
        let e = rim(&mut app, blank, 20.0);
        let t = app.project.add_thread(blank, e, spec, 20.0, 2.0, 2.0);
        app.project.finish_base_body(t, 1);
        app.mode_3d = true;
        app.rebuild_if_dirty_for_test();

        app.cam.yaw = 0.0;
        app.cam.pitch = 0.0;
        fit(&mut app);
        shot(&mut app, "5-the-reported-thread.png");
        app.cam.target = [0.0, 0.0, 17.0];
        app.cam.scale *= 2.5;
        shot(&mut app, "6-the-reported-thread-entry.png");
    }

    /// THE REPORTED PART: a boss standing on a flange, threaded.
    ///
    /// Reported, looking at the picture: the exit of the thread at the bottom is broken off by a flat wall
    /// perpendicular to the profile, and the thread is unusable.
    ///
    /// This is the piece every earlier measurement was missing. A free-standing shaft has BOTH ends open, and
    /// the code says so itself: an open end carries the turn past the face, while an end that runs INTO the
    /// body needs a fading depth or a relief groove, or the last turn breaks off at a vertical wall. The
    /// bottom end here is blind — it runs into the flange — and that path was never looked at.
    #[test]
    #[ignore]
    fn look_at_a_boss_on_a_flange() {
        let spec = ThreadSpec {
            standard: ThreadStandard::Custom,
            nominal_d: 40.0,
            pitch: 5.0,
            fit: 0.2,
            // A PROFILE THAT FITS. The reported one (80 degrees, 3.6 deep at a 5 pitch) cannot be cut at all —
            // the flanks close to a point 2.9 mm down and the land between the turns comes to a tenth of a
            // millimetre, which is the plates that were reported. What is looked at here is whether the ENDS
            // are right once the profile itself is buildable.
            custom_angle: 60.0,
            custom_depth: 2.5,
            crest_r: Some(0.0),
            root_r: Some(0.0),
            ..Default::default()
        };
        let mut app = App::default();
        app.project = Project::default();
        app.project.new_document();
        // THE FLANGE AND THE BOSS: a wide low disc with a narrower cylinder standing on it, as on the picture.
        let flange = app.project.add_cylinder(35.0, 6.0);
        let boss = app.project.add_cylinder(20.0, 26.0);
        let blank = app.project.add_body_boolean(flange, boss, 1);
        let e = rim(&mut app, blank, 20.0);
        // The thread sits on the boss and STOPS above the flange: its lower end runs into the part.
        let t = app.project.add_thread(blank, e, spec, 20.0, 2.0, 2.0);
        app.project.finish_base_body(t, 1);
        app.mode_3d = true;
        app.rebuild_if_dirty_for_test();

        app.cam.yaw = 0.0;
        app.cam.pitch = 0.0;
        fit(&mut app);
        shot(&mut app, "7-boss-on-a-flange.png");
        // THE EXIT AT THE BOTTOM, close up: that is where the wall was reported.
        app.cam.target = [0.0, 0.0, 7.0];
        app.cam.scale *= 2.2;
        shot(&mut app, "8-boss-the-lower-exit.png");
    }

    /// THE SHAFT AS IT IS, AND THE SAME SHAFT CUT ALONG ITS AXIS.
    ///
    /// Ignored on purpose: this is not a verdict but a pair of eyes. It is run by hand when the run-out is
    /// being worked on.
    #[test]
    #[ignore]
    fn look_at_the_run_out() {
        let (d, pitch, len, lead) = (10.0, 1.5, 20.0, 1.5);
        let mut app = threaded_shaft(d, pitch, len, lead);

        // FROM THE SIDE: the axis lies across the frame, so both ends and their run-outs are in view.
        app.cam.yaw = 0.0;
        app.cam.pitch = 0.0;
        fit(&mut app);
        shot(&mut app, "1-shaft-from-the-side.png");

        // CUT ALONG THE AXIS. The plane passes through the axis, so the section shows the profile of every
        // turn - which is the only place a thickened turn can be seen at all.
        app.section.plane = Some(([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
        app.section.offset = 0.0;
        shot(&mut app, "2-shaft-cut-along-the-axis.png");

        // THE ENTRY CLOSE UP: the first turns, where a nut has to catch.
        app.cam.target = [0.0, 0.0, len - 2.0 * pitch];
        app.cam.scale *= 3.0;
        shot(&mut app, "3-the-entry-close-up.png");

        // AND THE FAR END, where the thread runs out into the shaft.
        app.cam.target = [0.0, 0.0, 2.0 * pitch];
        shot(&mut app, "4-the-far-end-close-up.png");
    }
}
