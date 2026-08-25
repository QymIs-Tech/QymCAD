//! Kernel errors as codes, not as phrases.
//!
//! The core is a library and has no language of its own. While it returned finished prose, two problems
//! followed at once:
//!
//! 1. **The interface could not be translated.** Half of what a person reads at the difficult moment — when a
//!    feature fails to build — came from the core and would have stayed in one language whatever the interface
//!    was set to.
//! 2. **The application told errors apart by substring.** A couple of places matched on a fragment of the
//!    message, which breaks as soon as the wording is edited, and breaks silently: the code compiles, the test
//!    is green, the behaviour is different.
//!
//! The core now names a fact (`CutPlaneDeleted`, `SplitPieceCount { got, want }`) and the application supplies
//! the words in the language of the user. The side benefit outweighs the localisation: errors became
//! programmatically distinguishable and enumerable, so they can be handled rather than guessed at.
//!
//! **Numbers stay inside the error code.** `SplitPieceCount { got: 3, want: 2 }` carries data rather than a
//! finished string: different languages put those numbers in different places in the sentence, and building
//! the text in advance would nail the language down again.
use serde::{Deserialize, Serialize};

/// A kernel operation, used by the "operation failed" and "a real kernel is required" errors.
///
/// A separate enumeration, because there are four dozen such errors and the operation is precisely what tells
/// them apart: a variant per error would mean forty near-identical names and forty near-identical
/// translations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    Extrude,
    ExtrudeProfile,
    ExtrudeContour,
    Revolve,
    RevolveProfile,
    RevolveAxis,
    Sweep,
    Loft,
    LoftBoolean,
    Boolean,
    BodyBoolean,
    Fillet,
    FilletVar,
    Chamfer,
    ChamferAsym,
    Shell,
    ShellCenter,
    Draft,
    PushFace,
    RemoveFaces,
    Thicken,
    /// A copy of faces as a standalone surface.
    CopyFaces,
    /// Replacing faces of a body with a surface.
    ReplaceFaces,
    /// Stitching sheets together.
    Stitch,
    /// Trimming a surface.
    Trim,
    /// A patch over a chain of edges.
    Patch,
    SplitBody,
    SplitFaces,
    Hole,
    Holes,
    Thread,
    Helix,
    Auger,
    Mirror,
    MirrorPlane,
    Array,
    Move,
    Transform,
    Cylinder,
    Sphere,
    Cone,
    Torus,
    Prism,
    FuseProfiles,
    Place,
}

impl Op {
    /// The localisation key of an operation; the application uses it to fetch both the name and the hint on
    /// why it might have failed. The key is derived from the name of the variant, so a new operation cannot be
    /// added and then forgotten: a test catches keys without strings.
    pub fn key(self) -> &'static str {
        use Op::*;
        match self {
            Extrude => "extrude",
            ExtrudeProfile => "extrude-profile",
            ExtrudeContour => "extrude-contour",
            Revolve => "revolve",
            RevolveProfile => "revolve-profile",
            RevolveAxis => "revolve-axis",
            Sweep => "sweep",
            Loft => "loft",
            LoftBoolean => "loft-boolean",
            Boolean => "boolean",
            BodyBoolean => "body-boolean",
            Fillet => "fillet",
            FilletVar => "fillet-var",
            Chamfer => "chamfer",
            ChamferAsym => "chamfer-asym",
            Shell => "shell",
            ShellCenter => "shell-center",
            Draft => "draft",
            PushFace => "push-face",
            RemoveFaces => "remove-faces",
            Thicken => "thicken",
            CopyFaces => "copy-faces",
            ReplaceFaces => "replace-faces",
            Stitch => "stitch",
            Trim => "trim",
            Patch => "patch",
            SplitBody => "split-body",
            SplitFaces => "split-faces",
            Hole => "hole",
            Holes => "holes",
            Thread => "thread",
            Helix => "helix",
            Auger => "auger",
            Mirror => "mirror",
            MirrorPlane => "mirror-plane",
            Array => "array",
            Move => "move",
            Transform => "transform",
            Cylinder => "cylinder",
            Sphere => "sphere",
            Cone => "cone",
            Torus => "torus",
            Prism => "prism",
            FuseProfiles => "fuse-profiles",
            Place => "place",
        }
    }

    /// Every operation, for the test that checks the translations are complete.
    pub fn all() -> &'static [Op] {
        use Op::*;
        &[
            Extrude, ExtrudeProfile, ExtrudeContour, Revolve, RevolveProfile, RevolveAxis, Sweep, Loft, LoftBoolean, Boolean, BodyBoolean,
            Fillet, FilletVar, Chamfer, ChamferAsym, Shell, ShellCenter, Draft, PushFace, RemoveFaces, Thicken, SplitBody, SplitFaces, Hole,
            Holes, Thread, Helix, Auger, Mirror, MirrorPlane, Array, Move, Transform, Cylinder, Sphere, Cone, Torus, Prism, FuseProfiles, Place,
        ]
    }
}

