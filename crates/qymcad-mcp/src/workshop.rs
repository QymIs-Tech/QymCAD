//! THE DOCUMENT THE TOOLS WORK ON, wherever it lives: in this process with no window (`Bench`), or in a running
//! window, where every step is drawn as it is made and goes onto the person's undo stack.
//!
//! The tools are written once, against this; what differs between the two is only how an operation opens and closes
//! and what the files are. A window rebuilds in the background, so a step closed there is not built yet: the answer to
//! it is read once the window's rebuild has finished, not right after `finish`.
use qymcad_core::model::{Id, Project};
use qymcad_kernel::Shape;

/// The document a tool acts on.
pub trait Workshop {
    fn project(&self) -> &Project;
    fn project_mut(&mut self) -> &mut Project;
    /// Open one operation, named for the person: in a window, the step Undo takes back whole.
    fn begin(&mut self, what: &str);
    /// Close it: the change is rebuilt - at once with no window, by the window's scheduler in one.
    fn finish(&mut self);
    /// The live body the kernel built for a body id: the exact geometry behind STEP, a saved B-rep and the volume.
    fn shape(&self, body: Id) -> Option<&Shape>;
    /// Where the document lives, said to the model: "window" or "file".
    fn place(&self) -> &'static str;
    /// The file the document was opened from or saved to.
    fn path(&self) -> Option<String>;
    /// Start a new document in place of this one.
    fn new_document(&mut self) -> Result<(), String>;
    /// Open a `.qcad` in place of this document; it is rebuilt before the call returns.
    fn open(&mut self, path: &str) -> Result<(), String>;
    /// Remember the file the document was saved to.
    fn saved_as(&mut self, path: &str);
    /// Turn the camera to a standard view and frame the model. Only a window has a camera.
    fn look(&mut self, _view: View) -> Result<(), String> {
        Err(NO_WINDOW.into())
    }
    /// Take the last step back, as Ctrl+Z does: the name of the step taken back. Only a window keeps the steps.
    fn undo(&mut self) -> Result<String, String> {
        Err(NO_WINDOW.into())
    }

    /// THE BODIES A PERSON SEES: built, visible, not taken up into another body by a boolean or a fillet.
    fn shown_bodies(&self) -> Vec<Id> {
        let p = self.project();
        let consumed = p.consumed_bodies();
        p.bodies.iter().filter(|b| b.visible && !b.mesh.tris.is_empty() && !consumed.contains(&b.id)).map(|b| b.id).collect()
    }

    /// The exact volume of a body, mm^3, when the kernel holds it live.
    fn exact_volume(&self, body: Id) -> Option<f64> {
        self.shape(body).map(Shape::volume)
    }

    /// THE NODES THAT DO NOT BUILD NOW, with the core's own code of the reason - the same in every language.
    fn failed(&self) -> Vec<NodeError> {
        let mut out: Vec<NodeError> = self.project().regen_errors.iter().map(|(node, e)| NodeError { node: *node, reason: format!("{e:?}") }).collect();
        out.sort_by_key(|f| f.node);
        out
    }

    /// Save as `.qcad`, the live bodies inside, so the window opens the file without rebuilding it. An empty
    /// document is refused over a full file, as the window refuses it.
    fn save(&mut self, path: &str) -> Result<(), String> {
        let breps: Vec<_> = self.shown_bodies().into_iter().filter_map(|id| self.shape(id).and_then(Shape::to_brep_bytes).map(|bytes| (id, bytes))).collect();
        qymcad_io::save_project_guarded_with_brep(self.project(), path, &breps)?;
        self.saved_as(path);
        Ok(())
    }

    /// Write every body shown into one STEP file, each where it stands in the assembly. Returns how many.
    fn export_step(&self, path: &str) -> Result<usize, String> {
        let bodies: Vec<_> = self.shown_bodies().into_iter().filter_map(|id| self.shape(id).map(|s| (s, self.project().body_world_transform(id)))).collect();
        qymcad_kernel::write_step(&bodies, path)?;
        Ok(bodies.len())
    }

    /// Write the mesh of every body shown into one STL file, each where it stands in the assembly. Returns how many.
    fn export_stl(&self, path: &str) -> Result<usize, String> {
        let p = self.project();
        let meshes: Vec<qymcad_core::geom::Mesh> = self
            .shown_bodies()
            .into_iter()
            .filter_map(|id| {
                let mut mesh = p.bodies.iter().find(|b| b.id == id)?.mesh.clone();
                mesh.transform(&p.body_world_transform(id));
                Some(mesh)
            })
            .collect();
        if meshes.is_empty() {
            return Err("there is no body to export".into());
        }
        qymcad_io::export_stl(&meshes, path)?;
        Ok(meshes.len())
    }
}

/// The refusal of what only a window does: look, turn the camera.
pub const NO_WINDOW: &str = "this needs a QymCAD window: open QymCAD, tick Settings -> General -> Let an AI assistant work in this window, and call again";

/// A STANDARD VIEW, as the cube of the view names them: where the eye stands looking at the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    /// From the front, right and above: the view a part is usually shown in.
    Iso,
    /// From the back, left and above: the other side of the same part.
    IsoBack,
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl View {
    /// The words a call names the view by.
    pub const WORDS: [&'static str; 8] = ["iso", "iso_back", "front", "back", "left", "right", "top", "bottom"];

    pub fn of_word(word: &str) -> Option<View> {
        let all = [View::Iso, View::IsoBack, View::Front, View::Back, View::Left, View::Right, View::Top, View::Bottom];
        View::WORDS.iter().position(|w| *w == word).map(|i| all[i])
    }

    /// Where the eye stands, as the camera holds it: the turn about Z from +X, and the rise above the XY plane, in
    /// radians. Front looks from -Y, as the default view of the window does. The rise of an isometric view is the one
    /// that makes the three axes equally short, atan(1 / sqrt 2) = 35.26 deg; straight down or up stops 0.06 deg short
    /// of the pole, where the turn about Z would mean nothing.
    pub fn eye(self) -> Eye {
        use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
        let iso = (1.0 / 2f64.sqrt()).atan();
        let pole = FRAC_PI_2 - 0.001;
        match self {
            View::Iso => Eye { turn: -FRAC_PI_4, rise: iso },
            View::IsoBack => Eye { turn: 3.0 * FRAC_PI_4, rise: iso },
            View::Front => Eye { turn: -FRAC_PI_2, rise: 0.0 },
            View::Back => Eye { turn: FRAC_PI_2, rise: 0.0 },
            View::Right => Eye { turn: 0.0, rise: 0.0 },
            View::Left => Eye { turn: PI, rise: 0.0 },
            View::Top => Eye { turn: -FRAC_PI_2, rise: pole },
            View::Bottom => Eye { turn: -FRAC_PI_2, rise: -pole },
        }
    }
}

/// Where the eye stands: the turn about Z and the rise above the XY plane, radians.
pub struct Eye {
    pub turn: f64,
    pub rise: f64,
}

/// A NODE THAT DID NOT BUILD, and the code of the reason.
pub struct NodeError {
    pub node: Id,
    /// The core's own code for the reason (`CoreError`), the same in every language.
    pub reason: String,
}
