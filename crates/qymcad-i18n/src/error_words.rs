//! A KERNEL ERROR TURNED INTO WORDS.
//!
//! It sat in the dictionary crate and dragged the whole document crate in with it. The rule is the other
//! way round: the kernel gives a CODE, and whoever has a window picks the words. So the mapping lives
//! here, and the dictionary knows nothing of features, errors or bodies.
/// A KERNEL ERROR -> TEXT IN THE LANGUAGE OF THE PERSON.
///
/// The kernel names A FACT (`SplitPieceCount { got, want }`), and the words are chosen here. The numbers
/// are passed AS ARGUMENTS rather than glued into the string in advance: in different languages they land
/// in different places in the phrase.
pub fn error_text(e: &qymcad_core::errors::CoreError) -> String {
    use qymcad_core::errors::{CoreError as E, ExprError as X};
    let mut args = fluent_bundle::FluentArgs::new();
    match e {
        E::SplitPieceCount { got, want } => {
            args.set("got", *got as i64);
            args.set("want", *want as i64);
        }
        E::AugerOuterNotBigger { outer, shaft } => {
            args.set("outer", fmt1(*outer));
            args.set("shaft", fmt1(*shaft));
        }
        E::ThreadRemovedNothing { before, after } | E::AugerAddedNothing { before, after } => {
            args.set("before", fmt0(*before));
            args.set("after", fmt0(*after));
        }
        E::EdgesNotFound { asked } => args.set("asked", *asked as i64),
        E::ThreadPitchTooSmall { pitch } => args.set("pitch", fmt2(*pitch)),
        E::ThreadTooManyTurns { turns } => args.set("turns", fmt0(*turns)),
        E::ThreadDepthTooDeep { depth, radius, dia, pitch } => {
            args.set("depth", fmt2(*depth));
            args.set("radius", fmt2(*radius));
            args.set("dia", fmt1(*dia));
            args.set("pitch", fmt2(*pitch));
        }
        E::FilletRadiusTooBig { radius, issues, smooth_skipped } => {
            args.set("radius", fmt2(*radius));
            // THE BREAKDOWN BY EDGES is assembled HERE, out of translated pieces: each edge says whether
            // it takes any radius at all. That is the most useful part of the message — it answers "what
            // to do" rather than only "it did not work".
            let parts: Vec<String> = issues
                .iter()
                .map(|i| {
                    let mut a = fluent_bundle::FluentArgs::new();
                    a.set("edge", i.edge as i64);
                    match i.takes_up_to {
                        Some(m) => {
                            a.set("max", fmt2(m));
                            crate::tr_args("error-fillet-edge-takes-up-to", Some(&a))
                        }
                        None => crate::tr_args("error-fillet-edge-takes-none", Some(&a)),
                    }
                })
                .collect();
            args.set("issues", parts.join(", "));
            args.set(
                "smooth",
                if *smooth_skipped == 0 {
                    String::new()
                } else {
                    let mut a = fluent_bundle::FluentArgs::new();
                    a.set("n", *smooth_skipped as i64);
                    crate::tr_args("error-fillet-smooth-skipped", Some(&a))
                },
            );
        }
        E::FilletEdgesOneByOne { radius } => args.set("radius", fmt2(*radius)),
        E::ChamferTooBig { dist } => args.set("dist", fmt2(*dist)),
        E::SurfaceDoesNotClose { free } => args.set("n", *free as i64),
        E::OperationSplitBody { pieces } => args.set("n", *pieces as i64),
        E::ShellOfMultiShellBody { shells } => args.set("n", *shells as i64),
        E::ShellThicknessOverRound { thickness, limit } => {
            args.set("t", format!("{thickness:.2}"));
            args.set("r", format!("{limit:.2}"));
        }
        E::DraftFailed { angle } => args.set("angle", fmt2(*angle)),
        E::JointUnsatisfied { residual } => args.set("residual", fmt2(*residual)),
        E::CrossComponentInput { input } | E::SketchOnForeignFace { input } => args.set("input", *input as i64),
        E::SketchFaceRefLost { sketch, body } => {
            args.set("sketch", *sketch as i64);
            args.set("body", *body as i64);
        }
        E::RemoveFacesFailed { why } => args.set("why", why.clone()),
        // A MESSAGE FROM THE BRIDGE TO OCCT IS A CODE: the bridge has no language and hands back a
        // catalogue key. Foreign text (the error number of OCCT itself) passes through as it stands —
        // `name` translates known keys only.
        E::Kernel(msg) => args.set("message", crate::name(msg)),
        E::Expr(
            X::UnknownChar(w)
            | X::UnknownFn(w)
            | X::UnknownName(w)
            | X::NeedsOneArg(w)
            | X::NeedsTwoArgs(w)
            | X::UnexpectedToken(w)
            | X::TrailingInput(w),
        ) => args.set("what", w.clone()),
        _ => {}
    }
    crate::tr_args(&e.key(), Some(&args))
}



/// AN EXPRESSION ERROR -> TEXT IN THE LANGUAGE OF THE PERSON.
///
/// A door of its own, because it is called from places where the error is NOT a `CoreError`: the
/// parameter field, the dimension field, the gizmo field. Each of them used to get by on its own — and
/// all three differently: the parameters window printed "(!)" with no reason, the dimension popup showed
/// `Display`, that is, English text in a non-English interface, and the gizmo field said nothing at all.
///
/// The expression parser ALWAYS knew the reason (`ExprError` lists nine kinds, and all of them are
/// translated). It was lost at the last step — in the interface.
pub fn expr_error_text(e: &qymcad_core::errors::ExprError) -> String {
    error_text(&qymcad_core::errors::CoreError::Expr(e.clone()))
}

/// NUMBERS IN MESSAGES ARE FORMATTED HERE rather than handed to fluent: it has typography of its own
/// (digit separators by locale), and in a diameter like 12.5 it gives 12,5 in the middle of technical
/// text where a dot is expected.
fn fmt0(v: f64) -> String {
    format!("{v:.0}")
}

fn fmt1(v: f64) -> String {
    format!("{v:.1}")
}

fn fmt2(v: f64) -> String {
    format!("{v:.2}")
}
