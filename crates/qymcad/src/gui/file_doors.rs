//! FILES IN AND OUT: an export planned, meshes and exact files read, the project saved and its recent list.

use qymcad_ui_state::Notice;
use super::*;

/// The parse of the export's target bodies by [`qymcad_core::model::ExportKind`] - ONE parse for both formats.
/// STEP writes only `brep`; STL writes EVERYTHING that is on the screen (`brep` by tessellation plus the
/// `mesh_only` and `stale` meshes). The difference must not be kept quiet: [`ExportPlan::note`] gives it in words,
/// meaning the same thing in both statuses.
#[derive(Default)]
pub(crate) struct ExportPlan {
    /// A live B-rep - the exact geometry.
    pub(crate) brep: Vec<Id>,
    /// An STL import: there never was a B-rep.
    pub(crate) mesh_only: Vec<Id>,
    /// A failed rebuild: the last good geometry is on the screen and there is no B-rep.
    pub(crate) stale: Vec<Id>,
}

impl ExportPlan {
    /// The bodies for STL: everything visible (a B-rep is re-tessellated, the rest go as their stored mesh).
    pub(crate) fn stl_bodies(&self) -> Vec<Id> {
        self.brep.iter().chain(&self.mesh_only).chain(&self.stale).copied().collect()
    }
    /// An honest note appended to the status: what exactly is missing and why. Empty means everything went out
    /// intact. `step=true` gives the wording for STEP (those bodies did NOT reach the file), otherwise for STL
    /// (they did, but as a mesh).
    pub(crate) fn note(&self, step: bool) -> String {
        let (m, s) = (self.mesh_only.len(), self.stale.len());
        if m + s == 0 {
            return String::new();
        }
        let mut what = Vec::new();
        if m > 0 {
            what.push(crate::i18n::tr1("note-mesh-bodies", "n", &m.to_string()));
        }
        if s > 0 {
            what.push(crate::i18n::tr1("note-failed-bodies", "n", &s.to_string()));
        }
        let what = what.join(", ");
        if step {
            crate::i18n::tr1("note-not-exported", "what", &what)
        } else {
            crate::i18n::tr1("note-as-mesh", "what", &what)
        }
    }
}

/// Restore the B-rep shapes of the imported solids from the embedded sources, with their names (`raise_imports`).
pub(crate) fn restore_import_shapes_for(project: &Project) -> Vec<(Id, qymcad_kernel::Shape)> {
    qymcad_kernel::imports::raise_imports(project)
}

/// THE DOCUMENT ON DISK: where it came from, what is unsaved, and what is being written or read.
///
/// Ten fields of `App` about ONE subject - the file. They were spread through a hundred, so "is there
/// anything unsaved" and "where does Save write" were questions one answered by remembering which field
/// held what.
pub(crate) struct OnDisk {
    /// The path of the open document; `None` for one that has never been saved.
    pub(crate) project_path: Option<String>,
    /// The last DXF a drawing was imported from - the next import opens there.
    pub(crate) dxf_path: Option<String>,
    /// The folder the last export was written to - the next export opens there.
    pub(crate) export_folder: Option<export_folder::ExportFolder>,
    /// Reading and writing: the background jobs and what they came back with.
    pub(crate) io: DocIo,
    /// What has changed since the last save, and when the last autosave was.
    pub(crate) edits: Edits,
    /// A file chooser that is open, and what to do with the answer.
    pub(crate) file_ask: Option<file_ask::FileAsk>,
    /// A mesh export waiting for its deflection.
    pub(crate) mesh_export: Option<(qymcad_ui_state::MeshFormat, ExportTarget)>,
    /// A crash report left by an earlier run, to be shown once.
    pub(crate) crash_report: Vec<std::path::PathBuf>,
    /// What the person is writing into "report a problem".
    pub(crate) report: crate::gui::report_problem::ReportDraft,
    /// The title as it stands in the window bar - kept so it is not set on every frame.
    pub(crate) title_shown: String,
    /// Somewhere to go once the unsaved-work question has been answered.
    pub(crate) pending_nav: Option<Nav>,
}

