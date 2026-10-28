//! THE DOCUMENT IN THIS PROCESS, with no window: the project, the live bodies the kernel built for it, and the file
//! it came from.
//!
//! The live bodies stay between calls, as the window keeps them: a rebuild after one change rebuilds only what that
//! change marked dirty, and the rest is taken from the cache, so the tenth feature of a part costs one feature, not
//! ten.
use std::collections::HashMap;

use qymcad_core::model::{Id, Project};
use qymcad_kernel::{OcctKernel, Shape};

use crate::workshop::Workshop;

/// The document being worked on.
pub struct Bench {
    project: Project,
    /// The live body of every body id the kernel built.
    shapes: HashMap<Id, Shape>,
    /// The file the document was opened from or last saved to.
    path: Option<String>,
}

impl Default for Bench {
    /// A new document: the root assembly with one empty part standing open, as the window starts a new file.
    fn default() -> Self {
        let mut project = Project::default();
        project.new_document();
        Bench { project, shapes: HashMap::new(), path: None }
    }
}

impl Bench {
    /// Rebuild what is dirty with the real kernel, under its lock: OCCT is not thread-safe.
    fn rebuild(&mut self) {
        let _gate = qymcad_kernel::kernel_gate();
        let kernel = OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut self.shapes)), quality_k: self.project.geom_quality.deflection_k(), ..Default::default() };
        let _ = self.project.regenerate(&kernel);
        self.shapes = kernel.shapes.into_inner();
    }
}

impl Workshop for Bench {
    fn project(&self) -> &Project {
        &self.project
    }

    fn project_mut(&mut self) -> &mut Project {
        &mut self.project
    }

    /// No window, no undo: an operation here is the change and its rebuild.
    fn begin(&mut self, _what: &str) {}

    fn finish(&mut self) {
        self.rebuild();
    }

    fn shape(&self, body: Id) -> Option<&Shape> {
        self.shapes.get(&body)
    }

    fn place(&self) -> &'static str {
        "file"
    }

    fn path(&self) -> Option<String> {
        self.path.clone()
    }

    fn new_document(&mut self) -> Result<(), String> {
        *self = Bench::default();
        Ok(())
    }

    /// Open a `.qcad`: the document is read and built anew by the kernel, the bodies of imports raised from the
    /// sources the file carries first - an import has no recipe to rebuild it from.
    fn open(&mut self, path: &str) -> Result<(), String> {
        let mut project = qymcad_io::load_project(path)?;
        project.ensure_document();
        for n in &mut project.timeline {
            if n.kind.body().is_some() {
                n.dirty = true;
            }
        }
        self.shapes = qymcad_kernel::imports::raise_imports(&project).into_iter().collect();
        self.project = project;
        self.path = Some(path.to_string());
        self.rebuild();
        Ok(())
    }

    fn saved_as(&mut self, path: &str) {
        self.path = Some(path.to_string());
    }
}
