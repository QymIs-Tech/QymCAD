//! THE LIGHTS OF THE 3D VIEW PART THE FACES of a part: by their angle to the key light, and, at one angle to it, by the
//! fill. One light taken by its absolute value gave a face and the face opposite it one tone, and every face at one
//! angle to the light one tone.

use qymcad_ui_state::{scene_lights, v_cross, v_dot, v_norm, Cam3, SceneLights};

/// The lights of the usual three-quarter view.
fn lights() -> SceneLights {
    scene_lights(&Cam3::default().basis())
}

/// A turn of a normal: towards which direction, by how many degrees.
struct Tilt {
    towards: [f64; 3],
    deg: f64,
}

/// A unit normal turned away from `from` by `tilt`.
fn tilted(from: [f64; 3], tilt: Tilt) -> [f64; 3] {
    let side = v_norm(v_cross(v_cross(from, tilt.towards), from));
    let (c, s) = (tilt.deg.to_radians().cos(), tilt.deg.to_radians().sin());
    v_norm(std::array::from_fn(|a| from[a] * c + side[a] * s))
}

#[test]
fn faces_at_0_30_60_and_90_deg_to_the_key_light_take_four_tones() {
    let l = lights();
    let eye = Cam3::default().basis().2.map(|v| -v);
    let tones: Vec<f64> = [0.0, 30.0, 60.0, 90.0].iter().map(|&deg| l.on(tilted(l.key, Tilt { towards: eye, deg: -deg }))).collect();
    for k in 1..tones.len() {
        assert!(tones[k - 1] - tones[k] > 0.05, "a face further from the key light is not darker: {tones:?}");
    }
}

#[test]
fn a_face_and_the_face_opposite_it_take_different_tones() {
    let l = lights();
    for n in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], v_norm([1.0, -1.0, 1.0])] {
        let back = n.map(|v: f64| -v);
        assert!((l.on(n) - l.on(back)).abs() > 0.05, "{n:?} and its opposite take {} and {}", l.on(n), l.on(back));
    }
}

/// Two faces at one angle to the key light, on either side of it: the fill comes from one side and parts them.
#[test]
fn faces_at_one_angle_to_the_key_light_are_parted_by_the_fill() {
    let l = lights();
    let (a, b) = (tilted(l.key, Tilt { towards: l.fill, deg: 40.0 }), tilted(l.key, Tilt { towards: l.fill, deg: -40.0 }));
    assert!((v_dot(a, l.key) - v_dot(b, l.key)).abs() < 1e-9, "setup: the two faces are not at one angle to the key");
    assert!((l.on(a) - l.on(b)).abs() > 0.05, "faces at one angle to the key light take {} and {}", l.on(a), l.on(b));
}

/// A face looking straight at the eye takes the whole of the light - the shading only darkens what turns away.
#[test]
fn a_face_looking_at_the_eye_takes_the_whole_light() {
    let basis = Cam3::default().basis();
    let at_eye = basis.2.map(|v| -v);
    assert!((scene_lights(&basis).on(at_eye) - 1.0).abs() < 1e-9);
}