/// Why an edge would not take a fillet, reported per edge.
///
/// Per-edge diagnostics are the most valuable part of a fillet message: instead of "it failed" they say "edge
/// 6101 takes no radius at all, drop it; edge 6102 takes up to 1.50". Reducing that to "the operation failed"
/// would take away the only hint about what to do next. The data therefore travels inside the error code and
/// the application supplies the words.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilletEdgeIssue {
    /// the persistent name of the edge
    pub edge: u32,
    /// the largest radius this edge does accept; `None` means it accepts none at all, because its ends run
    /// into the tangent junction of an earlier fillet — a limit of the kernel, and the edge has to be dropped
    pub takes_up_to: Option<f64>,
}

/// An expression parsing error. Kept apart from the geometric ones: these are read somewhere else, in the
/// formula field, and they carry a fragment of what the user typed, which must not be translated — they wrote
/// it themselves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExprError {
    /// an unknown character in the input
    UnknownChar(String),
    /// an unknown function
    UnknownFn(String),
    /// An unknown name: not a constant, not a parameter, and written without parentheses, so it was not
    /// called as a function.
    ///
    /// Separate from `UnknownFn` because the advice differs: there is nothing to do about an unknown function,
    /// while an unknown name calls for creating a parameter or fixing a typo. A bare `w` in an expression used
    /// to answer "unknown function: w", which misled exactly where parameters are actually used.
    UnknownName(String),
    /// the function needs exactly one argument
    NeedsOneArg(String),
    /// the function needs two arguments
    NeedsTwoArgs(String),
    /// a closing parenthesis was expected
    ExpectedParen,
    /// a closing parenthesis was expected after the arguments
    ExpectedParenAfterArgs,
    /// an unexpected token
    UnexpectedToken(String),
    /// The expression ends too early: a number or a name was expected and the input ran out, as in `60 +` or
    /// `w/`.
    ///
    /// A variant of its own rather than `UnexpectedToken("None")`: the end of input is not a token, and putting
    /// its debug name into a message shows the user the internals. That is what used to happen — the parameter
    /// window read "Unexpected token None".
    UnexpectedEnd,
    /// trailing input left over
    TrailingInput(String),
    /// the result is not a number, from a division by zero or similar
    NotANumber,
}

