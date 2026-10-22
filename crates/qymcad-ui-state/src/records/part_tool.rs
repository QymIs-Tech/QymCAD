//! THE TOOLS OF A PART, by name, and what a tool is - the questions the window, the picking and the drawing ask of the
//! tool in hand instead of naming it.

/// HOW A TOOL SHOWS WHAT IT HAS TAKEN in the view, before Enter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shows {
    /// Every face of its set lit, by persistent id: the faces a shell opens.
    FaceSet,
    /// The one face of the selection: where a hole goes.
    FaceOfTheSelection,
    /// Its one face lit and the plate it will grow, the outline of the face shifted by the thickness.
    FaceAndPlate,
    /// Its one face lit and the outline of the face shifted by the offset it will be pushed by.
    FaceAndPush,
    /// The faces it makes a surface of, in the colour of what that surface does; a replacement shows its surface too.
    FacesOfASurface,
    /// The sheet kept and the body that cuts it.
    SheetAndCutter,
    /// The bodies it takes whole: the sheets to stitch.
    Bodies,
    /// The mesh it takes whole, and once counted what it found on it.
    MeshAndWhatItFound,
    /// The faces that will cease to exist.
    FacesThatGo,
    /// The faces it tilts, and the neutral face they turn about.
    FacesAndTheirNeutral,
    /// Nothing of its own.
    Nothing,
}

/// WHAT A DESCRIPTION OF THE SELECTION ("all parallel", "the loop") MAY GIVE THE TOOL.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TakesDescribed {
    /// A set of edges.
    EdgeSet,
    /// A set of faces.
    FaceSet,
    /// Exactly one face: "all parallel" is no description for it.
    OneFace,
}

impl TakesDescribed {
    /// Does the description give edges rather than faces?
    pub fn edges(self) -> bool {
        self == TakesDescribed::EdgeSet
    }

    /// May the description give more than one?
    pub fn many(self) -> bool {
        self != TakesDescribed::OneFace
    }
}

/// A TOOL OF A PART: a command of its timeline, a primitive or a datum.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PartTool {
    Extrude,
    Revolve,
    Fillet,
    Chamfer,
    Shell,
    Hole,
    Sweep,
    Loft,
    Box,
    Cylinder,
    Sphere,
    Cone,
    Torus,
    Prism,
    Mirror,
    LinearPattern,
    CircularPattern,
    DatumPlane,
    DatumPoint,
    DatumAxis,
    Draft,
    Thread,
    PushFace,
    RemoveFace,
    SplitBody,
    Thicken,
    SplitFace,
    FaceCopy,
    SurfaceReplace,
    Patch,
    Stitch,
    Trim,
    Recognise,
    OffsetSurface,
}

impl PartTool {
    /// Every tool of a part, in the order of the bar.
    pub const ALL: [PartTool; 34] = [
        PartTool::Extrude,
        PartTool::Revolve,
        PartTool::Sweep,
        PartTool::Loft,
        PartTool::Box,
        PartTool::Cylinder,
        PartTool::Sphere,
        PartTool::Cone,
        PartTool::Torus,
        PartTool::Prism,
        PartTool::Fillet,
        PartTool::Chamfer,
        PartTool::Shell,
        PartTool::Hole,
        PartTool::Draft,
        PartTool::PushFace,
        PartTool::RemoveFace,
        PartTool::Thicken,
        PartTool::FaceCopy,
        PartTool::OffsetSurface,
        PartTool::SurfaceReplace,
        PartTool::Patch,
        PartTool::Stitch,
        PartTool::Trim,
        PartTool::Recognise,
        PartTool::SplitBody,
        PartTool::SplitFace,
        PartTool::Thread,
        PartTool::Mirror,
        PartTool::LinearPattern,
        PartTool::CircularPattern,
        PartTool::DatumPlane,
        PartTool::DatumPoint,
        PartTool::DatumAxis,
    ];

    /// A PRIMITIVE: a body of its own fields, made without a sketch.
    pub fn is_a_primitive(self) -> bool {
        matches!(self, PartTool::Box | PartTool::Cylinder | PartTool::Sphere | PartTool::Cone | PartTool::Torus | PartTool::Prism)
    }

    /// A DATUM: a plane, a point or an axis to build from, shared by the part and the assembly.
    pub fn is_a_datum(self) -> bool {
        matches!(self, PartTool::DatumPlane | PartTool::DatumPoint | PartTool::DatumAxis)
    }

    /// IT PICKS THE CONTOURS OF A SKETCH AS ITS PROFILES, one by one, in the sketch or in 3D: the extrusion and the
    /// revolution.
    pub fn picks_profiles(self) -> bool {
        matches!(self, PartTool::Extrude | PartTool::Revolve)
    }

    /// IT BLENDS THE EDGES it is given: the fillet rounds them, the chamfer cuts them.
    pub fn blends_edges(self) -> bool {
        matches!(self, PartTool::Fillet | PartTool::Chamfer)
    }

    /// IT TAKES EDGES CLICKED IN THE VIEW: the blends, and the patch, whose edges close the hole it fills.
    pub fn picks_edges(self) -> bool {
        self.blends_edges() || self == PartTool::Patch
    }

    /// IT READS THE LIVE B-REP while it is in hand: the blends take its edges, the thread the axis of a face.
    pub fn reads_the_live_brep(self) -> bool {
        self.blends_edges() || self == PartTool::Thread
    }