/// The GUI tests live in a file of their own (`gui/tests.rs`) rather than at the end of this one.
/// The file name out of a path (used as the name of a sketch or a source).
pub(crate) fn file_name(path: &str) -> String {
    std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

/// Embed the original of an imported file into the document and return its Id.
pub(crate) fn embed_source(project: &mut qymcad_core::model::Project, path: &str) -> Option<Id> {
    match std::fs::read(path) {
        Ok(bytes) => Some(project.add_source(file_name(path), bytes)),
        Err(_) => None,
    }
}

pub(crate) fn snapshot(project: &qymcad_core::model::Project) -> Snapshot {
    // THE BYTES OF THE EMBEDDED SOURCES DO NOT GO INTO A SNAPSHOT. They never change (they are the
    // original of a file imported once), and they weigh tens of megabytes: on a real assembly 89 MB
    // times 40 undo steps is gigabytes of memory and about 90 MB of memcpy for EVERY committed edit.
    // Restoring puts them back from the live document by id (see `restore`).
    let mut project = project.clone_without_source_data();
    project.regen_faces.clear(); // derived: the snapshot holds the faces itself (`faces`)
    project.regen_edges.clear();
    Snapshot { project }
}

/// The entry of a mesh format in the table of formats: its name and extensions live there, once.
pub(crate) fn mesh_entry(format: qymcad_ui_state::MeshFormat) -> qymcad_io::Format {
    match format {
        qymcad_ui_state::MeshFormat::Stl => qymcad_io::Format::Stl,
        qymcad_ui_state::MeshFormat::Obj => qymcad_io::Format::Obj,
        qymcad_ui_state::MeshFormat::Ply => qymcad_io::Format::Ply,
        qymcad_ui_state::MeshFormat::Glb => qymcad_io::Format::Gltf,
        qymcad_ui_state::MeshFormat::ThreeMf => qymcad_io::Format::ThreeMf,
        qymcad_ui_state::MeshFormat::Amf => qymcad_io::Format::Amf,
    }
}

pub(crate) fn open_mesh(regen: &mut Rebuilding, path: String, format: qymcad_ui_state::MeshFormat) {
    // parsing the mesh and detecting its faces run in a worker thread; the UI spins a spinner and the window does not freeze
    let name = mesh_entry(format).name();
    let (tx, rx) = std::sync::mpsc::channel();
    let p = path.clone();
    std::thread::spawn(move || {
        let read = match format {
            qymcad_ui_state::MeshFormat::Stl => qymcad_io::import_stl_named(&p),
            qymcad_ui_state::MeshFormat::Obj => qymcad_io::import_obj(&p),
            qymcad_ui_state::MeshFormat::Ply => qymcad_io::import_ply_coloured(&p).map(|n| vec![n]),
            qymcad_ui_state::MeshFormat::Glb => qymcad_io::import_gltf(&p),
            qymcad_ui_state::MeshFormat::ThreeMf => qymcad_io::import_3mf(&p),
            qymcad_ui_state::MeshFormat::Amf => qymcad_io::import_amf(&p),
        };
        let res = match read {
            Ok(meshes) => {
                let pieces = meshes.into_iter().map(|n| {
                    let faces = qymcad_meshfit::raw_faces(&n.mesh);
                    qymcad_ui_state::MeshPiece { name: n.name, mesh: n.mesh, faces, color: n.color, place: n.place, tri_colors: n.tri_colors, within: n.within }
                });
                JobResult::MeshImported { path: p, format, pieces: pieces.collect() }
            }
            Err(e) => JobResult::Failed(crate::i18n::tr2("g-mesh-error", "format", name, "error", &crate::i18n::name(&e))),
        };
        let _ = tx.send(res);
    });
    regen.busy = Some(Busy { started: std::time::Instant::now(), label: crate::i18n::tr1("g-import-mesh", "format", name), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
}

/// WRITE THE SETTINGS TO A FILE — to carry them to another machine, to share them, to attach them to
/// a bug report.
///
/// The format is the same RON as in storage: a separate "export format" would have to be maintained
/// as a second one, and it would drift from the first at the very first new setting.
pub(crate) fn export_settings_to(set: &Settings, path: &str) -> Result<(), String> {
    let text = ron::ser::to_string_pretty(&set, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// READ THE SETTINGS FROM A FILE and adopt them.
///
/// Missing fields fall back to the factory ones (`serde(default)` on the record): a profile taken
/// from a different version of the program must still read, not be rejected wholesale — otherwise it
/// cannot be shared. A broken file does NOT touch the current settings: the change arrives whole or
/// not at all.
pub(crate) fn import_settings_from(regen: &mut Rebuilding, scheme: &mut SchemeUi, set: &mut Settings, status: &mut Notice, path: &str, ctx: &egui::Context) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let s: Settings = ron::from_str(&text).map_err(|e| e.to_string())?;
    adopt_settings(regen, scheme, set, status, s, ctx);
    Ok(())
}

/// Put a path at the top of the recent list: no duplicates, trimmed to length.
///
/// The freshest one goes first: the list is read from the top, and "the last thing worked on" must
/// be right under the cursor.
pub(crate) fn remember_recent(set: &mut Settings, path: String) {
    if path.trim().is_empty() {
        return;
    }
    set.recent.retain(|p| p != &path);
    set.recent.insert(0, path);
    let n = set.recent_limit.max(1);
    set.recent.truncate(n);
}

/// Drop a path from the recent list — the file would not open (renamed, moved away, the drive
/// unmounted).
///
/// A dead row must not be left there silently: it would be clicked again and again, while the list
/// exists so that one lands on the file at the first attempt.
pub(crate) fn forget_recent(set: &mut Settings, path: &str) {
    set.recent.retain(|p| p != path);
}

/// What a drawing read from IGES adds to the status: that a library's cells are what is shown, and what was not
/// drawn at all. Each is a warning; the line that carries them takes that tone.
pub(crate) fn drawing_note(d: &qymcad_io::IgesDrawing) -> String {
    let mut out = Vec::new();
    if d.definitions_only {
        out.push(crate::i18n::tr("io-iges-definitions-only"));
    }
    if !d.skipped.is_empty() {
        let n: usize = d.skipped.iter().map(|(_, n)| n).sum();
        let kinds = d.skipped.iter().map(|(t, n)| format!("{t} x{n}")).collect::<Vec<_>>().join(", ");
        out.push(crate::i18n::tr2("io-iges-skipped", "n", &n.to_string(), "kinds", &kinds));
    }
    out.join(" ")
}

/// The entry of an exact format in the table of formats: its name and extensions live there, once.
pub(crate) fn exact_entry(format: qymcad_kernel::ExactFormat) -> qymcad_io::Format {
    match format {
        qymcad_kernel::ExactFormat::Step => qymcad_io::Format::Step,
        qymcad_kernel::ExactFormat::Iges => qymcad_io::Format::Iges,
    }
}

pub(crate) fn open_exact(regen: &mut Rebuilding, path: String, format: qymcad_kernel::ExactFormat) {
    // the heavy work (parsing, tessellation, the live solids) runs in a worker thread; the UI shows an
    // "importing" spinner and does not freeze (large assemblies carry fat imports).
    let name = exact_entry(format).name();
    let (tx, rx) = std::sync::mpsc::channel();
    let p = path.clone();
    qymcad_kernel::worker(move || {
        // ONE READING for both the bodies shown and the live solids: reading the file twice cost the whole wait twice
        let res = match qymcad_kernel::read_exact_tree(format, &p, 0.5) {
            Ok(qymcad_kernel::ExactTree { bodies, shapes, nodes }) if !bodies.is_empty() => JobResult::ExactImported { path: p, format, bodies, shapes, nodes },
            // A FILE THAT IS NOT THERE IS SAID SO, before an IGES is tried as a drawing: the drawing reader would speak
            // of the same absence in words of its own
            Err(e) if e.starts_with("cad-file-not-found#") => JobResult::Failed(crate::i18n::trn("io-exact-load-error", &[("format", name), ("error", &crate::i18n::name(&e))])),
            // AN IGES WITH NO SURFACES IS A DRAWING, and it goes into a sketch the way DXF does. Reported behaviour:
            // a chip library cell (curves and subfigures, not a single face) did not open at all.
            _ if format == qymcad_kernel::ExactFormat::Iges => match qymcad_io::read_iges_drawing(&p) {
                Ok(d) => JobResult::DrawingRead { note: drawing_note(&d), path: p, curves: d.curves },
                Err(e) => JobResult::Failed(crate::i18n::name(&e)),
            },
            Ok(_) => JobResult::Failed(crate::i18n::tr1("io-exact-no-solids", "format", name)),
            // the kernel answers with a code, and the person reads words: the code went to the status line as it was
            Err(e) => JobResult::Failed(crate::i18n::trn("io-exact-load-error", &[("format", name), ("error", &crate::i18n::name(&e))])),
        };
        let _ = tx.send(res);
    });
    regen.busy = Some(Busy { started: std::time::Instant::now(), label: crate::i18n::tr1("io-exact-importing", "format", name), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
}

/// AN SVG IS READ ON A WORKER THREAD, as the meshes and the exact formats are: the window does not wait on a large
/// drawing, and a panic in the reading ends that thread with a message instead of the program. A stack overflow is not
/// a panic - it ends the process from any thread - and what stops it is `qymcad_io::SVG_MAX_DEPTH`, which refuses a
/// drawing nested past 1024 before the parse. The thread gets the 8 MB stack the main thread has, so a legal drawing
/// keeps the margin it had there: the XML parser recurses once per level, and a 2 MB thread ran out at 3,466 nested
/// groups, the main thread at 13,866.
pub(crate) fn open_svg(regen: &mut Rebuilding, path: String) {
    let (tx, rx) = std::sync::mpsc::channel();
    let p = path.clone();
    std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(move || {
            let res = match qymcad_io::import_svg(&p) {
                Ok(d) => JobResult::DrawingRead { note: not_read_note(&d.skipped), path: p, curves: d.curves },
                Err(e) => JobResult::Failed(crate::i18n::tr1("pk-svg-error", "error", &crate::i18n::name(&e))),
            };
            let _ = tx.send(res);
        })
        .expect("a thread for the SVG import");
    regen.busy = Some(Busy {
        started: std::time::Instant::now(),
        label: crate::i18n::tr1("io-exact-importing", "format", qymcad_io::Format::Svg.name()),
        rx,
        kind: BgKind::ImportShapes,
        pulse: None,
        quiet: false,
    });
}

/// What a drawing read in the background held and was not read, in the words the import door uses for one read in
/// place ("· Not read: TEXT 1"), or nothing.
pub(crate) fn not_read_note(skipped: &[(String, usize)]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    let list = skipped.iter().map(|(kind, n)| format!("{kind} {n}")).collect::<Vec<_>>().join(", ");
    format!("· {}", crate::i18n::tr1("import-not-read", "list", &list))
}

// --- Exporting 3D (STEP, STL) and sketches (SVG, DXF) ------------------------------------
/// The VISIBLE bodies of the target: for the whole document, every live body; for a component, the
/// bodies of its subtree (itself plus its descendants). "Visible" means the body's own tick
/// (`mesh_visible[idx]`) AND the owner's hierarchical visibility (`component_chain_visible` up to the
/// root) — a hidden part or subassembly does not reach the export.
pub(crate) fn visible_export_bodies(dc: &qymcad_ui_state::DrawCtx, target: ExportTarget) -> Vec<Id> {
    let subtree: Option<std::collections::HashSet<Id>> = match target {
        ExportTarget::Project => None,
        ExportTarget::Component(cid) => {
            let mut s: std::collections::HashSet<Id> = dc.project.descendants(cid).into_iter().collect();
            s.insert(cid);
            Some(s)
        }
    };
    // CRITICAL: the CONSUMED bodies are excluded (the bases eaten by cuts, booleans and modifiers). In
    // 3D they are hidden by a separate filter, `consumed_bodies`, not by `mesh_visible`, so the export
    // used to pull them in TOGETHER with the final body — the bases covered the cuts, and the STEP or
    // STL came out as a solid blank with nothing cut away.
    let consumed = dc.project.consumed_bodies();
    dc.project
        .bodies
        .iter()
        .map(|b| b.id)
        .enumerate()
        .filter(|&(mi, _)| dc.project.bodies.get(mi).is_none_or(|b| b.visible))
        .filter(|&(_, b)| !consumed.contains(&b))
        .filter(|&(_, b)| dc.project.body_owner(b).is_none_or(|o| component_chain_visible(dc.project, o, None)))
        .filter(|&(_, b)| match &subtree {
            None => true,
            Some(s) => dc.project.body_owner(b).is_some_and(|o| s.contains(&o)),
        })
        .map(|(_, b)| b)
        .collect()
}

/// Write a `.qpart` out of the open dialogue — `subproject_of(component)` plus `save_part`. Returns the path.
pub(crate) fn commit_save_part(parts: &mut PartsLibrary, project: &mut Project, tex_graveyard: &mut Vec<egui::TextureHandle>) -> Result<String, String> {
    let d = parts.save.as_ref().ok_or(&crate::i18n::tr("lib-no-dialog"))?;
    let name = d.name.trim().to_string();
    if name.is_empty() {
        return Err(crate::i18n::tr("lib-need-name"));
    }
    let root = crate::parts_library::user_parts_dir().ok_or(&crate::i18n::tr("lib-no-user-dir"))?;
    let cat = d.category.trim().trim_matches('/').trim();
    let dir = if cat.is_empty() { root } else { root.join(cat) };
    std::fs::create_dir_all(&dir).map_err(|e| crate::i18n::tr1("lib-category-error", "error", &e.to_string()))?;
    let stem = crate::parts_library::sanitize_part_stem(&name);
    let path = dir.join(format!("{stem}.qpart"));
    let sub = project.subproject_of(d.component).ok_or(&crate::i18n::tr("lib-no-root"))?;
    let tags: Vec<String> = d.tags.split([',', ';', '\n']).flat_map(|s| s.split_whitespace()).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    let png = d.preview.as_ref().and_then(color_image_to_png);
    let manifest = qymcad_core::part::PartManifest { schema_version: qymcad_core::part::PART_SCHEMA, name, description: d.description.trim().to_string(), tags, author: String::new() };
    qymcad_io::save_part(&sub, &manifest, &[], png.as_deref(), &path.to_string_lossy())?;
    // the catalogue is re-read, so the new part appears in the "My parts" tree at once
    parts.tree = Some(crate::parts_library::load_library_tree());
    // the old thumbnails go to the graveyard (freed later); textures must NOT be dropped in the current frame
    tex_graveyard.extend(parts.thumbs.drain().filter_map(|(_, v)| v));
    Ok(path.to_string_lossy().to_string())
}

/// Commit an undo step once the edit has settled (the pointer is released, nothing is being typed).
/// A drag is coalesced into one step (committed only when it ends).
pub(crate) fn maybe_commit(edits: &mut Edits, place: &mut Placing, project: &Project, set: &Settings, ctx: &egui::Context) {
    // A dimension flying after the cursor (`placing_dim`), and typing the dimensions of a rectangle,
    // polygon or ellipse, are edits NOT YET finished: the offset or the value changes every frame, and
    // committing is not allowed (otherwise undo would step through the intermediate states instead of
    // removing the dimension or the shape as a whole). These states are cleared by any new action of
    // the tool, so they do not get stuck.
    let placing = place.dim.is_some() || place.active();
    // a history begun afresh (a new document) takes its baseline on the next frame: nothing is a step before it
    let busy = ctx.egui_is_using_pointer() || ctx.egui_wants_keyboard_input() || placing || edits.open.is_some() || !edits.ready;
    if busy {
        return;
    }
    let k = doc_key(project);
    if k == edits.committed_key {
        return;
    }
    // THE BOUNDARY HAS BECOME THE ONLY PATH. A per-frame safety net used to stand here, and it was
    // the reason undo knew no names, depended on the state of the mouse and did not exist outside a
    // window. Now the step is created by the operation, and this place only WATCHES: if the document
    // changed outside `App::edit`, some place is editing it around the boundary. The edit is not
    // lost, but the place has to be found and moved onto the boundary rather than swept under a
    // snapshot.
    // THE GUARD DOES NOT BRING THE APPLICATION DOWN. A `debug_assert!(false)` used to stand here, and
    // that was a mistake: every UI path not yet moved onto `App::edit` turned into a panic of the
    // debug build — entering a subassembly, turning the gizmo. A tool that breaks someone's work in
    // order to tell a developer about their own debt is unacceptable. Now the place is RECORDED (once
    // per call site), the edit is kept by a snapshot, work carries on, and the list of offenders is
    // visible in the log and moved over one at a time.
    #[cfg(debug_assertions)]
    {
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        let site: String = bt.lines().filter(|l| l.contains("qymcad::gui")).nth(2).unwrap_or("undo-unknown").trim().to_string();
        if edits.debt.insert(site.clone()) {
            eprintln!("[operation boundary] the document was changed outside App::edit: {site}");
        }
    }
    let snap = snapshot(project);
    let cur = std::mem::replace(&mut edits.baseline, snap);
    edits.undo.push(Step { name: crate::i18n::tr("undo-edit"), snap: cur });
    if edits.undo.len() > set.undo_cap.max(1) {
        edits.undo.remove(0);
    }
    edits.redo.clear();
    edits.committed_key = k;
}

/// THE DOCUMENT PATH HAS BECOME THE CURRENT ONE — AND WENT STRAIGHT INTO THE RECENT LIST.
///
/// One point for both events, deliberately: "opened" or "saved as" and "remember it" are one action,
/// and spread across the call sites, forgetting the second would only be a matter of time. A guard
/// (`recent_files.rs`) keeps a direct assignment to `project_path` out of the working code.
pub(crate) fn set_project_path(project_path: &mut Option<String>, set: &mut Settings, path: String) {
    *project_path = Some(path.clone());
    remember_recent(set, path);
    // A crash hook cannot reach the document, so it keeps the PATHS instead and the next start
    // offers them back. Told here, at the one point where the path can change.
    crate::crash::note_document(project_path.as_deref(), Some(&autosave_path(project_path)));
}

/// OPEN A RECENT FILE. If it is gone, say so and DROP it from the list.
///
/// A dead row must not be left there silently: the list exists so that one lands on the file at the
/// first attempt, and a row where nothing happens on a click reads as a broken program.
pub(crate) fn open_recent(regen: &mut Rebuilding, set: &mut Settings, status: &mut qymcad_ui_state::Notice, path: String) {
    if !std::path::Path::new(&path).exists() {
        *status = Notice::refusal(crate::i18n::tr1("recent-missing", "path", &path));
        forget_recent(set, &path);
        return;
    }
    crate::gui::io_jobs::spawn_project_load(regen, path);
}

/// Save the CURRENT document as a template.
pub(crate) fn save_as_template(project: &Project, status: &mut qymcad_ui_state::Notice, title: &str) {
    match crate::templates::save(project, title) {
        Ok(path) => *status = Notice::info(crate::i18n::tr1("tpl-saved", "path", &path)),
        Err(e) => *status = Notice::error(crate::i18n::tr1("tpl-save-failed", "error", &e)),
    }
}