/// A kernel error: a fact rather than a phrase, with the words supplied by the application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CoreError {
    /// The operation failed inside the kernel itself.
    OpFailed(Op),
    /// Only the real kernel can perform this operation, and the stub answered instead — in tests, or in a
    /// build without the kernel.
    KernelRequired(Op),
    /// The source body is not built, its own feature having failed earlier in the timeline.
    SourceBodyNotBuilt,
    /// The source part has no active body, as with a mirrored part or a pattern instance.
    SourcePartHasNoBody,
    /// Body A of the boolean is not built.
    BodyANotBuilt,
    /// Body B of the boolean is not built.
    BodyBNotBuilt,
    /// The face reference was not found in the source body: the name went stale after an edit earlier in the
    /// timeline.
    FaceNotFound,
    /// The same for a set of faces.
    FacesNotFound,
    /// The draft did not take: the angle is too large for this geometry, most often a thin wall left by a
    /// shell.
    DraftFailed { angle: f64 },
    /// A solid-only tool such as a fillet or a chamfer was applied to a surface.
    NeedsSolidNotSheet,
    /// There is nothing to push a sheet face with: pushing is an operation for solids, and a sheet gets its
    /// thickness from thickening.
    PushFaceOnSheet,
    /// The surface did not match the opening; this many edges were left without a counterpart.
    SurfaceDoesNotClose { free: u32 },
    /// There is nothing to stitch: the selected surfaces share no edges and do not touch. This is common
    /// after filleting, where adjacent faces are separated by a rounded strip.
    StitchNothingJoined,
    /// Thickening could not give this face a thickness: the shape does not allow it and the offset runs into
    /// itself.
    ThickenFaceRefused,
    /// The plate was built but did not join the part.
    ThickenPlateNotJoined,
    /// A shell cannot be built on this body: offsetting the faces fails inside the kernel, which raises an
    /// internal error. This is neither the user's fault nor a lost reference but a limit of the algorithm.
    ShellNotBuiltHere,
    /// Shelling a body assembled from copies, by a pattern or a mirror: it has many shells and the kernel
    /// cannot offset such a body. The shell count travels in the error, leaving the decision to the user.
    ShellOfMultiShellBody { shells: u32 },
    /// Mirroring a hollow part about one of its own faces: fusing the halves produces a body with extra
    /// shells that nothing repairs. A limitation of the kernel rather than a mistake by the user, and it has
    /// to be said in words rather than shown as a red node with no reason given.
    MirrorOfHollowBody,
    /// The operation broke the part into several bodies, violating the rule that a part is one body, and such
    /// a result cannot be accepted. The piece count travels in the error, leaving the next step to the user.
    OperationSplitBody { pieces: u32 },
    /// The cut removed nothing: the tool did not intersect the part. Staying silent is not an option, since a
    /// body boolean consumes the tool in the process and the user loses a body and gets nothing in return.
    CutRemovedNothing,
    /// The shell wall is thicker than the smallest fillet or chamfer on the body: the offset consumes them
    /// entirely and the kernel will not build such a shell. The numbers belong in the message, since it is the
    /// user who decides what to change.
    ShellThicknessOverRound { thickness: f64, limit: f64 },
    /// The edges of the patch were not found on the body: the boundary is gone and there is nothing to
    /// stretch a surface over.
    /// Not one of the edges a feature recorded is in the body any more: the names were minted by a feature
    /// above it in the timeline, and that feature was edited. `asked` is how many were named, which is what
    /// tells a stale reference from a feature that never had one.
    EdgesNotFound { asked: usize },
    /// The mirror plane was deleted. Silently falling back to a world plane is not acceptable: a measurement
    /// showed the part moving as a result — x from 10 to 90 about a datum at x = 50 became −30 to 30 about the
    /// world YZ plane — and all of it without a single word.
    MirrorPlaneDeleted,
    /// The cutting plane was deleted, so there is nothing to cut with.
    CutPlaneDeleted,
    /// The plane used to split faces was deleted.
    SplitPlaneDeleted,
    /// The plane of a mirrored part is not set, as in a file from an older version.
    MirrorPlaneUnset,
    /// The plane normal is zero, so no direction is defined.
    ZeroNormal,
    /// Zero thickness produces no plate.
    ZeroThickness,
    /// Zero distance leaves nowhere to push the face.
    ZeroPushDistance,
    /// The kernel returned a solid that fails validation. Such a solid used to become a part silently: holes
    /// appeared on screen instead of walls, the volume was nonsense, and later operations landed on a broken
    /// face. Refusing is the honest answer.
    BrokenSolid,
    /// The split now divides the body into a different number of pieces than when it was created.
    SplitPieceCount { got: usize, want: usize },
    /// A loft needs at least two closed sections.
    LoftNeedsTwoSections,
    /// A draft needs faces to tilt and a neutral face.
    DraftNeedsFaces,
    /// The sweep profile was not found.
    SweepProfileMissing,
    /// The sweep path was not found.
    SweepPathMissing,
    /// The sketch has no isolated points to place holes at.
    NoIsolatedPointsForHoles,
    /// There are no points for the holes.
    NoPointsForHoles,
    /// Thread: the rim of the cylinder or hole, a circular edge, was not found.
    ThreadRimNotFound,
    /// Auger: the rim of the shaft, a circular edge, was not found.
    AugerRimNotFound,
    /// Auger: the pitch and the length have to be greater than zero.
    AugerBadPitchOrLength,
    /// Auger: the outer diameter is not greater than the shaft diameter.
    AugerOuterNotBigger { outer: f64, shaft: f64 },
    /// The thread removed nothing: the volume did not change.
    ThreadRemovedNothing { before: f64, after: f64 },
    /// The auger added nothing: the volume did not change.
    AugerAddedNothing { before: f64, after: f64 },
    /// The sketch profile was not found: the contour disappeared after the sketch was edited.
    ProfileNotFound,
    /// The profile crosses the axis of revolution, and no CAD builds a solid of revolution from that.
    RevolveProfileCrossesAxis,
    /// The thread length is not set.
    ThreadLengthUnset,
    /// Thread: the pitch is too small for this geometry.
    ThreadPitchTooSmall { pitch: f64 },
    /// Thread: the depth of a turn is not smaller than the radius, so a turn would consume the whole shaft.
    ThreadDepthTooDeep { depth: f64, radius: f64, dia: f64, pitch: f64 },
    /// Thread: too many turns.
    ThreadTooManyTurns { turns: f64 },
    /// Isolation: a body may only be built inside a part.
    BodyOnlyInPart,
    /// Isolation: the input belongs to a different component.
    CrossComponentInput { input: crate::model::Id },
    /// Isolation: the input sketch sits on a face of another body without an external reference.
    SketchOnForeignFace { input: crate::model::Id },
    /// A warning: the supporting face of the sketch was lost by name and the nearest match was taken, so the
    /// feature may have landed in the wrong place. This does not fail the rebuild, but the user has to be told
    /// why the result came out different.
    SketchFaceRefLost { sketch: crate::model::Id, body: crate::model::Id },
    /// There are no contours for the operation.
    NoContours,
    /// Every selected edge is a smooth junction, a boundary of a fillet, so there is nothing to fillet or
    /// chamfer.
    AllEdgesSmooth,
    /// The fillet did not take on these edges at this radius, with a breakdown per edge.
    FilletRadiusTooBig {
        radius: f64,
        /// what exactly is wrong with each rejected edge
        issues: Vec<FilletEdgeIssue>,
        /// how many smooth junctions were excluded automatically, there being nothing to fillet on them
        smooth_skipped: usize,
    },
    /// Fillet: the edges only take one at a time and will not go together.
    FilletEdgesOneByOne { radius: f64 },
    /// The chamfer failed: the leg is larger than the side.
    ChamferTooBig { dist: f64 },
    /// The auger flight was not built.
    AugerFlightFailed,
    /// The thread was not built, on account of its pitch, length or diameter.
    ThreadFailed,
    /// The faces cannot be removed: the adjacent surfaces do not extend, or the reference is stale.
    RemoveFacesFailed { why: String },
    /// The pattern produced an empty result.
    ArrayEmpty,
    /// The operation produced an empty body.
    EmptyResult,
    /// An assembly constraint is not satisfied: a residual is left.
    JointUnsatisfied { residual: f64 },
    /// An expression parsing error.
    Expr(ExprError),
    /// A message from the kernel itself. It arrives from C++ and is not subject to translation: it is kernel
    /// diagnostics rather than a sentence for the user. It is shown as it is, next to the translated
    /// "operation failed".
    Kernel(String),
}