    /// A PATTERN OF A BODY: linear or circular.
    pub fn is_a_pattern(self) -> bool {
        matches!(self, PartTool::LinearPattern | PartTool::CircularPattern)
    }

    /// IT SIZES WHAT IS PICKED, so before the pick there is nothing to size: the hole on its face and the patterns of
    /// their body.
    pub fn sizes_what_is_picked(self) -> bool {
        self == PartTool::Hole || self.is_a_pattern()
    }

    /// IT AIMS AT A PLANE, a datum or a face under a click: the mirror plane, the cutting plane, the base of a datum
    /// plane.
    pub fn aims_at_a_plane(self) -> bool {
        self.splits() || matches!(self, PartTool::Mirror | PartTool::DatumPlane)
    }

    /// IT SPLITS BY A PLANE: a body into pieces, or the faces of a body.
    pub fn splits(self) -> bool {
        matches!(self, PartTool::SplitBody | PartTool::SplitFace)
    }

    /// IT SAYS ITSELF WHAT IT STILL WANTS when Enter comes before its picks: the general "not ready" would say less
    /// than its own refusal.
    pub fn says_what_it_wants(self) -> bool {
        matches!(self, PartTool::DatumAxis | PartTool::PushFace | PartTool::RemoveFace | PartTool::Thicken | PartTool::FaceCopy | PartTool::Patch | PartTool::Trim)
    }

    /// IT WORKS ON A SOLID ONLY: a click does not catch on a sheet lying over the face of the part. The tools for
    /// which a sheet is lawful - thicken, replace a face, stitch, trim, patch, copy a face - are not among them.
    pub fn wants_a_solid(self) -> bool {
        matches!(
            self,
            PartTool::Fillet | PartTool::Chamfer | PartTool::Shell | PartTool::Hole | PartTool::Draft | PartTool::PushFace | PartTool::RemoveFace | PartTool::SplitBody | PartTool::SplitFace
        )
    }

    /// IT TAKES THE FACE UNDER A CLICK rather than the body.
    pub fn picks_faces(self) -> bool {
        self.gathers_faces_of_one_body() || matches!(self, PartTool::Hole | PartTool::SurfaceReplace)
    }

    /// IT GATHERS FACES OF ONE BODY: a click on a face of another body starts the set afresh on it, as the ids of faces
    /// are local to a body. Replacing a face is not among them - there a click on another body names the surface.
    pub fn gathers_faces_of_one_body(self) -> bool {
        matches!(self, PartTool::Shell | PartTool::Draft | PartTool::PushFace | PartTool::RemoveFace | PartTool::Thicken | PartTool::FaceCopy | PartTool::OffsetSurface)
    }

    /// A CLICK ON A FACE TAKES IT OR LETS IT GO, by its id.
    pub fn toggles_faces(self) -> bool {
        matches!(self, PartTool::Hole | PartTool::PushFace | PartTool::RemoveFace | PartTool::Thicken | PartTool::FaceCopy | PartTool::OffsetSurface)
    }

    /// IT TAKES EXACTLY ONE FACE: a click replaces the one before. Pushing several faces by one offset would give a
    /// different result on each, their normals differ; one plate comes of one face.
    pub fn takes_one_face(self) -> bool {
        matches!(self, PartTool::Hole | PartTool::PushFace | PartTool::Thicken)
    }

    /// IT TAKES A WHOLE BODY by a click: stitching sheets, recognising a mesh.
    pub fn picks_bodies(self) -> bool {
        matches!(self, PartTool::Stitch | PartTool::Recognise)
    }

    /// THE FACES IT TAKES STAY and a surface is added of them: a copy and an offset. A replacement takes them away.
    pub fn keeps_the_faces_it_takes(self) -> bool {
        matches!(self, PartTool::FaceCopy | PartTool::OffsetSurface)
    }

    /// HOW IT SHOWS WHAT IT HAS TAKEN in the view.
    pub fn shows(self) -> Shows {
        match self {
            PartTool::Shell => Shows::FaceSet,
            PartTool::Hole => Shows::FaceOfTheSelection,
            PartTool::Thicken => Shows::FaceAndPlate,
            PartTool::PushFace => Shows::FaceAndPush,
            PartTool::FaceCopy | PartTool::OffsetSurface | PartTool::SurfaceReplace => Shows::FacesOfASurface,
            PartTool::Trim => Shows::SheetAndCutter,
            PartTool::Stitch => Shows::Bodies,
            PartTool::Recognise => Shows::MeshAndWhatItFound,
            PartTool::RemoveFace => Shows::FacesThatGo,
            PartTool::Draft => Shows::FacesAndTheirNeutral,
            _ => Shows::Nothing,
        }
    }

    /// WHAT A DESCRIPTION OF THE SELECTION MAY GIVE IT: the blends a set of edges, the shell, the draft and the removal
    /// a set of faces, the push and the thickening one face; the rest take no descriptions.
    pub fn takes_described(self) -> Option<TakesDescribed> {
        match self {
            PartTool::Fillet | PartTool::Chamfer => Some(TakesDescribed::EdgeSet),
            PartTool::Shell | PartTool::Draft | PartTool::RemoveFace => Some(TakesDescribed::FaceSet),
            PartTool::PushFace | PartTool::Thicken => Some(TakesDescribed::OneFace),
            _ => None,
        }
    }
}