impl CoreError {

    /// A temporary refusal, worth retrying once the data appears.
    ///
    /// "The source body is not built" is not a broken recipe but an ordering of events: there is no live B-rep
    /// yet, because the project has only just been opened and the geometry came from the file. Such a node has
    /// to stay dirty and rebuild once its source appears. Everything else is a final refusal, and repeating it
    /// every frame means computing something that cannot succeed, without ever stopping.
    pub fn retryable(&self) -> bool {
        matches!(self, CoreError::SourceBodyNotBuilt | CoreError::BodyANotBuilt | CoreError::BodyBNotBuilt)
    }
    /// The localisation key. The application uses it to fetch the text in the language of the user.
    ///
    /// The key is derived from the variant rather than written next to it: otherwise a new variant could be
    /// added and its string forgotten, which is exactly the class of mistake this whole arrangement exists to
    /// prevent.
    pub fn key(&self) -> String {
        use CoreError::*;
        match self {
            OpFailed(op) => format!("error-op-failed-{}", op.key()),
            KernelRequired(op) => format!("error-kernel-required-{}", op.key()),
            SourceBodyNotBuilt => "error-source-body-not-built".into(),
            SourcePartHasNoBody => "error-source-part-has-no-body".into(),
            BodyANotBuilt => "error-body-a-not-built".into(),
            BodyBNotBuilt => "error-body-b-not-built".into(),
            FaceNotFound => "error-face-not-found".into(),
            FacesNotFound => "error-faces-not-found".into(),
            SurfaceDoesNotClose { .. } => "error-surface-does-not-close".into(),
            ShellThicknessOverRound { .. } => "error-shell-thickness-over-round".into(),
            OperationSplitBody { .. } => "error-operation-split-body".into(),
            MirrorOfHollowBody => "error-mirror-of-hollow-body".into(),
            ShellOfMultiShellBody { .. } => "error-shell-of-multi-shell-body".into(),
            ShellNotBuiltHere => "error-shell-not-built-here".into(),
            CutRemovedNothing => "error-cut-removed-nothing".into(),
            StitchNothingJoined => "error-stitch-nothing-joined".into(),
            ThickenFaceRefused => "error-thicken-face-refused".into(),
            ThickenPlateNotJoined => "error-thicken-plate-not-joined".into(),
            PushFaceOnSheet => "error-push-face-on-sheet".into(),
            NeedsSolidNotSheet => "error-needs-solid-not-sheet".into(),
            DraftFailed { .. } => "error-draft-failed".into(),
            EdgesNotFound { .. } => "error-edges-not-found".into(),
            CutPlaneDeleted => "error-cut-plane-deleted".into(),
            MirrorPlaneDeleted => "error-mirror-plane-deleted".into(),
            SplitPlaneDeleted => "error-split-plane-deleted".into(),
            MirrorPlaneUnset => "error-mirror-plane-unset".into(),
            ZeroNormal => "error-zero-normal".into(),
            ZeroThickness => "error-zero-thickness".into(),
            ZeroPushDistance => "error-zero-push-distance".into(),
            BrokenSolid => "error-broken-solid".into(),
            SplitPieceCount { .. } => "error-split-piece-count".into(),
            LoftNeedsTwoSections => "error-loft-needs-two-sections".into(),
            DraftNeedsFaces => "error-draft-needs-faces".into(),
            SweepProfileMissing => "error-sweep-profile-missing".into(),
            SweepPathMissing => "error-sweep-path-missing".into(),
            NoIsolatedPointsForHoles => "error-no-isolated-points-for-holes".into(),
            NoPointsForHoles => "error-no-points-for-holes".into(),
            ThreadRimNotFound => "error-thread-rim-not-found".into(),
            AugerRimNotFound => "error-auger-rim-not-found".into(),
            AugerBadPitchOrLength => "error-auger-bad-pitch-or-length".into(),
            AugerOuterNotBigger { .. } => "error-auger-outer-not-bigger".into(),
            ThreadRemovedNothing { .. } => "error-thread-removed-nothing".into(),
            AugerAddedNothing { .. } => "error-auger-added-nothing".into(),
            ProfileNotFound => "error-profile-not-found".into(),
            RevolveProfileCrossesAxis => "error-revolve-profile-crosses-axis".into(),
            ThreadLengthUnset => "error-thread-length-unset".into(),
            ThreadPitchTooSmall { .. } => "error-thread-pitch-too-small".into(),
            ThreadDepthTooDeep { .. } => "error-thread-depth-too-deep".into(),
            ThreadTooManyTurns { .. } => "error-thread-too-many-turns".into(),
            BodyOnlyInPart => "error-body-only-in-part".into(),
            CrossComponentInput { .. } => "error-cross-component-input".into(),
            SketchOnForeignFace { .. } => "error-sketch-on-foreign-face".into(),
            SketchFaceRefLost { .. } => "error-sketch-face-ref-lost".into(),
            NoContours => "error-no-contours".into(),
            AllEdgesSmooth => "error-all-edges-smooth".into(),
            FilletRadiusTooBig { .. } => "error-fillet-radius-too-big".into(),
            FilletEdgesOneByOne { .. } => "error-fillet-edges-one-by-one".into(),
            ChamferTooBig { .. } => "error-chamfer-too-big".into(),
            AugerFlightFailed => "error-auger-flight-failed".into(),
            ThreadFailed => "error-thread-failed".into(),
            RemoveFacesFailed { .. } => "error-remove-faces-failed".into(),
            ArrayEmpty => "error-array-empty".into(),
            EmptyResult => "error-empty-result".into(),
            JointUnsatisfied { .. } => "error-joint-unsatisfied".into(),
            Expr(e) => e.key().into(),
            Kernel(_) => "error-kernel-message".into(),
        }
    }
}

impl ExprError {
    pub fn key(&self) -> &'static str {
        use ExprError::*;
        match self {
            UnknownChar(_) => "error-expr-unknown-char",
            UnknownFn(_) => "error-expr-unknown-fn",
            UnknownName(_) => "error-expr-unknown-name",
            NeedsOneArg(_) => "error-expr-needs-one-arg",
            NeedsTwoArgs(_) => "error-expr-needs-two-args",
            ExpectedParen => "error-expr-expected-paren",
            ExpectedParenAfterArgs => "error-expr-expected-paren-after-args",
            UnexpectedToken(_) => "error-expr-unexpected-token",
            UnexpectedEnd => "error-expr-unexpected-end",
            TrailingInput(_) => "error-expr-trailing-input",
            NotANumber => "error-expr-not-a-number",
        }
    }
}

/// Fallback text for when no translation is available at all.
///
/// The core does not know a language and should not, but staying silent during debugging is no good either:
/// the tests of the core and its diagnostics read an error without an application present. So what follows is
/// a short technical wording in the language of the code rather than a translation: it is for a developer, not
/// for a user.
impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CoreError::*;
        match self {
            OpFailed(op) => write!(f, "operation failed: {:?}", op),
            KernelRequired(op) => write!(f, "real OCCT kernel required: {:?}", op),
            SplitPieceCount { got, want } => write!(f, "split produced {got} pieces, expected {want}"),
            AugerOuterNotBigger { outer, shaft } => write!(f, "auger outer {outer:.1} not bigger than shaft {shaft:.1}"),
            ThreadRemovedNothing { before, after } => write!(f, "thread removed nothing ({before:.0} -> {after:.0})"),
            AugerAddedNothing { before, after } => write!(f, "auger added nothing ({before:.0} -> {after:.0})"),
            JointUnsatisfied { residual } => write!(f, "joint unsatisfied, residual {residual:.3}"),
            ThreadPitchTooSmall { pitch } => write!(f, "thread pitch {pitch:.3} too small"),
            ThreadDepthTooDeep { depth, radius, .. } => write!(f, "thread depth {depth:.2} >= radius {radius:.2}"),
            ThreadTooManyTurns { turns } => write!(f, "{turns:.0} turns is too many"),
            CrossComponentInput { input } => write!(f, "cross-component input {input}"),
            SketchOnForeignFace { input } => write!(f, "sketch input {input} sits on a foreign face"),
            SketchFaceRefLost { sketch, body } => write!(f, "sketch {sketch} lost its face ref on body {body}"),
            FilletRadiusTooBig { radius, issues, .. } => write!(f, "fillet R{radius:.2} failed on {} edges", issues.len()),
            FilletEdgesOneByOne { radius } => write!(f, "fillet R{radius:.2} only works edge by edge"),
            ChamferTooBig { dist } => write!(f, "chamfer {dist:.2} too big"),
            SurfaceDoesNotClose { free } => write!(f, "surface leaves {free} free edges"),
            ShellThicknessOverRound { thickness, limit } => write!(f, "shell {thickness:.2} exceeds smallest round {limit:.2}"),
            OperationSplitBody { pieces } => write!(f, "operation split the body into {pieces} pieces"),
            MirrorOfHollowBody => write!(f, "mirror of a hollow body is not supported"),
            ShellOfMultiShellBody { shells } => write!(f, "shell of a body with {shells} shells is not supported"),
            ShellNotBuiltHere => write!(f, "kernel failed to offset faces on this body"),
            StitchNothingJoined => write!(f, "surfaces share no edges — nothing to stitch"),
            ThickenFaceRefused => write!(f, "cannot give this face a thickness — the offset would run into itself; try a smaller thickness or thicken earlier"),
            ThickenPlateNotJoined => write!(f, "the plate was built but would not join the part"),
            PushFaceOnSheet => write!(f, "push face on a sheet"),
            NeedsSolidNotSheet => write!(f, "solid tool on a sheet"),
            DraftFailed { angle } => write!(f, "draft {angle:.1} failed"),
            RemoveFacesFailed { why } => write!(f, "cannot remove faces: {why}"),
            Kernel(msg) => write!(f, "{msg}"),
            Expr(e) => write!(f, "{e}"),
            other => write!(f, "{}", other.key()),
        }
    }
}

impl std::fmt::Display for ExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ExprError::*;
        match self {
            UnknownChar(c) => write!(f, "unknown character: {c}"),
            UnknownFn(n) => write!(f, "unknown function: {n}"),
            UnknownName(n) => write!(f, "unknown name: {n}"),
            NeedsOneArg(n) => write!(f, "{n}() needs one argument"),
            NeedsTwoArgs(n) => write!(f, "{n}() needs two arguments"),
            UnexpectedToken(t) => write!(f, "unexpected token: {t}"),
            UnexpectedEnd => write!(f, "expression ends too early"),
            TrailingInput(t) => write!(f, "trailing input: {t}"),
            ExpectedParen => write!(f, "expected ')'"),
            ExpectedParenAfterArgs => write!(f, "expected ')' after arguments"),
            NotANumber => write!(f, "result is not a number"),
        }
    }
}
