//! The egui interface: the operation tree, the tool library, the 2D backplot, the projects.
//!
//! The model is `qymcad_core::model::Project`. A 2D top view (an egui `Painter`) with panning, zooming and
//! contour selection by click.

use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke};
#[allow(unused_imports)] // the child modules reach it through `super::Vec2`
pub(crate) use egui::Vec2;
use egui_phosphor::regular as ph;

use qymcad_kernel::OcctKernel; // THE ONLY kernel implementation, shared with the repro harness
use qymcad_core::geom::{MeshFace, Point2};

use qymcad_core::model::{Id, Project};

pub(crate) use qymcad_i18n::error_words;
pub(crate) use qymcad_ui_state::settings_sections;
pub(crate) use qymcad_ui_state::*;

impl Default for OnDisk {
    /// THE TITLE STARTS AS THE PROGRAM'S NAME, not empty: an empty title bar at launch reads as a window
    /// that failed to open rather than as one with no document.
    fn default() -> Self {
        OnDisk {
            project_path: None,
            dxf_path: None,
            export_folder: None,
            io: DocIo::default(),
            edits: Edits::default(),
            file_ask: None,
            mesh_export: None,
            crash_report: Vec::new(),
            report: crate::gui::report_problem::ReportDraft::default(),
            title_shown: APP_NAME.to_string(),
            pending_nav: None,
        }
    }
}

pub(crate) struct App {
    /// THE DRAWING TOOLS, all fourteen in one record (see `DrawState`). They only ever move together, and
    /// as fourteen fields among a hundred "the tools in hand" was something one had to know rather than read.
    tools: qymcad_ui_state::DrawState,
    /// THE PARAMETERS OF THE FEATURE BEING BUILT, all fourteen in one record (see `FeatParams`).
    params: qymcad_ui_state::FeatParams,
    /// HOW THE SCENE IS BEING LOOKED AT: the camera, the flat view, the mode, and what a turn or a drag
    /// of them needs while it lasts (see `Viewing`).
    viewing: qymcad_ui_state::Viewing,
    /// WHAT IS BEING DRAGGED in the scene, and by which handle (see `Dragged`).
    dragged: qymcad_ui_state::Dragged,
    /// THE TOOLS THAT ARE NOT DRAWING TOOLS: trimming, datums, mates, measuring, the section, the
    /// component array, the clipboard, the sketch array, a rotation and a name being typed (see `SideTools`).
    side: qymcad_ui_state::SideTools,
    /// THE DOCUMENT ON DISK: where it came from, what is unsaved, what is being written (see `OnDisk`).
    disk: OnDisk,
    /// WHAT IS CHOSEN and what is under the cursor (see `Chosen`).
    chosen: qymcad_ui_state::Chosen,
    /// WHICH BODIES OCCUPY THE SAME SPACE, and at which revision that was last worked out.; see [`Interference`].
    interference: Interference,
    /// THE COLOUR SCHEME AND THE SETTINGS WINDOW: the palette in force, the ones to choose from, and what is; see [`SchemeUi`].
    scheme: SchemeUi,
    /// WHAT THE PERSON IS BEING MADE TO WAIT FOR, and since when - so that a wait shorter than an eye blink; see [`Waiting`].
    waiting: Waiting,
    /// THE LIVE GEOMETRY behind the meshes: the kernel's own solids and the faces they were tessellated into.; see [`LiveGeom`].
    live: LiveGeom,
    /// THE STATE OF THE REBUILD: what is pending, what is running in the background, and how far the geometry; see [`Rebuilding`].
    regen: Rebuilding,
    /// THE PARTS LIBRARY: its tree, what is picked in it, and the thumbnails already drawn.; see [`PartsLibrary`].
    parts: PartsLibrary,
    /// THE MODEL TREE AS DRAWN: what is being searched for, what is being dragged, and where the rows landed.; see [`TreeUi`].
    tree: TreeUi,
    /// THE EDGES OF THE SELECTED BODY, ready for picking and for drawing.; see [`EdgeCache`].
    edges: EdgeCache,
    /// What is open on screen; see [`Windows`].
    win: Windows,
    /// Derived state that a frame may rebuild at will; see [`Caches`].
    cache: Caches,
    project: Project,
    /// The splash and its progress. The logo for the splash (its texture is loaded lazily on the first frame).
    logo_tex: Option<egui::TextureHandle>,
    /// The hotkey window's own state while it waits for a press (see `HotkeyCapture`).
    hotkeys: qymcad_ui_state::HotkeyCapture,
    /// THE TEXTS TYPED INTO THE BARS' FIELDS: the field's key mapped to what was typed.
    ///
    /// The values in the state are numbers (`tool_prefs.fillet`, `arr.count` and so on), and text cannot be put
    /// into them. So what was typed lives here and the number is recomputed from it. Without this the bars' fields
    /// would stay `DragValue`s, into which a formula CANNOT be typed: the sketch fillet radius, the offset, the
    /// copy counts and a polygon's number of sides could not be made parametric at all.
    bar_exprs: std::collections::HashMap<&'static str, String>,
    /// ALL THE PROGRAM'S SETTINGS in one record (see `Settings`). The single owner of the values: the settings
    /// window edits it, the store saves it whole, and the program reads from it.
    set: Settings,
    /// THE COMMAND DID NOT APPLY - a flag, not a guess from the status text.
    ///
    /// This used to be decided by searching the status line for a substring. While the interface was in one
    /// language it worked; with a translation the status would stop matching, and a FAILED operation would land
    /// silently in the undo history as a successful one. Checking text is always a lie about what happened: the
    /// text is written for a person, while the decision is taken by the program.
    cmd_failed: bool,
    /// THE SNAP HINT, derived from the cursor and recomputed every frame. It used to live inside the snap settings
    /// and got in the way of saving them whole; it is not a setting.
    snap_hint: Option<(Point2, u8)>,

    status: qymcad_ui_state::Notice,
    /// The cursor's coordinates in the part's frame (for the status line), in the 2D view.
    cursor: Option<Point2>,
    /// what the command points at by a click - an enumeration (see Picking)
    /// an import awaiting placement - one record (see PendingImport)
    /// what is being dragged with the mouse in a sketch - one record (see Dragging)
    /// The parameters window's search: it filters both the variables and the drivers, by name and by path.
    par_search: String,
    /// The graveyard of textures awaiting deferred release. A `TextureHandle` MUST NOT be dropped in a frame whose
    /// texture is still being drawn (wgpu reports "Texture ... has been destroyed" on submit). They are put here and
    /// cleared at the START of the next frame, before any drawing, so the texture is no longer used by anyone.
    tex_graveyard: Vec<egui::TextureHandle>,
    /// THE REBUILD GRAPH: the parameter values as of the last rebuild. When a name's value changes, only those
    /// that refer to it are marked dirty, not the project's whole parametrics.
    params_seen: std::collections::HashMap<String, f64>,
    /// The active workbench (Sketch, Part, Assembly or Machining).
    workbench: Workbench,
    /// the core of the active Part command - one record (see FeatCommand)
    /// The extrude's operation: 0 a new body, 1 join, 2 cut, 3 intersect.
    /// WHAT the current command builds and WHERE - one record instead of three independent fields (see FeatTarget)
    feat: FeatTarget,
    /// the geometry selected for the commands - one record (see GeomSelection)
    /// the sketch working session - one record (see SketchSession)
    sketch_ses: SketchSession,
    /// The active context's path: [the root, ..., the current one]. The breadcrumbs plus drilling in and out.
    active_path: Vec<Id>,
    /// The sketches hidden in 3D (by Id) - the visibility checkbox in the tree (they are visible by default).
    sketch_hidden: std::collections::HashSet<Id>,
    /// the Part tools' options - one record (see FeatOptions)
    opts: FeatOptions,
    /// the interface's deferred intentions - one record (see DeferredUi)
    deferred: DeferredUi,
    /// editing an annotation or a text - one record (see AnnotEdit)
    /// the inline editing of a sketch element - an enumeration (see InlineEdit)
    /// the sketch pattern's parameters - one record (see SketchPattern)
    sk_pat: SketchPattern,
    /// the drawing tools' preferences - one record (see SketchToolPrefs)
    tool_prefs: SketchToolPrefs,
    /// the sketch pattern tool - one record (see PatternTool)
    font_cache: qymcad_ui_state::FontCache,
    /// The GPU viewport is available (the wgpu backend is active and the resources are installed). Otherwise the CPU raster is used.
    gpu_ok: bool,

    /// a running sweep of a degree of freedom (a mate's animation)
    joint_anim: Option<JointAnim>,
}

/// THE STATE OF WRITING AND OPENING A DOCUMENT: what is awaiting a write, what the thread has already confirmed,
/// what to open at start-up. As separate fields, "the deferred request to write" and "the key it will confirm"
/// drifted apart: a failed save marked the project clean, because the key was applied without regard to the fact.
#[derive(Clone, Default)]
pub(crate) struct DocIo {
    /// a write was requested while the previous one was still running (the last one wins)
    pub save_request: Option<(String, bool)>,
    /// the state keys AWAITING the thread's confirmation: applied only on success
    pub saved_key: Option<u64>,
    pub autosave_key: Option<u64>,
    /// the file opened when the application starts
    pub startup: Option<String>,
}

/// HOW LONG A WRITE MUST RUN BEFORE THE WAITING CARD IS SHOWN AT ALL.
///
/// A small document is written faster than an eye can catch, and a card flashing for one frame reads as a
/// glitch rather than an answer. Below this the program simply gets on with it and says nothing.
const SAVE_WAIT_GRACE: std::time::Duration = std::time::Duration::from_millis(120);

/// AND HOW LONG IT STAYS ONCE SHOWN.
///
/// Without a floor the card could still blink: the write ends a moment after the grace has passed. What was
/// shown must be readable, so once up it stays for this long even if the write is already over.
const SAVE_WAIT_MIN: std::time::Duration = std::time::Duration::from_millis(400);

impl App {
    /// THE ONE DOORWAY OF THE WINDOWS.
    pub(crate) fn win_ctx<'a>(&'a mut self, ask: &'a mut Vec<WinAsk>) -> WinCtx<'a> {
        let file_ask_open = self.asking_for_a_file();
        WinCtx {
            cache: &mut self.cache,
            sketch_ses: &self.sketch_ses,
            deferred: &mut self.deferred,
            edits: &mut self.disk.edits,
            gpu_ok: &mut self.gpu_ok,
            logo_tex: &mut self.logo_tex,
            par_search: &mut self.par_search,
            parts: &mut self.parts,
            pending_nav: &mut self.disk.pending_nav,
            project: &mut self.project,
            project_path: &mut self.disk.project_path,
            dxf_path: &mut self.disk.dxf_path,
            regen: &mut self.regen,
            scheme: &mut self.scheme,
            sel: &mut self.chosen.sel,
            set: &mut self.set,
            status: &mut self.status,
            mesh_export: &mut self.disk.mesh_export,
            tex_graveyard: &mut self.tex_graveyard,
            tree: &mut self.tree,
            waiting: &mut self.waiting,
            win: &mut self.win,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            view: &mut self.viewing.view,
            cam: &mut self.viewing.cam,
            active_path: &mut self.active_path,
            file_ask_open,
            hotkeys: &mut self.hotkeys,
            workbench: self.workbench,
            ask,
        }
    }

    /// Carry out what a window asked for, after it has drawn.
    pub(crate) fn do_win_asks(&mut self, asks: Vec<WinAsk>, ctx: &egui::Context) {
        for a in asks {
            match a {
                WinAsk::Nav(n) => self.do_nav(n, ctx),
                WinAsk::NavGuarded(n) => self.request_nav(n, ctx),
                WinAsk::Save => self.save_project(),
                WinAsk::GoTo { owner, sel } => {
                    if let Some(o) = owner {
                        self.enter_component(o);
                    }
                    self.chosen.sel = sel;
                }
                WinAsk::InsertPart(src) => self.insert_part_from(src),
                WinAsk::ExportSettings => self.ask_save_file(rfd::AsyncFileDialog::new().set_file_name("qym-cad-settings.ron").add_filter("qym-cad settings", &["ron"]), |app, p| {
                    let path = p.to_string_lossy().into_owned();
                    app.scheme.note = match crate::gui::export_settings_to(&app.set, &path) {
                        Ok(()) => Notice::info(crate::i18n::tr1("settings-profile-saved", "path", &path)),
                        Err(e) => Notice::error(crate::i18n::tr1("settings-profile-failed", "error", &e)),
                    };
                }),
                WinAsk::ImportSettings => {
                    let ctx = ctx.clone();
                    self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("qym-cad settings", &["ron"]), move |app, p| {
                        let path = p.to_string_lossy().into_owned();
                        app.scheme.note = match app.import_settings_from(&path, &ctx) {
                            Ok(()) => Notice::info(crate::i18n::tr("settings-profile-loaded")),
                            // A BROKEN FILE DOES NOT TOUCH THE CURRENT SETTINGS: it is said that it did not
                            // work, and what was there stays.
                            Err(e) => Notice::error(crate::i18n::tr1("settings-profile-failed", "error", &e)),
                        };
                    });
                }
                WinAsk::ExportMesh(format, target, defl) => self.export_mesh(format, target, defl),
                WinAsk::Delete(sel, dependents) => self.execute_delete(sel, dependents),
                // THE ONE DOOR a button uses. The search must not have a launch path of its own: a second one
                // sooner or later starts doing something other than the button, and nothing says so.
                WinAsk::RunCommand(code) => self.run_command(code, ctx),
            }
        }
    }

    /// THE ONE DOORWAY OF THE BARS.
    pub(crate) fn bar_ctx<'a>(&'a mut self, ask: &'a mut Vec<BarAsk>) -> BarCtx<'a> {
        BarCtx {
            armed: &mut self.tools.armed,
            active_path: &mut self.active_path,
            annot: &mut self.tools.annot,
            bar_exprs: &mut self.bar_exprs,
            boolean: &mut self.params.boolean,
            cam: &mut self.viewing.cam,
            carr: &mut self.side.carr,
            clip: &mut self.side.clip,
            cmd: &mut self.tools.cmd,
            corner: &mut self.tools.corner,
            dim: &mut self.tools.dim,
            drag: &mut self.tools.drag,
            dxf_path: &mut self.disk.dxf_path,
            edits: &mut self.disk.edits,
            feat: &mut self.feat,
            gsel: &mut self.tools.gsel,
            inline: &mut self.tools.inline,
            joint: &mut self.side.joint,
            m3: &mut self.side.m3,
            measure: &mut self.tools.measure,
            mirror: &mut self.params.mirror,
            mode_3d: &mut self.viewing.mode_3d,
            parts: &mut self.parts,
            pat: &mut self.tools.pat,
            pending_import: &mut self.tools.pending_import,
            picking: &mut self.tools.picking,
            place: &mut self.tools.place,
            project: &mut self.project,
            regen: &mut self.regen,
            scheme: &mut self.scheme,
            section: &mut self.side.section,
            sel: &mut self.chosen.sel,
            sel_sk: &mut self.tools.sel_sk,
            set: &mut self.set,
            sk_pat: &mut self.sk_pat,
            sketch_ses: &mut self.sketch_ses,
            status: &mut self.status,
            mesh_export: &mut self.disk.mesh_export,
            tool: &mut self.tools.tool,
            tool_prefs: &mut self.tool_prefs,
            view: &mut self.viewing.view,
            win: &mut self.win,
            workbench: &mut self.workbench,
            ask,
        }
    }

    /// Carry out what a bar asked for, after it has drawn.
    pub(crate) fn do_bar_asks(&mut self, asks: Vec<BarAsk>, ctx: &egui::Context) {
        for a in asks {
            match a {
                BarAsk::Nav(n) => self.request_nav(n, ctx),
                BarAsk::Save => self.save_project(),
                BarAsk::SaveAs => self.save_project_as(),
                BarAsk::Import(want) => self.pick_import(want),
                BarAsk::PickFont => self.font_cache.picker.open = true,
                BarAsk::PickFontFile => self.pick_font(),
                BarAsk::ExportExact(f, t) => self.export_exact(f, t),
                BarAsk::Undo => self.undo(),
                BarAsk::Redo => self.redo(),
                BarAsk::Clipboard { cut } => self.clipboard_copy(cut),
                BarAsk::Paste => self.clipboard_paste(),
                BarAsk::RebuildEverything => self.rebuild_everything(),
                BarAsk::Help(article) => self.open_help(&article),
                BarAsk::GotoPath(i) => self.goto_path_index(i),
                BarAsk::ExitContext => self.exit_context(),
                BarAsk::SketchTool(t) => self.set_sk_tool(t),
                BarAsk::FeatCmd(c) => self.start_feat_cmd(c),
                BarAsk::PrimCmd(c) => crate::gui::commands::start_prim_cmd(&mut self.part_ctx(), c),
                BarAsk::ToggleSection => self.toggle_section(),
                BarAsk::MirrorPart(comp) => mirror_part_door(self, comp),
                BarAsk::ToggleMeasure3d => self.toggle_measure_3d(),
                BarAsk::EnterComponent(cid) => self.enter_component(cid),
                BarAsk::CompArray(m) => self.start_comp_array(m),
                BarAsk::CancelAllTools(then) => {
                    self.cancel_all_tools();
                    then.take(&mut self.params.boolean, &mut self.status)
                }
                BarAsk::ToggleLibrary => toggle_parts_library(&mut self.parts, &mut self.win),
                BarAsk::ToggleSketchPick => self.toggle_sketch_pick(),
                BarAsk::SketchSelectMode => crate::gui::sketching::sketch_select_mode(&mut self.sketch_ctx()),
                BarAsk::Constraint(code) => crate::gui::sketching::constraint_button(&mut self.sketch_ctx(), code),
                BarAsk::JointPick => self.start_joint_pick(),
                BarAsk::GroundPick => self.start_ground_pick(),
                BarAsk::GroupPick => self.start_group_pick(),
                BarAsk::WidthPick => self.start_width_pick(),
                BarAsk::TangentPick => self.start_tangent_pick(),
                BarAsk::RelationPick => self.start_relation_pick(),
            }
        }
    }

    /// THE ONE DOORWAY OF THE FEATURE TREE. The borrows are disjoint by field, so the panel gets exactly
    /// what it touches and nothing else.
    pub(crate) fn tree_ctx<'a>(&'a mut self, ask: &'a mut Vec<TreeAsk>) -> TreeCtx<'a> {
        TreeCtx {
            project: &mut self.project,
            view: &mut self.viewing.view,
            sel: &mut self.chosen.sel,
            tree_sel: &mut self.chosen.tree_sel,
            tree: &mut self.tree,
            rename: &mut self.side.rename,
            clip: &mut self.side.clip,
            status: &mut self.status,
            edits: &mut self.disk.edits,
            regen: &mut self.regen,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            datum: &mut self.side.datum,
            deferred: &mut self.deferred,
            interference: &mut self.interference,
            sketch_hidden: &mut self.sketch_hidden,
            sketch_ses: &mut self.sketch_ses,
            mesh_export: &mut self.disk.mesh_export,
            rollback: &mut self.dragged.rollback,
            workbench: self.workbench,
            set: &mut self.set,
            active_path: &self.active_path,
            cam: &self.viewing.cam,
            scheme: &self.scheme,
            ask,
        }
    }

    /// Carry out what the tree asked for, after it has drawn.
    pub(crate) fn do_tree_asks(&mut self, asks: Vec<TreeAsk>) {
        for a in asks {
            match a {
                TreeAsk::EditFeature(id) => crate::gui::commands::start_feat_cmd_edit(&mut self.part_ctx(), id),
                TreeAsk::EnterSketch(si) => self.enter_sketch_edit(si),
                TreeAsk::SketchAgain(si) => qymcad_part::sketch_row_again(&mut self.part_ctx(), si),
                TreeAsk::Clipboard { cut } => self.clipboard_copy(cut),
                TreeAsk::Paste => self.clipboard_paste(),
                TreeAsk::ExportSketch { si, flat } => self.export_sketch(si, flat),
                TreeAsk::ReplaceSketchPlane(si) => self.start_replace_sketch_plane(si),
                TreeAsk::Action { act, ti, nid, prev_feat, next_feat } => self.tree_action(act, ti, nid, prev_feat, next_feat),
                TreeAsk::RescaleImport(nid) => crate::gui::import_scale::rescale(&mut self.win_ctx(&mut Vec::new()), nid),
                TreeAsk::SketchOnBasePlane(b) => {
                    self.create_sketch_on(qymcad_core::feature::SketchPlane::World(b));
                }
                TreeAsk::EnterComponent(cid) => self.enter_component(cid),
                TreeAsk::EditCompArray(pid) => self.start_comp_array_edit(pid),
                TreeAsk::ImportFile(path) => crate::gui::import_door::import_answer(qymcad_ui_state::Want::Anything)(self, path),
                TreeAsk::ExportExact(f, cid) => self.export_exact(f, ExportTarget::Component(cid)),
                TreeAsk::SavePart(cid) => self.open_save_part_dialog(cid),
                TreeAsk::Drop { dragged, target, how } => {
                    self.tree_apply_drop(dragged, target, how);
                }
                TreeAsk::Resync => crate::gui::commands::resync_after_topology_change(&mut self.part_ctx()),
            }
        }
    }

    pub(crate) fn props_ctx<'a>(&'a mut self, ask: &'a mut Vec<PropsAsk>) -> PropsCtx<'a> {
        PropsCtx {
            project: &mut self.project,
            datum: &mut self.side.datum,
            deferred: &mut self.deferred,
            hover: &mut self.chosen.hover,
            joint: &mut self.side.joint,
            picking: &mut self.tools.picking,
            sel_conn: &mut self.chosen.sel_conn,
            status: &mut self.status,
            edits: &mut self.disk.edits,
            regen: &mut self.regen,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            sel: &mut self.chosen.sel,
            array: &mut self.side.array,
            boolean: &mut self.params.boolean,
            opts: &mut self.opts,
            cache: &mut self.cache,
            win: &mut self.win,
            view: &mut self.viewing.view,
            gsel: &mut self.tools.gsel,
            sketch_ses: &mut self.sketch_ses,
            set: &mut self.set,
            active_path: &self.active_path,
            cam: &self.viewing.cam,
            scheme: &self.scheme,
            workbench: self.workbench,
            mode_3d: &mut self.viewing.mode_3d,
            ask,
        }
    }

    /// Carry out what the properties panel asked for, after it has drawn.
    pub(crate) fn do_props_asks(&mut self, asks: Vec<PropsAsk>) {
        crate::gui::commands::do_props_asks(self, asks)
    }

    /// Is the cursor on the face arrow? The threshold matches the body gizmo's, in pixels along the segment.
    /// The gizmo mode of the selected component: grounded gives None; driven by a joint that has a freedom
    /// gives Joint(jid); otherwise (free, or a seed with no freedoms of its own) gives Free, the plain 6-DOF one.
    fn comp_gizmo_mode(&self, comp: Id) -> CompGizmoMode {
        qymcad_pick::comp_gizmo_mode(&self.painting(), comp)
    }

    /// The geometry of the section GIZMO: the centre of the quad on the plane, u, v, the half-size, and the arrow's tip.
    fn section_gizmo_geom(&self) -> Option<qymcad_ui_state::SectionGizmo> {
        section_gizmo_geom(&self.painting())
    }

    /// The one doorway into drawing. Shared throughout, so it costs nothing to build and can be taken from
    /// a `&self` method - which every drawing method is.
    pub(crate) fn painting(&self) -> Painting<'_> {
        Painting {
            armed: &self.tools.armed,
            active_path: &self.active_path,
            arr: self.params.arr,
            body_giz: &self.dragged.body_giz,
            cache: &self.cache,
            cam: self.viewing.cam,
            carr: &self.side.carr,
            clip: &self.side.clip,
            cmd: &self.tools.cmd,
            comp_giz: self.dragged.comp_giz,
            cursor: self.cursor,
            datum: &self.side.datum,
            draft: self.params.draft,
            chamfer: self.params.chamfer,
            edges: &self.edges,
            handle_drag: self.dragged.handle_drag,
            feat: self.feat,
            gpu_ok: self.gpu_ok,
            gsel: &self.tools.gsel,
            hole: self.params.hole,
            hover: self.chosen.hover,
            inline: self.tools.inline,
            interference: &self.interference,
            joint: &self.side.joint,
            live: &self.live,
            loft: &self.params.loft,
            m3: &self.side.m3,
            mirror: &self.params.mirror,
            mode_3d: self.viewing.mode_3d,
            pat: self.tools.pat,
            pending_import: &self.tools.pending_import,
            picking: self.tools.picking,
            prim: self.params.prim,
            project: &self.project,
            regen: &self.regen,
            repl_surface: self.params.repl_surface,
            rev: self.params.rev,
            rot: &self.side.rot,
            scheme: &self.scheme,
            section: &self.side.section,
            snap_hint: self.snap_hint,
            sel: self.chosen.sel,
            sel_sk: &self.tools.sel_sk,
            set: &self.set,
            sk_pat: self.sk_pat,
            sketch_hidden: &self.sketch_hidden,
            sketch_ses: self.sketch_ses,
            split: &self.params.split,
            boolean: &self.params.boolean,
            stitch_parts: &self.params.stitch_parts,
            recognise: &self.params.recognise,
            sweep: self.params.sweep,
            thread: self.params.thread,
            tool: &self.tools.tool,
            tool_prefs: &self.tool_prefs,
            trim: &self.side.trim,
            view: self.viewing.view,
            view_dragging: self.viewing.view_dragging,
            win: &self.win,
            workbench: self.workbench,
        }
    }

    /// The one doorway into the sketch workbench: built in a single place so the borrows stay disjoint.
    pub(crate) fn sketch_ctx(&mut self) -> SketchCtx<'_> {
        SketchCtx {
            armed: &mut self.tools.armed,
            project: &mut self.project,
            regen: &mut self.regen,
            status: &mut self.status,
            place: &mut self.tools.place,
            cmd: &mut self.tools.cmd,
            tool: &mut self.tools.tool,
            sel: &mut self.chosen.sel,
            sel_sk: &mut self.tools.sel_sk,
            drag: &mut self.tools.drag,
            edits: &mut self.disk.edits,
            dim: &mut self.tools.dim,
            pending_import: &mut self.tools.pending_import,
            gsel: &mut self.tools.gsel,
            measure: &mut self.tools.measure,
            inline: &mut self.tools.inline,
            pat: &mut self.tools.pat,
            picking: &mut self.tools.picking,
            corner: &mut self.tools.corner,
            annot: &mut self.tools.annot,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            view: &mut self.viewing.view,
            cursor: &mut self.cursor,
            cache: &self.cache,
            cam: &self.viewing.cam,
            set: &self.set,
            scheme: &self.scheme,
            active_path: &self.active_path,
            sk_pat: &mut self.sk_pat,
            tool_prefs: &mut self.tool_prefs,
            sketch_ses: &mut self.sketch_ses,
            font_cache: &mut self.font_cache,
            mode_3d: &mut self.viewing.mode_3d,
            snap_hint: &mut self.snap_hint,
            workbench: &mut self.workbench,
            body_giz: &mut self.dragged.body_giz,
            tree_sel: &mut self.chosen.tree_sel,
            trim: &mut self.side.trim,
            clip: &mut self.side.clip,
            loft: &mut self.params.loft,
            rev: &mut self.params.rev,
            rot: &mut self.side.rot,
            sweep: &mut self.params.sweep,
        }
    }

    /// The one doorway into the Part workbench: built in a single place so the borrows stay disjoint.
    pub(crate) fn part_ctx(&mut self) -> PartCtx<'_> {
        PartCtx {
            armed: &mut self.tools.armed,
            status: &mut self.status,
            cmd: &mut self.tools.cmd,
            project: &mut self.project,
            gsel: &mut self.tools.gsel,
            sel: &mut self.chosen.sel,
            split: &mut self.params.split,
            opts: &mut self.opts,
            feat: &mut self.feat,
            mirror: &mut self.params.mirror,
            loft: &mut self.params.loft,
            chamfer: &mut self.params.chamfer,
            arr: &mut self.params.arr,
            datum: &mut self.side.datum,
            stitch_parts: &mut self.params.stitch_parts,
            recognise: &mut self.params.recognise,
            trim: &mut self.side.trim,
            prim: &mut self.params.prim,
            draft: &mut self.params.draft,
            hole: &mut self.params.hole,
            sweep: &mut self.params.sweep,
            repl_surface: &mut self.params.repl_surface,
            thread: &mut self.params.thread,
            edges: &mut self.edges,
            edits: &mut self.disk.edits,
            live: &mut self.live,
            regen: &mut self.regen,
            params_seen: &mut self.params_seen,
            boolean: &mut self.params.boolean,
            bar_exprs: &mut self.bar_exprs,
            rev: &mut self.params.rev,
            cmd_failed: &mut self.cmd_failed,
            carr: &mut self.side.carr,
            view: &mut self.viewing.view,
            view_restore: &mut self.viewing.view_restore,
            cam: &mut self.viewing.cam,
            picking: &mut self.tools.picking,
            set: &self.set,
            active_path: &self.active_path,
            scheme: &self.scheme,
            comp_giz: &self.dragged.comp_giz,
            mode_3d: &mut self.viewing.mode_3d,
            body_giz: &mut self.dragged.body_giz,
            joint: &self.side.joint,
            workbench: self.workbench,
            cache: &mut self.cache,
            sketch_ses: &mut self.sketch_ses,
            win: &mut self.win,
        }
    }
    /// The one doorway into the rebuild: built in a single place so the borrows stay disjoint.
    pub(crate) fn rebuild_ctx(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: &mut self.project,
            view: &mut self.viewing.view,
            edits: &mut self.disk.edits,
            regen: &mut self.regen,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            status: &mut self.status,
            sel: &mut self.chosen.sel,
            active_path: &self.active_path,
            cam: &self.viewing.cam,
            scheme: &self.scheme,
            set: &self.set,
            sketch_ses: &self.sketch_ses,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        let mut project = Project::default();
        project.new_document(); // the root assembly plus an active empty first Part right from the start
        let mut app = Self {
            set: Settings::default(),
            cmd_failed: false,
            snap_hint: None,
            cache: Caches::default(),
            interference: Interference::default(),
            scheme: SchemeUi::default(),
            waiting: Waiting::default(),
            live: LiveGeom::default(),
            regen: Rebuilding::default(),
            parts: PartsLibrary::default(),
            tree: TreeUi::default(),
            edges: EdgeCache::default(),
            // The start screen is open at launch; the constraint glyphs are on. Built and then opened, because
            // the open set is private - one door in, so nobody can invent a second way of saying "it is open".
            win: {
                let mut w = Windows::default();
                w.help.article = "index".into();
                w.constraints = true;
                w.open(WinKind::Start);
                w
            },
            tools: qymcad_ui_state::DrawState::default(),
            params: qymcad_ui_state::FeatParams::default(),
            viewing: qymcad_ui_state::Viewing::default(),
            dragged: qymcad_ui_state::Dragged::default(),
            side: qymcad_ui_state::SideTools::default(),
            disk: OnDisk::default(),
            chosen: qymcad_ui_state::Chosen::default(),
            project,
            logo_tex: None,
            hotkeys: qymcad_ui_state::HotkeyCapture::default(),
            bar_exprs: std::collections::HashMap::new(),
            status: Notice::info(crate::i18n::tr("g-start-hint")),
            cursor: None,
            // THE "ANGLE" FIELD IS EMPTY AT START-UP. It used to hold 90 deg, and that silently became a
            // REQUIREMENT: `joint_pick_anchor_click` writes a non-zero angle into `drive[0]` of the joint
            // being created. Every joint was born demanding a 90 deg turn that nobody had asked for —
            // against the contract stated in that same place ("a joint is born with its degrees of
            // freedom free"). On a slider that pinned a degree it does not even have; a gear relation
            // became unsatisfiable, the solve did not converge, and the WHOLE mechanism froze.
            par_search: String::new(),
            tex_graveyard: Vec::new(),
            params_seen: std::collections::HashMap::new(),
            workbench: Workbench::Part,
            feat: FeatTarget::default(),
            sketch_ses: SketchSession::default(),
            active_path: Vec::new(),
            sketch_hidden: std::collections::HashSet::new(),
            opts: FeatOptions { mirror_keep: true, ..Default::default() },
            deferred: DeferredUi::default(),
            sk_pat: SketchPattern { dx: 20.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 20.0, count2: 1, angle: 360.0 },
            tool_prefs: SketchToolPrefs { poly_n: 6, fillet: 3.0, offset: 3.0, text_h: 10.0, ..Default::default() },
            font_cache: qymcad_ui_state::FontCache::default(),
            gpu_ok: false,
            joint_anim: None,
        };
        // THE GUARD'S BASELINE COMES FROM THE DOCUMENT ITSELF, not from zero. With zero the very first
        // frame saw "the key does not match" and declared a fresh empty document changed outside an
        // operation.
        app.disk.edits.committed_key = doc_key(&app.project);
        app.disk.edits.saved_key = edit_key(&app.draw_ctx());
        app
    }
}

impl App {
    /// Load the imported curves (DXF or SVG) and switch to picking the PLACEMENT plane — a click on a
    /// plane (XY/XZ/YZ), on a datum, or on a face of a part in the viewport builds an EDITABLE sketch
    /// out of these curves in the active context (the assembly, or the part if the import was started
    /// inside one). The original file is embedded (for re-importing and for comparison). Esc cancels.
    fn arm_sketch_import(&mut self, curves: Vec<qymcad_core::geom::ProfEdge>, path: &str) {
        if curves.is_empty() {
            self.status = Notice::refusal(crate::i18n::tr("g-import-empty"));
            return;
        }
        let n = curves.len();
        let src = embed_source(&mut self.project, path);
        self.cancel_all_tools(); // drop the other picking modes and commands, so clicks do not conflict
        self.viewing.mode_3d = true; // placement planes and faces are picked in 3D (like picking a sketch plane)
        self.chosen.sel = Sel::None;
        self.tools.pending_import.curves = Some((curves, src, file_name(path)));
        self.disk.dxf_path = Some(path.to_string());
        self.status = Notice::info(crate::i18n::tr1("g-import-place", "n", &n.to_string()));
    }

    /// Place the waiting import on the chosen plane: build an editable sketch in the active context and
    /// enter its editing (visible and editable straight away — it can be drawn onto and dimensioned).
    fn place_pending_import(&mut self, plane: qymcad_core::feature::SketchPlane) {
        let Some((curves, source, name)) = self.tools.pending_import.curves.take() else { return };
        let plane = resolve_placement_plane(&mut self.tools.cmd, &mut self.project, &mut self.status, plane);
        let si = self.project.import_sketch(name, curves, source, plane);
        let ents = self.project.sketches[si].entities.len();
        invalidate(&mut self.regen);
        self.viewing.view.initialized = false;
        self.enter_sketch_edit(si);
        self.status = Notice::info(crate::i18n::tr1("g-sketch-imported", "n", &ents.to_string()));
    }

    /// A new empty document (geometry and operations are reset; the global tool and machine libraries
    /// are kept, they live outside the document).
    /// A NEW DOCUMENT FROM A TEMPLATE.
    ///
    /// A template is read as an ordinary document — which is what it is — but its path is NOT
    /// REMEMBERED. Otherwise the very first Save would write the work over the template, and both would
    /// be lost at once: the template, and the confidence that files stay where they were left.
    ///
    /// The creation date is cleared too: the document is created NOW, not when the template was saved.
    pub(crate) fn new_from_template(&mut self, path: &str) {
        match qymcad_io::load_project(path) {
            Ok(mut project) => {
                project.ensure_document();
                project.meta.created.clear();
                self.finish_project_load(path.to_string(), project, Vec::new());
                // ...and forget straight away where it came from: this is a new document, not an opened file
                self.disk.project_path = None;
                self.set.recent.retain(|p| p != path); // a template is not a "recent file"
                self.disk.edits.saved_key = edit_key(&self.draw_ctx()); // a blank slate: no edits have been made yet
                self.status = Notice::info(crate::i18n::tr1("tpl-new-from", "name", &std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()));
            }
            Err(e) => self.status = Notice::error(crate::i18n::tr1("tpl-open-failed", "error", &e.to_string())),
        }
    }

    pub(crate) fn new_project(&mut self) {
        self.project = Project::default();
        self.project.new_document();
        self.live.shapes.clear();
        self.chosen.sel = Sel::None;
        self.disk.project_path = None;
        self.disk.dxf_path = None;
        self.tools.pending_import.draw_pts = None;
        invalidate(&mut self.regen);
        self.viewing.view.initialized = false;
        self.viewing.cam.init = false;
        self.disk.edits.fresh(); // a clean start: no steps of the document before, the baseline taken on the next frame
        self.status = Notice::info(crate::i18n::tr("g-new-project"));
    }

    /// A NEW ASSEMBLY DOCUMENT: the root without an empty part, with the root active.
    ///
    /// "Create a part" and "create an assembly" are different intents, and starting an assembly with an
    /// empty part that then has to be deleted means making a person clean up after the program.
    pub(crate) fn new_assembly_project(&mut self) {
        self.new_project();
        // `new_document` created a part under the root — an assembly document does not need it
        let root = self.project.root;
        let parts: Vec<Id> = self.project.components.iter().filter(|c| c.parent == Some(root)).map(|c| c.id).collect();
        for id in parts {
            self.project.delete_component(id); // ask_delete-exempt: this is not deleting anyone's work but cleaning up while creating the document
        }
        self.project.set_active_component(Some(root));
        self.disk.edits.saved_key = edit_key(&self.draw_ctx()); // a clean start — there are no edits
        self.status = Notice::info(crate::i18n::tr("g-new-assembly"));
    }

    /// Request navigation that could lose edits: with unsaved work it asks first, otherwise it goes ahead.
    fn request_nav(&mut self, nav: Nav, ctx: &egui::Context) {
        if qymcad_ui_state::is_dirty(&mut self.rebuild_ctx()) {
            self.deferred.nav = Some(nav);
        } else {
            self.do_nav(nav, ctx);
        }
    }

    /// Carry out the navigation (there are no edits, or it was confirmed in the dialogue).
    fn do_nav(&mut self, nav: Nav, ctx: &egui::Context) {
        match nav {
            Nav::New => {
                self.new_project();
                self.sync_workbench()
            } // the context follows the new document, as on opening a file, before its clean mark is taken
            Nav::NewAssembly => self.new_assembly_project(),
            Nav::OpenDialog => {
                self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("QymCAD", &["qcad", "ron"]), |app, p| {
                    crate::gui::io_jobs::spawn_project_load(&mut app.regen, p.to_string_lossy().into_owned());
                });
            }
            Nav::NewFromTemplate => self.ask_open_file(crate::templates::chooser(), |app, p| app.new_from_template(&p.to_string_lossy())),
            Nav::OpenPath(path) => open_recent(&mut self.regen, &mut self.set, &mut self.status, path),
            Nav::Exit => {
                self.wait_bg(); // an unfinished background write must reach the disk before exiting
                self.disk.edits.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Enter the editing mode of sketch `si` (the usual "open the sketch").
    pub(crate) fn enter_sketch_edit(&mut self, si: usize) {
        // Going from one sketch straight to another (a double click on a different sketch in the tree):
        // FINISH the current one first, so that its level comes off the `nav_stash` stack. Otherwise the
        // pushed mode_3d=false piles up and the flat view is restored on exit — 3D got stuck unrotatable
        // until the isometric button was pressed.
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        self.cancel_all_tools(); // entering a sketch CANCELS an active datum or Part command (two tools are never held at once)
                                 // the origin is a real fixed point from the very start: always selectable (for a coincidence or
                                 // a dimension from the origin), always grounding the sketch. It is created once.
        self.project.ensure_origin(si);
        if let Some(s) = self.project.sketches.get(si) {
            // remember the viewpoint and mode, to bring them back on leaving the sketch (the drill-in stack)
            self.viewing.nav_stash.push((self.viewing.cam, self.viewing.view, self.viewing.mode_3d));
            qymcad_ui_state::open_sketch_session(&mut self.sketch_ses, &self.project, s.id);
            self.workbench = Workbench::Sketch;
            self.viewing.mode_3d = false;
            self.chosen.sel = Sel::Sketch(si);
            self.tools.sel_sk.clear(); // the selection, and whatever was waiting on it
            self.tools.pending_import.draw_pts = None;
            self.tools.armed = qymcad_ui_state::Armed::None;
            self.viewing.view.initialized = false; // fit the view to the sketch
            self.status = Notice::info(sketch_entry_status(&self.project, s));
        }
    }

    /// Pick a sketch drawing tool (entering a sketch first, if not in one yet).
    pub(crate) fn set_sk_tool(&mut self, t: u8) {
        if edit_si(&self.project, &self.sketch_ses).is_none() {
            self.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        }
        qymcad_ui_state::take_drawing_tool(&mut qymcad_ui_state::tools_of!(self), t);
        self.viewing.mode_3d = false;
        self.status = match self.tools.armed.draw_kind() {
            1 => Notice::info(crate::i18n::tr("g-line-hint")),
            2 => Notice::info(crate::i18n::tr("g-rect-hint")),
            3 => Notice::info(crate::i18n::tr("g-circle-hint")),
            4 => Notice::info(crate::i18n::tr("g-arc-cse")),
            5 => Notice::info(crate::i18n::tr("g-point-hint")),
            6 => Notice::info(crate::i18n::tr("g-polygon-hint")),
            7 => Notice::info(crate::i18n::tr("g-slot-hint")),
            8 => Notice::info(crate::i18n::tr("g-ellipse-hint")),
            9 => Notice::info(crate::i18n::tr("g-spline-hint")),
            10 => Notice::info(crate::i18n::tr("g-circle-3pt")),
            11 => Notice::info(crate::i18n::tr("g-text-hint")),
            _ => Notice::info(crate::i18n::tr("g-select")),
        };
    }

    /// Create a sketch on a given plane (World, Datum or Face) and enter its editing.
    fn create_sketch_on(&mut self, plane: qymcad_core::feature::SketchPlane) -> usize {
        qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-new-sketch")); // a new sketch is one step of undo of its own
        if let Some(part) = self.project.part_to_draw_in() {
            self.enter_component(part);
        } // in an assembly a sketch makes its part
        let plane = resolve_placement_plane(&mut self.tools.cmd, &mut self.project, &mut self.status, plane);
        let si = self.project.new_sketch(crate::i18n::tr1("g-sketch-n", "n", &(self.project.sketches.len() + 1).to_string()));
        self.project.sketches[si].plane = plane;
        let (sid, name) = (self.project.sketches[si].id, crate::i18n::name(&self.project.sketches[si].name));
        self.project.add_sketch_node(sid, name); // a sketch is a node of the timeline (owned by the active part)
        self.enter_sketch_edit(si);
        qymcad_ui_state::close_edit(&mut self.disk.edits, &self.project);
        si
    }

    /// Enter a component (drill in): remember the camera, go one level down, fit the view.
    pub(crate) fn enter_component(&mut self, cid: Id) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        ensure_active_path(&mut self.active_path, &mut self.project);
        if current_ctx_id(&self.active_path, &self.project) == cid || !self.project.components.iter().any(|c| c.id == cid) {
            return;
        }
        self.cancel_all_tools(); // entering a component drops the active tool (nothing is carried in from the parent context)
        self.tree.search.clear(); // and the search query: it was about the PREVIOUS context, here it lies
        self.viewing.nav_stash.push((self.viewing.cam, self.viewing.view, self.viewing.mode_3d));
        self.active_path = qymcad_ui_state::path_into(&self.active_path, &self.project, cid); // a clone lands in its original
        self.chosen.sel = Sel::None;
        self.viewing.view.initialized = false;
        self.viewing.cam.init = false;
        self.sync_workbench();
        self.status = Notice::info(crate::gui::look_at_point::context_is(&self.project, &self.active_path));
    }

    /// Go one level up (drill out): restore the camera.
    pub(crate) fn exit_context(&mut self) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
            return;
        }
        if self.active_path.len() <= 1 {
            return;
        }
        self.cancel_all_tools(); // leaving a component drops the active tool
        self.tree.search.clear(); // the query was about the context being left — outside it searches for the wrong thing
        self.active_path.pop();
        if let Some((cam, view, mode_3d)) = self.viewing.nav_stash.pop() {
            self.viewing.cam = cam;
            self.viewing.cam.init = true;
            self.viewing.view = view;
            self.viewing.mode_3d = mode_3d;
        }
        self.chosen.sel = Sel::None;
        self.sync_workbench();
    }

    /// Jump to an arbitrary context (the "make active" click in the tree): the path is built from the
    /// chain of ancestors. A jump does not restore the intermediate viewpoints — the view is fitted anew.
    fn set_context_to(&mut self, cid: Id) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        self.project.ensure_root();
        self.active_path = qymcad_ui_state::context_path_to(&self.project, cid);
        self.viewing.nav_stash.clear();
        self.chosen.sel = Sel::None;
        self.viewing.view.initialized = false;
        self.viewing.cam.init = false;
        self.sync_workbench();
    }

    /// Go to level `i` of the path (a click on a breadcrumb), leaving one level at a time.
    fn goto_path_index(&mut self, i: usize) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        while self.active_path.len() > i + 1 {
            self.exit_context();
        }
    }

    /// READ THE SETTINGS FROM A FILE and adopt them.
    ///
    /// Missing fields fall back to the factory ones (`serde(default)` on the record): a profile taken
    /// from a different version of the program must still read, not be rejected wholesale — otherwise it
    /// cannot be shared. A broken file does NOT touch the current settings: the change arrives whole or
    /// not at all.
    pub(crate) fn import_settings_from(&mut self, path: &str, ctx: &egui::Context) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let s: Settings = ron::from_str(&text).map_err(|e| e.to_string())?;
        adopt_settings(&mut self.regen, &mut self.scheme, &mut self.set, &mut self.status, s, ctx);
        Ok(())
    }

    /// After a DATUM is edited (its coordinates or its definition): datums are resolved unconditionally
    /// during regeneration, but their consumers (bodies on sketches that sit on a datum plane, axes
    /// through points) have to be rebuilt, otherwise they stay where they were.
    /// A FORCED regeneration of the whole document used to stand here, on the grounds that datums are
    /// rarely edited — on an assembly of a thousand bodies that is the same tens of seconds of freeze.
    /// Now only the nodes that depend on datums are marked dirty.
    fn regen_after_datum_change(&mut self) {
        crate::gui::io_jobs::ensure_brep(&mut self.rebuild_ctx()); // the kernel rebuilds the consumers' geometry, so a live B-rep is needed
        self.project.mark_datum_consumers_dirty();
        qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
    }

    /// Apply or update the pattern on Enter.
    fn confirm_pattern(&mut self) {
        begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("g-sketch-array")); // THE OPERATION BOUNDARY
        let Sel::Sketch(si) = self.chosen.sel else { return };
        if let Some(pi) = self.tools.pat.edit {
            let src = self.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default();
            let kind = current_pattern_kind(&self.tools.armed, self.tools.pat, &self.project, self.sk_pat, si, &src);
            self.project.update_pattern(si, pi, kind);
            self.status = Notice::info(crate::i18n::tr("g-array-updated"));
        } else {
            let eids: Vec<Id> = self.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
            if eids.is_empty() {
                self.status = Notice::info(crate::i18n::tr("g-select-for-array"));
                return;
            }
            let kind = current_pattern_kind(&self.tools.armed, self.tools.pat, &self.project, self.sk_pat, si, &eids);
            self.project.add_pattern(si, &eids, kind);
            self.status = Notice::info(crate::i18n::tr("g-array-created"));
        }
        self.tools.armed = qymcad_ui_state::Armed::None;
        self.tools.pat.edit = None;
        self.tools.pat.center = None;
        invalidate(&mut self.regen);
        qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
    }

    pub(crate) fn clipboard_copy(&mut self, cut: bool) {
        // while editing a sketch, the GEOMETRY is copied, whatever is selected in the tree
        if edit_si(&self.project, &self.sketch_ses).is_some() {
            let eids: Vec<Id> = self.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
            if eids.is_empty() {
                // A REFUSAL COSTS NOTHING. Nothing is selected, so there is no copy to make - and the tool
                // stays in hand: taking a half-drawn shape away over a mis-press would punish the person for
                // pressing the wrong key.
                self.status = Notice::info(crate::i18n::tr("g-pick-for-copy"));
                return;
            }
            // THE TOOL GOES DOWN BEFORE THE COPY IS ARMED.
            //
            // Reported behaviour: copying with a tool active does not cancel the tool. Both then wait for
            // the same click - the copy for its base point, the tool for its next vertex - and whichever
            // gets it, the person did not ask for that; a half-built shape is left on the canvas with
            // nothing to finish it. Copying acts on what is SELECTED, so what is being BUILT has no part in
            // it.
            exit_draw_tools(&mut qymcad_ui_state::tools_of!(self));
            self.side.clip.arm_copy(eids, cut);
            self.status = Notice::info(if cut { crate::i18n::tr("g-cut-base-point") } else { crate::i18n::tr("g-copy-base-point") });
            return;
        }
        // outside a sketch, Ctrl+C acts on THE TREE (a part, a subassembly, a sketch node).
        // A call to ITSELF used to stand here — endless recursion, that is, a stack overflow on the very
        // first Ctrl+C outside sketch mode. A trace of a mechanical rename of the methods.
        self.tree_clipboard_copy(cut);
    }

    /// Ctrl+V. While editing a sketch with a non-empty geometry clipboard, this enters the placing mode
    /// (a ghost, then a click). Otherwise it pastes a tree node from the clipboard.
    fn clipboard_paste(&mut self) {
        if qymcad_sketch::arm_paste(&mut self.sketch_ctx()) {
            return;
        }
        self.tree_clipboard_paste(); // outside a sketch, paste into the tree (see `clipboard_copy`)
    }

    /// A bulk move or copy of components from the multiple-selection clipboard into a target assembly.
    /// Every node goes through the SINGLE core method `reparent_component` or `clone_component` (by
    /// design, the UI only delegates).
    fn paste_components_multi(&mut self, ids: &[Id], cut: bool) {
        use qymcad_core::feature::ComponentKind;
        let root = self.project.root;
        // the target is the selected subassembly; if a Part is selected, its parent assembly; otherwise the active context
        let target = match self.chosen.sel {
            Sel::Component(ci) => match self.project.components.get(ci) {
                Some(c) if self.project.component_kind(c.id) == Some(ComponentKind::Assembly) => c.id,
                Some(c) => c.parent.unwrap_or(root),
                None => self.project.active_ctx(),
            },
            _ => self.project.active_ctx(),
        };
        if self.project.component_is_part(target) {
            self.status = Notice::refusal(crate::i18n::tr("g-paste-needs-assembly"));
            return;
        }
        // Only the ROOTS of the selection: if the set holds both a subassembly and one of its
        // descendants, the descendant travels with the subassembly, as in a file manager. Otherwise the
        // reparent loop would pull the nested one out, and clone would duplicate it. This way the tree is
        // rebuilt predictably, with no orphans and no duplicates.
        let roots: Vec<Id> = ids.iter().copied().filter(|&id| !ids.iter().any(|&o| o != id && self.project.component_is_within(id, o))).collect();
        let (mut ok, mut fail) = (0u32, 0u32);
        if cut {
            for &id in &roots {
                if self.project.reparent_component(id, target) {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            self.side.clip.tree_multi = None;
            self.chosen.tree_sel.multi.clear();
            invalidate(&mut self.regen);
        } else {
            for &id in &roots {
                if self.project.clone_component(id, target).is_some() {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
        }
        self.status = if fail == 0 {
            Notice::info(crate::i18n::tr2("g-paste-result", "what", &if cut { crate::i18n::tr("g-body-moved") } else { crate::i18n::tr("g-copied") }, "n", &ok.to_string()))
        } else {
            Notice::warning(crate::i18n::tr2("g-paste-partial", "ok", &ok.to_string(), "fail", &fail.to_string()))
        };
    }

    /// A test facade for "enter a component" (a double click on a part in the tree).
    #[cfg(test)]
    pub(crate) fn enter_ctx_for_test(&mut self, id: Id) {
        self.active_path = vec![self.project.root, id];
        self.project.set_active_component(Some(id));
        self.sync_workbench();
    }

    /// Test facades for the "save?" question: a test must pull the same handles as the menu and the dialogue.
    #[cfg(test)]
    pub(crate) fn open_for_test(&mut self, path: String) {
        crate::gui::io_jobs::spawn_project_load(&mut self.regen, path);
        // READING A FILE GOES THROUGH THE MODAL QUEUE (`self.regen.busy`), not the background one (`self.regen.bg`).
        // Only `wait_bg` used to stand here, and it waited on an EMPTY queue: nobody took the result of
        // the read, the document stayed FACTORY EMPTY, and tests of the form "opened a file and..."
        // checked emptiness and therefore always passed.
        self.drain_busy_for_test();
        self.wait_bg();
        qymcad_ui_state::rebuild_if_dirty(&mut self.rebuild_ctx());
    }

    /// A facade: carry a BACKGROUND job (a rebuild) through to the end, as a frame of the program does.
    #[cfg(test)]
    /// Wait for BACKGROUND work with no overlay (saving, loading in the B-rep) — in a live program a
    /// frame picks it up. A test cannot "wait for a frame", and without this the written file does not
    /// exist yet.
    #[cfg(test)]
    pub(crate) fn drain_bg_for_test(&mut self) {
        for _ in 0..8 {
            if self.regen.bg.is_empty() {
                break;
            }
            let jobs: Vec<_> = std::mem::take(&mut self.regen.bg);
            for b in jobs {
                if let Ok(res) = b.rx.recv_timeout(std::time::Duration::from_secs(120)) {
                    self.apply_job_result(res);
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn drain_busy_for_test(&mut self) {
        for _ in 0..8 {
            let Some(busy) = self.regen.busy.take() else { break };
            match busy.rx.recv_timeout(std::time::Duration::from_secs(120)) {
                Ok(res) => self.apply_job_result(res),
                Err(_) => break,
            }
        }
    }

    /// The joints panel in a single call — the door for checks that look AT THE FRAME.
    #[cfg(test)]
    pub(crate) fn joints_panel_for_test(&mut self, ui: &mut egui::Ui) {
        let mut asks = Vec::new();
        crate::gui::panels_props::joints_panel(&mut self.props_ctx(&mut asks), ui);
        self.do_props_asks(asks);
    }

    /// THE BUILD TREE in a single call — the same door for checks that look at the frame.
    #[cfg(test)]
    pub(crate) fn build_tree_for_test(&mut self, ui: &mut egui::Ui) {
        let mut asks = Vec::new();
        crate::gui::panels_tree::build_tree(&mut self.tree_ctx(&mut asks), ui);
        self.do_tree_asks(asks);
    }

    #[cfg(test)]
    pub(crate) fn request_nav_for_test(&mut self, nav: Nav) {
        let ctx = egui::Context::default();
        self.request_nav(nav, &ctx);
    }

    /// Test facades for saving: a test must take the same path as Save does.
    #[cfg(test)]
    pub(crate) fn save_for_test(&mut self, path: String) {
        crate::gui::io_jobs::spawn_save(&mut self.disk.io, &mut self.live, &mut self.project, &mut self.regen, &mut self.status, path, false);
        self.wait_bg();
    }

    #[cfg(test)]
    pub(crate) fn autosave_for_test(&mut self, path: String) {
        crate::gui::io_jobs::spawn_save(&mut self.disk.io, &mut self.live, &mut self.project, &mut self.regen, &mut self.status, path, true);
        self.wait_bg();
    }

    #[cfg(test)]
    pub(crate) fn execute_deferred_delete_for_test(&mut self) {
        if let Some(sel) = self.deferred.delete.take() {
            self.execute_delete(sel, false);
        }
    }

    /// Answer Save in the unsaved-work dialogue — the same as pressing the button.
    #[cfg(test)]
    pub(crate) fn answer_save_for_test(&mut self) {
        self.save_project();
        if crate::gui::io_jobs::saving_now(&self.regen) || self.asking_for_a_file() {
            self.deferred.nav_after_save = true;
        }
    }

    /// Draw the dialogue or the card — the same thing a frame does.
    #[cfg(test)]
    pub(crate) fn nav_dialog_for_test(&mut self, ctx: &egui::Context) {
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::nav_dialog(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        }
    }

    /// How many bodies actually reach the viewport (the same list the CPU raster and the GPU pass draw).
    /// HOW MANY BODIES ARE ON SCREEN. The list itself cannot come back through a method: it borrows out of
    /// the document, and a context built on the stack dies with the call.
    #[cfg(test)]
    pub(crate) fn visible_mesh_count(&self) -> usize {
        let pn = self.painting();
        visible_mesh_items(&pn).len()
    }

    /// A facade for the parameters window: editing a global parameter by the same handle the window uses.
    #[cfg(test)]
    pub(crate) fn set_param_for_test(&mut self, name: &str, expr: &str) {
        use qymcad_core::model::Param;
        match self.project.parameters.iter_mut().find(|p| p.name == name) {
            Some(p) => p.expr = expr.to_string(),
            None => self.project.parameters.push(Param { name: name.to_string(), expr: expr.to_string(), value: 0.0 }),
        }
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::apply_param_edit(&mut self.win_ctx(&mut asks));
            self.do_win_asks(asks, &egui::Context::default());
        };
    }

    // --- Undo and redo (snapshots) ---

    /// OPEN AN OPERATION on the document. Everything that changes the document must go through it.
    /// A nested call joins the operation already open: one action, one undo step.
    pub(crate) fn edit(&mut self, name: impl Into<String>) -> Edit<'_> {
        qymcad_ui_state::edit_over(self.rebuild_ctx(), name)
    }

    pub(crate) fn undo(&mut self) {
        self.active_path = qymcad_ui_state::step_through_history(&mut self.rebuild_ctx(), false).unwrap_or_else(|| self.active_path.clone());
    }

    fn redo(&mut self) {
        self.active_path = qymcad_ui_state::step_through_history(&mut self.rebuild_ctx(), true).unwrap_or_else(|| self.active_path.clone());
    }
}

/// WHERE THE PANELS GO. The sizes and the framing that used to be written inside each panel are said here
/// instead - they describe the container, not what is drawn in it.
///
/// Built afresh each frame rather than kept in a field: it is a dozen pushes, and a stored layout is a
/// second copy of this list that would drift from it. What the person MOVED does live on, in the settings,
/// and is laid over the registration.
///
/// FREE, not a method: it reads the settings and nothing else, and `App` is large enough already.
pub(crate) fn shell(set: &Settings) -> qymcad_shell::Shell {
    use qymcad_shell::{Place, Slot};
    let mut s = qymcad_shell::Shell::default();
    s.put(Place::new("menubar", Slot::Menu));
    s.put(Place::new("toolbar", Slot::Top));
    s.put(Place::new("section_bar", Slot::Top).framed());
    s.put(Place::new("comp_array_bar", Slot::Top).framed());
    s.put(Place::new("feat_cmd_bar", Slot::Top).framed());
    s.put(Place::new("sk_tool_opts", Slot::Top).framed());
    s.put(Place::new("bool_tool_bar", Slot::Top).framed());
    s.put(Place::new("joint_tool_bar", Slot::Top).framed());
    s.put(Place::new("joint_edit_bar", Slot::Top).framed());
    s.put(Place::new("status", Slot::Bottom));
    s.put(Place::new("wbtools", Slot::Left).sized(108.0, false).bare());
    s.put(Place::new("tree", Slot::Left).sized(260.0, true));
    s.put(Place::new("props", Slot::Right).sized(290.0, true));
    s.put(Place::new("viewport", Slot::Centre));
    s.restore(set.layout.clone());
    s
}

impl qymcad_shell::Fills for App {
    /// IS THE PLACE WANTED THIS FRAME. Each of these used to be a `return` at the top of the panel - which
    /// worked only while the panel opened its own container. Now the container is opened first, and an empty
    /// one still eats a strip of the window, so the question is asked before opening rather than inside.
    fn live(&self, key: &'static str) -> bool {
        match key {
            "section_bar" => {
                self.side.section.plane.is_some()
                    || self.side.section.pick
                    || self.params.mirror.in_hand()
                    || self.tools.picking.is_sketch_plane()
                    || self.tools.picking.sketch_for().is_some()
                    || self.side.m3.on
            }
            "comp_array_bar" => self.side.carr.mode != 0,
            "feat_cmd_bar" => self.tools.armed.commanding(),
            "sk_tool_opts" => edit_si(&self.project, &self.sketch_ses).is_some(),
            "bool_tool_bar" => self.params.boolean.pick.is_some() || self.params.boolean.edit.is_some(),
            // THE ASSEMBLY BARS: each is one of several by what is being picked, and the shell now owns
            // their places. The conditions used to live at the top of each bar as an early return - which
            // meant the place existed even when nothing was drawn in it.
            "joint_tool_bar" => matches!(self.workbench, Workbench::Assembly) && self.viewing.mode_3d && qymcad_assembly::joint_picking(&self.side.joint),
            "joint_edit_bar" => self.side.joint.edit.is_some() && matches!(self.workbench, Workbench::Assembly) && self.viewing.mode_3d,
            // while a sketch is being edited the tree on the left is not needed - only the tools
            "tree" => edit_si(&self.project, &self.sketch_ses).is_none(),
            _ => true,
        }
    }

    fn fill(&mut self, key: &'static str, ui: &mut egui::Ui) {
        // WHAT THE BARS ASKED FOR IS DONE AFTER THE DRAWING. A button that acted mid-frame would start a
        // command while the bar of that command was still being laid out.
        let (mut bar_asks, mut win_asks) = (Vec::new(), Vec::new());
        match key {
            "menubar" => crate::gui::panels_bars::menu_bar(&mut self.bar_ctx(&mut bar_asks), ui),
            "toolbar" => crate::gui::panels_bars::toolbar(&mut self.bar_ctx(&mut bar_asks), ui),
            "section_bar" => crate::gui::panels_bars::section_bar(
                &mut qymcad_part::PlaneBarCtx {
                    regen: &mut self.regen,
                    section: &mut self.side.section,
                    mirror: &mut self.params.mirror,
                    picking: &mut self.tools.picking,
                    project: &mut self.project,
                    edits: &mut self.disk.edits,
                    active_path: &self.active_path,
                    status: &mut self.status,
                    m3: &mut self.side.m3,
                },
                ui,
            ),
            "comp_array_bar" => crate::gui::panels_bars::comp_array_bar(&mut self.part_ctx(), ui),
            "feat_cmd_bar" => self.feat_command_bar(ui),
            "sk_tool_opts" => crate::gui::panels_bars::tool_options_bar(&mut self.bar_ctx(&mut bar_asks), ui),
            "bool_tool_bar" => crate::gui::commands::bool_tool_bar(&mut self.part_ctx(), ui),
            "joint_tool_bar" => qymcad_assembly::joint_tool_bar(&mut self.joint_ctx(), ui),
            "joint_edit_bar" => qymcad_assembly::joint_edit_bar(&mut self.joint_ctx(), ui),
            "status" => crate::gui::panels_bars::status_bar(
                &mut qymcad_ui_state::StatusCtx {
                    cache: &self.cache,
                    cursor: self.cursor,
                    project: &self.project,
                    scheme: &self.scheme,
                    set: &mut self.set,
                    sketch_ses: &self.sketch_ses,
                    line: qymcad_ui_state::StatusLine { said: &mut self.status, rebuild: &mut self.regen.line },
                    win: &mut self.win,
                    par_search: &mut self.par_search,
                    ask: &mut win_asks,
                },
                ui,
            ),
            "wbtools" => crate::gui::panels_bars::wb_toolbar(&mut self.bar_ctx(&mut bar_asks), ui),
            "tree" => self.tree_panel(ui),
            "props" => self.properties_panel(ui),
            "viewport" => self.viewport(ui),
            _ => {}
        }
        self.do_bar_asks(bar_asks, &ui.ctx().clone());
        self.do_win_asks(win_asks, &ui.ctx().clone()); // a click on a fault in the status line goes where a window's link goes
    }

    fn bar_frame(&self) -> egui::Frame {
        qymcad_ui_state::tool_bar_frame(&self.scheme)
    }
}

impl eframe::App for App {
    /// Save the settings between sessions (the machine plus the view preferences).
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.set);
        eframe::set_value(storage, "last_project", &self.disk.project_path); // the path of the current document
    }

    /// THE FRAME. Named `ui` rather than `update` since egui 0.36: panels now live inside a `Ui`, and
    /// the framework hands the root one in. The context is still wanted for windows, input and viewport
    /// commands, and it comes from the same place.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw_frame(ui);
    }
}

impl App {
    /// THE WHOLE FRAME, in one method a test can call. `eframe::Frame` is not used by any of it and cannot be
    /// built outside a live window, so the body lives here rather than in the trait: without this the frame -
    /// the one place where the order of the phases is decided - could only be checked by reading it.
    pub(crate) fn draw_frame(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        // The canvas of this frame, for a problem report: three atomic stores, so it costs nothing at
        // this rate. Taken before the prologue can return - a report is wanted most when the start-up
        // load is what went wrong.
        crate::diagnostics::note_viewport(ctx.viewport_rect().size(), ctx.pixels_per_point());
        self.keep_the_title_current(ctx);
        // THE FRAME PROLOGUE: until it says "carry on", there is nothing to draw (the start-up load is running).
        if !self.frame_prologue(ctx) {
            return;
        }
        // THE ANSWER FROM AN OPEN FILE CHOOSER, picked up at the top of the frame so the file is acted on in
        // the same frame it was chosen in. While one is open the frames are asked for by hand: the answer
        // comes from a thread, and egui, which sleeps between input events, hears nothing of it.
        let choosing = self.poll_file_ask();
        if choosing {
            ctx.request_repaint();
            crate::gui::file_ask::inert_while_choosing(ui); // the system chooser is modal: nothing here answers until it does
        }
        // the reference window's keys go nowhere else: Esc must not walk the cancel ladder, E must not extrude
        let capturing = hotkeys::hotkeys_take_keyboard(&mut self.win, &mut self.hotkeys, ctx);
        // Ctrl+S saves (silently into the current file, or a dialogue for a new one); Ctrl+Shift+S is "save as".
        if !choosing && !capturing && !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            if ctx.input(|i| i.modifiers.shift) {
                self.save_project_as();
            } else {
                self.save_project();
            }
        }
        // the sign of a background rebuild; the canvas is the PREVIOUS frame's, drawn before the viewport itself
        rebuild_sign::draw_rebuild_sign(&self.regen, &self.scheme, self.viewing.view_rect, ctx, &mut self.status);
        self.sync_workbench(); // the workbench and the active context are derived from `active_path` (drill in and out)
        self.chosen.hover.joint = None; // the hovered joint is rebuilt every frame (by the panel and by 3D below)
        self.keep_selection_on_edited_sketch(); // the selection follows the sketch being edited — in one phase
                                                // THE KEYBOARD IS THE CHOOSER'S while it is open: a barrier eats clicks, but these two read the
                                                // input directly and would go on obeying Delete, Escape and every tool letter behind it.
        if !choosing && !capturing {
            self.handle_key_commands(ctx); // the frame's keyboard commands — in one phase
            self.handle_tool_hotkeys(ctx); // the tool shortcuts (L/R/C/A/P/G/D/S, E)
        }
        self.maybe_autosave(false); // a silent autosave every 3 minutes while there are unsaved edits
        self.help_window(ctx); // the help window
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::save_template_dialog(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        } // "save as a template" — the name and the confirmation
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::confirm_delete_popup(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        } // the popup confirming the deletion of a tree node
        let shell = crate::gui::shell(&self.set);
        shell.run_slot(qymcad_shell::Slot::Menu, ui, self);
        self.take_screenshot(ctx); // the picture of the window for a report comes back as an event
        crate::gui::panels_windows::crash_notice(&mut self.disk.crash_report, ctx); // "the last run ended in an error" - only after a crash
        self.report_window(ctx); // Help -> Report a problem
        crate::gui::panels_windows::about_dialog(&mut self.win, &self.scheme, ctx); // the About window
        crate::gui::panels_windows::updates_dialog(&mut self.win, &self.scheme, ctx); // Help -> Check for updates
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::doc_props_window(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        } // the document properties
        self.start_screen(ctx); // where to begin — only on a blank slate
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::nav_dialog(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        } // the modal "save the changes?" over the menu
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::mesh_quality_dialog(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        } // choosing the mesh quality before exporting
        crate::gui::import_scale::import_scale_window(&mut self.win_ctx(&mut Vec::new()), ctx); // the units and the scale of a file just read
                                                                                                // "Finish" lives in one place: the button in the breadcrumbs (the toolbar). There is no separate banner.
        tick_view_anim(&mut self.viewing.cam, &mut self.viewing.view_anim, ctx); // the smooth turn of the view (the ViewCube)
        tick_joint_anim(&mut self.joint_anim, &mut self.project, ctx); // sweeping a joint's degree of freedom
        self.hotkeys_window(ctx); // Help -> Shortcuts
        self.command_search_window(ctx); // the command search (Space or Ctrl+K)
        shell.run_slot(qymcad_shell::Slot::Top, ui, self);
        shell.run_slot(qymcad_shell::Slot::Bottom, ui, self);
        shell.run_slot(qymcad_shell::Slot::Left, ui, self);
        shell.run_slot(qymcad_shell::Slot::Right, ui, self);
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::parts_library_window(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        }
        {
            let mut asks = Vec::new();
            crate::gui::io_jobs::save_part_window(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        } // "save as a part of the library"
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::doc_windows(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        }
        {
            let mut asks = Vec::new();
            crate::gui::panels_windows::settings_window(&mut self.win_ctx(&mut asks), ctx);
            self.do_win_asks(asks, ctx);
        }
        shell.run_slot(qymcad_shell::Slot::Centre, ui, self); // last, and now for a stated reason
                                                              // A tool waiting for a sketch takes whichever one was just selected - tree or viewport, no
                                                              // difference. After the panels on purpose: the click that chose it has landed by now.
        qymcad_part::take_sketch_if_waiting(&mut self.part_ctx());
        // commit an undo step if the edit has finished
        maybe_commit(&mut self.disk.edits, &mut self.tools.place, &self.project, &self.set, ctx);
    }
}

impl App {
    /// Load a `.qpart` from a source and graft it into the target assembly (`graft`, then `regenerate_all`).
    fn insert_part_from(&mut self, src: crate::parts_library::PartSource) {
        let loaded = match &src {
            crate::parts_library::PartSource::User(p) => qymcad_io::load_part(&p.to_string_lossy()),
            crate::parts_library::PartSource::Embedded(rel) => match crate::parts_library::embedded_bytes(rel) {
                Some(b) => qymcad_io::load_part_bytes(b),
                None => Err(crate::i18n::tr("lib-part-not-found")),
            },
        };
        let loaded = match loaded {
            Ok(l) => l,
            Err(e) => {
                self.status = Notice::error(crate::i18n::tr1("lib-load-error", "error", &e.to_string()));
                return;
            }
        };
        let target = parts_insert_target(&mut self.project);
        match self.project.graft(&loaded.project, target, &crate::i18n::name) {
            Some(_) => {
                qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
                invalidate(&mut self.regen);
                self.status = Notice::info(crate::i18n::tr1("lib-part-inserted", "name", &loaded.manifest.name));
            }
            None => {
                self.status = Notice::refusal(crate::i18n::tr("lib-no-assembly"));
            }
        }
    }

    /// Open the "Save as a standard part" dialogue for component `cid`. The name comes from the component,
    /// the preview is rendered right away (the body in its own frame), and the list of existing categories
    /// is there for picking one quickly.
    fn open_save_part_dialog(&mut self, cid: qymcad_core::model::Id) {
        let name = self.project.components.iter().find(|c| c.id == cid).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
        let preview = crate::gui::render_scene::render_component_thumbnail(&self.draw_ctx(), cid);
        self.parts.save = Some(SavePartDialog {
            component: cid,
            name,
            description: String::new(),
            tags: String::new(),
            category: String::new(),
            known_cats: crate::parts_library::user_category_paths(),
            preview,
            tex: None,
        });
        self.status = Notice::info(crate::i18n::tr("lib-save-hint"));
    }

    // ============ The splash screen and the progress of background work ============

    /// THE TITLE FOLLOWS THE DOCUMENT: which file is open, and whether it holds unsaved work.
    ///
    /// Sent only when it has changed. A viewport command every frame is a message to the window manager
    /// sixty times a second for a string that moves a few times an hour, and on some of them it makes the
    /// title flicker.
    fn keep_the_title_current(&mut self, ctx: &egui::Context) {
        // THE DIRTY FLAG FIRST: it is a `&mut self` call now (the rebuild moved behind a context), and
        // taking it inside the argument list would borrow `self` twice in one expression.
        let dirty = qymcad_ui_state::is_dirty(&mut self.rebuild_ctx());
        let want = crate::gui::window_title::window_title(self.disk.project_path.as_deref(), dirty);
        if want != self.disk.title_shown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(want.clone()));
            self.disk.title_shown = want;
        }
    }

    /// Recompute the WHOLE timeline from scratch (the Edit -> Rebuild everything item). Every node is
    /// marked dirty and the work goes into a background regeneration — the screen does not collapse, an
    /// overlay spins instead.
    pub(crate) fn rebuild_everything(&mut self) {
        self.project.mark_all_dirty();
        // AND ADMIT THAT IT IS GONE. Two flags used to be left as they were, describing exactly what the
        // command had just thrown away: "the preparation is finished" and "this revision has already been
        // tried". After the rebuild the program considered the live geometry ready although no body had
        // any, and a repeat preparation bailed out immediately on the "already tried" guard.
        //
        // Reported behaviour: pressing Rebuild everything changed nothing, the B-rep still would not
        // build. The one command that could have put things right declared the work done.
        self.live.ready = false;
        self.live.tried_rev = None;
        self.spawn_regen();
        // AND BRING BACK WHAT THE TIMELINE CANNOT.
        //
        // Every node is dirty, so every body the timeline builds gets a live B-rep built anew - and one whose
        // node no longer builds keeps its last good body, as a partial rebuild keeps it. Imported bodies are
        // not in any recipe: their geometry lives in the embedded STEP, and a separate path raises it again.
        // When the cache used to be emptied here, not calling that path took the live geometry of a whole
        // imported assembly away until the program was restarted.
        //
        // MEASURED IN A LIVE WINDOW on a document with 138 imported bodies: after the command there were
        // "0 live shapes", and every following rebuild arrived with a plan of "0 nodes out of 142" — there
        // was nothing for it to do. Reported behaviour: pressing Rebuild everything sent the CAD into a
        // fever of endless flickering.
        self.spawn_import_shapes(false); // quietly, in the background: the model on screen is intact, only the geometry is awaited
        self.regen.import_asked = true;
        self.status = Notice::info(crate::i18n::tr("io-rebuilding-all"));
    }

    /// Turn the plane pick for a new sketch on or off (the Sketch button and the K shortcut). It cancels
    /// an active datum or command (`cancel_all_tools` clears the sketch-plane pick, so the target state is
    /// remembered BEFORE the reset), so that two tools are never held at once.
    fn toggle_sketch_pick(&mut self) {
        let turning_on = !self.tools.picking.is_sketch_plane();
        self.cancel_all_tools();
        self.tools.picking = if turning_on { Picking::SketchPlane(None) } else { Picking::None }; // pressed again, the pencil is put down
        if self.tools.picking.is_sketch_plane() {
            self.viewing.mode_3d = true; // planes and faces are picked in 3D
            self.chosen.sel = Sel::None;
            self.status = Notice::info(crate::i18n::tr("g-sketch-pick-plane"));
        }
    }

    /// Which gizmo axis is under the cursor (0 = X, 1 = Y, 2 = Z), if it is close enough (within 8 px).
    fn gizmo_axis_hit(&self, comp: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let (o, l) = gizmo_geometry(self.viewing.cam, self.dragged.comp_giz, &self.project, comp);
        gizmo_axis_hit_at(&self.draw_ctx(), o, l, rect, basis, pp)
    }

    /// Dragging a component gizmo's AXIS: accumulate the world shift along the axis (projected onto the
    /// FIXED screen axis) into `comp_giz_drag.amt` and apply it through `apply_comp_giz`. Unified with the
    /// body gizmo.
    fn drag_component_axis(&mut self, _comp: Id, ax: u8, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((_, _, origin, _)) = self.dragged.comp_giz.drag else { return };
        let l = 60.0 / self.viewing.cam.scale as f64;
        let mut tip = origin;
        tip[ax as usize] += l;
        let scr = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis };
        let s0 = scr.at(origin).0;
        let s1 = scr.at(tip).0;
        let pd = s1 - s0;
        let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
        if denom < 1e-6 {
            return;
        }
        let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
        if let Some(drag) = &mut self.dragged.comp_giz.drag {
            drag.3 += inc;
        }
        crate::gui::commands::apply_comp_giz(&mut self.part_ctx());
    }

    /// Which rotation ring of the gizmo is under the cursor (0 = X, 1 = Y, 2 = Z), if it is close enough
    /// (within 6 px). The ring of axis `ax` is a circle of radius L in the plane perpendicular to that
    /// axis, around the component's origin.
    fn gizmo_ring_hit(&self, comp: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let (o, l) = gizmo_geometry(self.viewing.cam, self.dragged.comp_giz, &self.project, comp);
        gizmo_ring_hit_at(&self.draw_ctx(), o, l, rect, basis, pp)
    }

    /// Dragging a component gizmo's RING: accumulate the angle (about the FIXED origin) into
    /// `comp_giz_drag.amt` and apply it through `apply_comp_giz` (the rotation is the accumulated one
    /// composed with the start, with snapping). Unified with the body gizmo.
    fn drag_component_ring(&mut self, _comp: Id, ax: u8, cursor: Pos2, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((_, _, origin, _)) = self.dragged.comp_giz.drag else { return };
        let center = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis }.at(origin).0;
        let radial = cursor - center;
        let r2 = (radial.x * radial.x + radial.y * radial.y) as f64;
        if r2 < 4.0 {
            return;
        }
        let ccw = -(radial.x * d.y - radial.y * d.x) as f64 / r2;
        let sign = ring_drag_sign(basis.2[ax as usize]);
        if let Some(drag) = &mut self.dragged.comp_giz.drag {
            drag.3 += ccw.to_degrees() * sign;
        }
        crate::gui::commands::apply_comp_giz(&mut self.part_ctx());
    }

    // ===== The degree-of-freedom aware gizmo of a driven component. What gets dragged is the JOINT'S
    // FREEDOM (angle, offset, offset2) rather than a free 6-DOF transform, so the drag stays WITHIN the
    // joint and `solve_joints` works the rest out. =====

    // ===== The BODY gizmo in a Part: it reuses `gizmo_*_hit_at` and `draw_gizmo_at`; the movement itself
    // is a parametric Move feature. The drag state is `body_giz_axis`/`ring` plus the accumulated
    // `body_giz_drag`. =====

    /// A click on an axis candidate for a REVOLVE: a STRAIGHT edge of a body, a CYLINDRICAL face, or a
    /// datum axis, each turned into an associative datum. Reported behaviour: the "pick an axis (3D)"
    /// button would not take a datum axis and simply never worked.
    ///
    /// A method of its own, because this branch used to live in the 2D half of the viewport while the
    /// candidates are drawn and hit-tested ONLY in 3D. The button did switch the view to 3D and the axes
    /// did light up, but there was NOBODY there to catch the click: the handler stayed in the flat branch
    /// and was never called. Now the branch is in 3D and the logic sits here, checked by a test without egui.
    fn rev_axis_pick_click(&mut self, rect: Rect, pos: Pos2) -> bool {
        let picked = match crate::gui::pick::pick_axis_at(&self.painting(), rect, pos) {
            Some(AxisHit::Datum(id)) => Some((id, "g-axis-datum")),
            Some(AxisHit::Edge(i)) => axis_from_edge(&self.active_path, &self.edges, &self.live, &mut self.project, i).map(|id| (id, "g-axis-body-edge")),
            Some(AxisHit::Face(b, f)) => axis_from_face(&self.active_path, &self.edges, &self.live, &mut self.project, b, f).map(|id| (id, "g-axis-cyl-face")),
            None => None,
        };
        match picked {
            Some((id, what)) => {
                self.params.rev.axis_datum = id;
                self.params.rev.axis_line = 0; // the axis was set in 3D, so the sketch's axis line no longer applies
                self.params.rev.pick_axis = false;
                self.status = Notice::info(format!("{} {}", ph::CHECK, crate::i18n::tr1("g-rev-axis", "what", &crate::i18n::tr(what))));
                true
            }
            None => {
                self.status = Notice::refusal(crate::i18n::tr("g-rev-axis-miss"));
                false
            }
        }
    }

    /// Finish a body gizmo drag: apply the accumulated transform as a PARAMETRIC Move feature.
    fn commit_body_gizmo(&mut self, snap: bool) {
        let accum = body_giz_accum(&self.dragged.body_giz, &self.set, snap);
        let drag = self.dragged.body_giz.drag;
        self.dragged.body_giz.drag = None;
        self.dragged.body_giz.axis = None;
        self.dragged.body_giz.ring = None;
        let (Some(accum), Some((mi, _, amt))) = (accum, drag) else { return };
        if amt.abs() < 1e-6 || qymcad_core::feature::is_identity12(&accum) {
            return; // a zero drag: nothing is committed
        }
        crate::gui::commands::apply_body_move(&mut self.part_ctx(), mi, accum);
    }

    /// Clear ALL the active tools and picking modes. Called on a change of workbench and on entering or
    /// leaving a component: a command belonging to one context must NOT leak into another (an assembly tool
    /// used to stay active after a double click into a part, and so on across every workbench).
    /// CANCEL EVERYTHING (Esc from the top bar, or the start of a new command): every active mode goes out.
    ///
    /// Assembled from the transitions that already exist rather than from a field list of its own. Such a
    /// copy is what it used to be — the seventh one — and its set differed from the others: it cleared the
    /// joint pick and the section but left the modify mode, the dimension's first reference, the dragging
    /// and the in-place editing alone. What is listed here is ONLY what is in neither the exit from the
    /// sketch tools nor the reset of a feature's aiming: the assembly, section and clipboard modes.
    fn cancel_all_tools(&mut self) {
        if self.tools.armed.commanding() {
            cancel_feat_cmd(&mut self.part_ctx());
        }
        exit_draw_tools(&mut qymcad_ui_state::tools_of!(self)); // every sketch mode, in one transition
        clear_feat_picks(qymcad_ui_state::feat_picks_of!(self)); // the aiming of the feature's subsystems
        self.params.boolean.pick = None;
        self.params.boolean.edit = None;
        // EVERY ASSEMBLY TOOL, THROUGH THE ONE DOOR THAT WALKS `AssemblyTool::ALL`.
        //
        // This used to be nine fields listed here by hand, and it had already fallen behind: the anchor
        // RE-PICK was missing. Taking any other assembly tool while it was armed left TWO in hand, the click
        // went to whichever handler stood higher, and the person was certain they were working with the last
        // one taken. Esc released it (that path always called `drop_assembly_tools`); changing tools did not.
        crate::gui::assembly_tools::drop_assembly_tools(&mut self.joint_ctx());
        qymcad_part::section_cancel(&mut self.side.section, &mut self.regen); // a section still being placed goes; one applied stays
        self.params.mirror.drop_part(); // an unfinished pick of the part to mirror, and its plane
        self.tools.pending_import.clear(); // the whole unfinished import (the curves plus the points)
        self.side.clip.drop_geom(); // an unfinished copy or paste of geometry
        self.side.m3.clear(); // the 3D measuring tool is a tool too, exclusive with the rest
        self.side.carr = CompArrayCmd::default(); // an unfinished component pattern
    }

    /// The Part layout: K a new sketch, D a datum plane, E extrude, Q cut, R revolve, F fillet, C chamfer,
    /// H shell, O hole, M mirror, B box, Y cylinder.
    pub(super) fn part_hotkey(&mut self, key: impl Into<qymcad_ui_state::Chord>) {
        // WE MATCH ON THE ACTION, NOT ON THE KEY. While `Key::E` stood in the `match`, remapping was
        // inexpressible: the letter and the meaning were one and the same thing. Which key leads to which
        // action is decided by `hotkey_action` — one place for every workbench.
        let Some(action) = qymcad_ui_state::hotkey_action(&self.set, "part", key) else { return };
        match action {
            "part.sketch-pick" => self.toggle_sketch_pick(),
            "part.datum-plane" => self.start_feat_cmd(20),
            "part.extrude" => {
                self.feat.op = 0;
                self.start_feat_cmd(1);
            }
            "part.cut" => {
                self.feat.op = 2;
                self.start_feat_cmd(1);
            }
            "part.revolve" => self.start_feat_cmd(3),
            "part.fillet" => self.start_feat_cmd(4),
            "part.chamfer" => self.start_feat_cmd(5),
            "part.shell" => self.start_feat_cmd(6),
            "part.hole" => self.start_feat_cmd(7),
            "part.mirror" => self.start_feat_cmd(16),
            "part.box" => self.start_prim_cmd(10),
            "part.cylinder" => self.start_prim_cmd(11),
            "part.measure" => self.toggle_measure_3d(),
            // go back to picking the contours of the active sketch command (extrude, revolve) from its 3D step
            "part.contour-reselect" if matches!(self.tools.armed.cmd_kind(), 1 | 3) && self.tools.cmd.sketch.is_some() && self.viewing.mode_3d => {
                crate::gui::commands::enter_contour_reselect(&mut self.part_ctx())
            }
            _ => {}
        }
    }

    /// The Assembly layout: K a new skeleton sketch, D a datum plane, N a new part, U a subassembly,
    /// I insert a component (STEP or STL), J a rigid joint (picking faces).
    pub(super) fn assembly_hotkey(&mut self, key: impl Into<qymcad_ui_state::Chord>) {
        let Some(action) = qymcad_ui_state::hotkey_action(&self.set, "assembly", key) else { return };
        match action {
            // there are NO sketch keys in an Assembly: a sketch is inert there (see `create_panel_common`)
            "assembly.datum-plane" => self.start_feat_cmd(20),
            "assembly.new-part" => {
                let id = self.project.add_part(self.project.free_part_name());
                self.enter_component(id);
            }
            "assembly.new-subassembly" => {
                let id = self.project.add_assembly(crate::i18n::tr1("node-assembly-n", "n", &self.project.components.len().to_string()));
                self.enter_component(id);
            }
            "assembly.insert" => self.pick_import(qymcad_ui_state::Want::Part),
            "assembly.rigid-joint" => {
                self.cancel_all_tools();
                self.side.joint.new_kind = qymcad_core::feature::JointKind::Rigid;
                self.side.joint.pick_faces = true;
                self.side.joint.pick_first = None;
                self.status = Notice::info(crate::i18n::tr("g-rigid-joint-hint"));
            }
            _ => {}
        }
    }

    /// Remember a body's faces and put them into the index-parallel `self.faces` (the single point where
    /// these two representations cannot drift apart).
    /// REORDER A TIMELINE NODE — one handle for everyone.
    ///
    /// The core refuses by itself if the move would break the dependencies (a consumer ending up above its
    /// input); what is left here is keeping the caches in step. Pulled out of the menu item for the same
    /// reason as the section: logic inside a button is out of reach of a check, and the hand would use a
    /// different door from the person's.
    /// Returns whether the move happened.
    pub(crate) fn move_feature(&mut self, from: usize, to: usize) -> bool {
        let done = self.project.reorder_feature(from, to);
        if done {
            crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
        }
        done
    }

    /// TURN THE SECTION VIEW ON OR OFF — one handle for everyone.
    ///
    /// The logic used to live inside the panel's button, and there was no way to reach it by hand: a test
    /// either poked at the fields directly (checking itself, that is) or checked nothing at all. The rule
    /// that a hand must use the same doors a person does applies here too.
    pub(crate) fn toggle_section(&mut self) {
        if self.side.section.plane.is_some() || self.side.section.pick {
            self.side.section.put_away();
            invalidate(&mut self.regen);
            self.status = Notice::info(crate::i18n::tr("tb-section-off"));
        } else {
            self.cancel_all_tools();
            self.side.section.pick = true;
            self.status = Notice::info(crate::i18n::tr("tb-section-pick"));
        }
    }

    /// Create (with target == 0) or re-target the face (target = plane_id) of an "offset from a face" datum plane.
    fn make_offset_plane_from_face(&mut self, target: Id, body: Id, key: qymcad_core::feature::FaceKey) {
        use qymcad_core::model::{PlaneDef, WorkPlane};
        if target == 0 {
            let wp = WorkPlane { name: crate::i18n::tr("plane-from-face"), def: PlaneDef::OffsetFace { body, face: key, dist: 10.0 }, ..Default::default() };
            let id = self.project.add_plane(wp);
            self.chosen.sel = self.project.planes.iter().position(|p| p.id == id).map(Sel::Plane).unwrap_or(Sel::None);
        } else if let Some(pi) = self.project.planes.iter().position(|p| p.id == target) {
            let dist = if let PlaneDef::OffsetFace { dist, .. } = self.project.planes[pi].def { dist } else { 10.0 };
            self.project.planes[pi].def = PlaneDef::OffsetFace { body, face: key, dist };
            self.chosen.sel = Sel::Plane(pi);
        }
        self.regen_after_datum_change();
        self.status = Notice::info(crate::i18n::tr("plane-from-face-hint"));
    }

    // `viewport` was 1509 lines and looked like a monolith, but it was in fact TWO unrelated viewports in
    // one if/else branch: the 3D one (537 lines) and the flat sketch one (959). While they lay together,
    // neither showed its phases — and neither could be taken apart on its own.

    /// THE FRAME PROLOGUE: free the previous frame's textures, show the splash screen, raise the undo
    /// baseline, finish a deferred datum regeneration, and intercept the closing of a window with unsaved
    /// work.
    ///
    /// Returns `false` when the rest of the frame must NOT be drawn (the start-up load is running and only
    /// the overlay is shown). This used to be a `return` in the middle of `update`, which made the prologue
    /// impossible to separate from the rest of the frame: the early exit hid inside the shared body.
    fn frame_prologue(&mut self, ctx: &egui::Context) -> bool {
        // free the previous frame's textures HERE, at the start and before any drawing, so that nothing is
        // still drawing them (otherwise wgpu panics with "Texture ... has been destroyed" on submit).
        self.tex_graveyard.clear();
        // the splash screen and the progress. While the start-up load of a document or a background job is
        // running, only the spinner overlay is drawn and we leave (the window with the logo is visible at
        // once and the UI does not appear frozen).
        if self.tick_async(ctx) {
            return false;
        }
        // the first initialisation of the undo baseline
        if !self.disk.edits.ready {
            self.disk.edits.baseline = snapshot(&self.project);
            self.disk.edits.committed_key = doc_key(&self.project);
            self.disk.edits.saved_key = edit_key(&self.draw_ctx()); // at start-up (an empty document, or one opened automatically) there are no edits
            self.disk.edits.ready = true;
        }
        // Debouncing a datum edit: while a coordinate or a direction is being dragged (the pointer is held
        // down) the consumers are NOT force-regenerated every frame; the datum's own glyph moves live,
        // because the renderer reads the fields directly. Release the pointer and one regeneration runs at
        // the final position. Done BEFORE the panels, so the new geometry shows in this same frame.
        if self.side.datum.regen_pending && !ctx.input(|i| i.pointer.any_down()) {
            self.side.datum.regen_pending = false;
            self.regen_after_datum_change();
        }
        // Intercepting the closing of the window (the cross, or Alt+F4): with unsaved work, cancel the close and ask.
        if ctx.input(|i| i.viewport().close_requested()) {
            // a background write must reach the disk even when the document is "clean" (Save was pressed and
            // the window closed straight away) — otherwise one is sure it saved while the file stayed as it
            // was (the write is atomic: an interruption does not spoil the old file, but neither does it
            // carry the new edits).
            self.wait_bg();
            if !self.disk.edits.allow_close && qymcad_ui_state::is_dirty(&mut self.rebuild_ctx()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if self.deferred.nav.is_none() {
                    self.deferred.nav = Some(Nav::Exit);
                }
            }
        }
        true
    }

    /// THE SELECTION FOLLOWS THE SKETCH BEING EDITED.
    ///
    /// While a sketch is being edited the selection stays on it (all the logic works through `Sel::Sketch`),
    /// and the sketch itself is remembered for projecting the part's geometry into it. Changing the active
    /// sketch clears the accumulated selection — otherwise it moves onto SOMEONE ELSE'S geometry. A phase
    /// of the frame rather than an implementation detail of a neighbouring block: this used to sit between
    /// the window-close interception and the keyboard, and the link "the sketch changed, so the selection
    /// was cleared" was invisible.
    fn keep_selection_on_edited_sketch(&mut self) {
        // sketch editing mode: the selection is kept on it (all the logic works through `Sel::Sketch`)
        match edit_si(&self.project, &self.sketch_ses) {
            Some(si) => {
                self.chosen.sel = Sel::Sketch(si);
                self.workbench = Workbench::Sketch;
            }
            None => {
                if self.sketch_ses.editing.is_some() {
                    self.sketch_ses.editing = None; // the sketch is gone, so leave the mode
                }
            }
        }
        // remember the active sketch (for projecting the part's geometry into it)
        if let Sel::Sketch(si) = self.chosen.sel {
            if let Some(s) = self.project.sketches.get(si) {
                if self.sketch_ses.last != Some(s.id) {
                    self.tools.sel_sk.clear(); // the selection, and whatever was waiting on it: the sketch changed, so the element selection goes
                }
                self.sketch_ses.last = Some(s.id);
            }
        } else {
            self.tools.sel_sk.clear(); // the selection, and whatever was waiting on it
        }
    }

    pub(crate) fn viewport(&mut self, ui: &mut egui::Ui) {
        // The panel lives inside a `Ui` now; the context is still wanted for windows,
        // input and viewport commands, and it comes from the same place.
        let ctx = &ui.ctx().clone();
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.viewing.view_rect = rect; // where the canvas stands: the rebuild overlay needs it so as not to blank it
                                       // THE CANVAS BACKGROUND COMES FROM THE SCHEME. A `from_gray(26)` used to stand here, asking the
                                       // theme nothing at all: switch to the light one and the viewport and the sketcher stayed black.
        painter.rect_filled(rect, 0.0, self.scheme.pal.viewport_bg());
        let has_geom = !self.project.contours.is_empty() || !self.project.bodies.is_empty();
        let scroll = qymcad_ui_state::view_scroll(ctx, &resp, self.set.mouse_nav, !self.viewing.mode_3d); // the wheel, or the layout's zoom gesture

        if self.viewing.mode_3d {
            self.viewport_3d(ctx, &resp, &painter, rect, has_geom, scroll);
        } else {
            self.viewport_2d(ctx, &resp, &painter, rect, has_geom, scroll);
        }
    }
}

#[cfg(test)]
pub(crate) fn shade_tri_for_test(pal: &crate::palette::Palette, ghost_alpha: u8, hot: bool, ghost: bool, base: [u8; 3], n: [f64; 3], lights: &qymcad_ui_state::SceneLights) -> Color32 {
    qymcad_pick::shade_tri(pal, ghost_alpha, hot, ghost, base, n, lights)
}

/// A PNG out of a `ColorImage` (RGBA8) — for the `thumb.png` inside a `.qpart`. Uses `image` with its png feature.
pub(crate) fn color_image_to_png(img: &egui::ColorImage) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let (w, h) = (img.size[0] as u32, img.size[1] as u32);
    let mut rgba: Vec<u8> = Vec::with_capacity(img.pixels.len() * 4);
    for p in &img.pixels {
        rgba.extend_from_slice(&p.to_array());
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(&rgba, w, h, image::ExtendedColorType::Rgba8).ok()?;
    Some(png)
}

/// VISIBILITY HAS CHANGED — DROP THE CACHE OF "WHAT WE SHOW".
///
/// Reported behaviour: a part is hidden, it is gone from 3D, and yet its edges and faces hang in the
/// air and can still be picked. The visibility rules had nothing to do with it: the list of visible
/// bodies is CACHED by the pair "rebuild plus context", and unticking a box changes neither. The
/// cache returned yesterday's answer, and both the highlighting and the picking followed it.
///
/// Called from EVERY place where visibility changes: the part's tick, the body's tick. Spread it
/// across those places and the next tick will forget again.
pub(super) fn visibility_changed(regen: &mut Rebuilding) {
    regen.geom_rev = regen.geom_rev.wrapping_add(1);
}

fn workbench_code(workbench: &Workbench) -> &'static str {
    workbench.code()
}

/// Did the document change outside an operation? (The boundary guard; pulled out so that a test can
/// check it rather than only a live run — that is precisely why the panic at start-up was not caught
/// by the tests.)
#[cfg(test)] // the guard is checked by a test; in production its role is played by the debt record itself
fn doc_changed_outside_edit(edits: &Edits, project: &Project) -> bool {
    edits.open.is_none() && doc_key(project) != edits.committed_key
}

/// A facade: the rebuild overlay WITH A LIVE CANVAS — by the same call the frame makes.
#[cfg(test)]
pub(crate) fn draw_regen_overlay_over(dc: &qymcad_ui_state::DrawCtx, ctx: &egui::Context, live: egui::Rect) -> bool {
    crate::gui::render::draw_dim_overlay_with(dc.scheme, ctx, &crate::i18n::tr("io-rebuilding"), Some(qymcad_ui_state::NodeCount { done: 1, total: 2 }), live)
}

/// Whether a multiple selection of components is active: the set holds more than one node AND the
/// current `sel` is a component from that set. Otherwise the set counts as stale and clears itself: a
/// click on a body or a sketch puts the multiple selection out without any explicit clean-up.
pub(crate) fn is_multi(project: &Project, sel: Sel, tree_sel: &TreeSelection) -> bool {
    tree_sel.multi.len() > 1 && matches!(sel, Sel::Component(ci) if project.components.get(ci).map(|c| tree_sel.multi.contains(&c.id)).unwrap_or(false))
}

/// ASK THE REBUILD TO STOP.
///
/// To ask, precisely: the thread will reach the boundary of the next node and come back by itself.
/// There is nothing to kill it with in the middle of an OCCT boolean, and no reason to — a COPY is
/// what is being computed, and the document on screen is intact. `finish_regen_checked` sums it up
/// when the result arrives marked as cancelled.
pub(crate) fn cancel_regen(regen: &Rebuilding, status: &mut qymcad_ui_state::Notice) {
    if let Some(p) = regen.busy.as_ref().and_then(|b| b.pulse.as_ref()) {
        p.ask_stop();
        *status = Notice::info(crate::i18n::tr("io-rebuild-cancelling"));
    }
}

/// Show or hide a component in 3D. Visibility is HIERARCHICAL: the flag is stored on the component
/// itself (`c.visible`), and a body's visibility is computed along the chain of its owners' ticks
/// (`component_chain_visible`). It does NOT cascade into `mesh_visible` — otherwise entering a hidden
/// subassembly would show everything hidden despite the ticks of the child parts being on
/// (`mesh_visible` means hiding an individual body by hand, and nothing else).
pub(crate) fn set_component_visible(project: &mut Project, regen: &mut Rebuilding, cid: Id, vis: bool) {
    project.set_component_visible(cid, vis);
    visibility_changed(regen);
}

/// Resolve the PLACEMENT plane (a shared step for a new sketch AND for an import).
/// A sketch on a face of ANOTHER component's body is a LIVE external reference (top-down): an
/// `ExternalRef` is registered and `Face(body, key)` is kept, so the sketch's frame resolves into the
/// consumer's local space on EVERY regeneration and travels with the neighbour's face (editing the
/// neighbour drives the part — exactly what in-context mode exists for). This used to take a ONE-OFF
/// snapshot of the face into a fixed datum — the part stayed free, but the top-down associativity was
/// lost silently. To break the link (freezing the geometry as a snapshot), use the part's properties
/// and its external references.
pub(crate) fn resolve_placement_plane(cmd: &mut FeatCommand, project: &mut Project, status: &mut Notice, plane: qymcad_core::feature::SketchPlane) -> qymcad_core::feature::SketchPlane {
    cmd.ref_body = None; // reset on every new placement
    if let qymcad_core::feature::SketchPlane::Face(body, key) = plane {
        let consumer = project.active_ctx();
        if project.body_owner(body).is_some_and(|bo| bo != consumer) {
            project.add_external_face_ref(consumer, body, key); // authorise the cross-reference (otherwise regeneration isolation blocks it)
                                                                // the source is remembered for the session: to highlight its edges and snap to them
            cmd.ref_body = Some(body);
            let src = project.body_owner(body).and_then(|o| project.components.iter().find(|c| c.id == o)).map(|c| crate::i18n::name(&c.name)).unwrap_or_else(|| crate::i18n::tr("g-neighbour"));
            *status = Notice::info(crate::i18n::tr1("g-sketch-on-foreign-face", "name", &src));
        }
    }
    plane
}

/// Create a pattern's datum AXIS from the straight edge `axis_edges[i]` — ASSOCIATIVELY, so it travels with the edge.
pub(crate) fn axis_from_edge(active_path: &[Id], edges: &EdgeCache, live: &LiveGeom, project: &mut Project, i: usize) -> Option<Id> {
    let &(body, edge, _) = edges.axes.get(i)?;
    axis_ref_world(active_path, edges, live, project, AxisHit::Edge(i))?; // check the geometry
    Some(project.add_axis_from_edge(body, edge))
}

/// Create a pattern's datum AXIS from the axis of a cylindrical or conical face — ASSOCIATIVELY, so it travels with the face.
pub(crate) fn axis_from_face(active_path: &[Id], edges: &EdgeCache, live: &LiveGeom, project: &mut Project, body: Id, face_id: u32) -> Option<Id> {
    axis_ref_world(active_path, edges, live, project, AxisHit::Face(body, face_id))?; // check that there is an axis at all
    Some(project.add_axis_from_face(body, face_id))
}

/// Recompute the pairs of bodies that INTERPENETRATE (interference) — lazily, while idle. Bodies of
/// different components that are visible in the context are transformed into the context's frame ONCE,
/// and then intersected pairwise for a common volume. It is expensive, so it runs only while
/// `show_interference` is on, the scene is not being dragged, and the cache is stale.
/// TAKE THE "MIRROR A WHOLE PART" TOOL, in the order that works: release the others FIRST.
///
/// The button used to push the release and set the field straight after. The release is carried out
/// AFTER the frame, so it cleared the field again and the button did nothing - the same shape as the
/// ruler that switched the measuring tool on and off within one frame.
///
/// Beside the application rather than on it: nothing here needs to see the whole of it.
fn mirror_part_door(app: &mut App, comp: Option<Id>) {
    // the button pressed again with the tool in hand puts it down, as every tool button does
    let held = app.params.mirror.in_hand();
    app.cancel_all_tools();
    if held {
        return;
    }
    // with no part picked before the button the tool is in hand all the same and waits for a click on one
    (app.params.mirror.part, app.params.mirror.waiting) = (comp, comp.is_none());
    app.status = Notice::info(crate::i18n::tr(if comp.is_some() { "tb-mirror-pick-plane" } else { "tb-mirror-pick-part" }));
}

pub(crate) fn refresh_interference(bv: qymcad_ui_state::BodyView, drag: qymcad_ui_state::SceneDrag, interference: &mut Interference, live: &LiveGeom, set: &Settings, workbench: Workbench) {
    if !set.show_interference || !matches!(workbench, Workbench::Assembly) {
        if !interference.pairs.is_empty() {
            interference.pairs.clear();
        }
        interference.rev = u64::MAX;
        return;
    }
    // the scene is moving (a gizmo drag), or the cache is fresh: nothing to recompute
    if qymcad_assembly::joint_drag_active(drag.joint, drag.part_pull) || drag.comp_giz.drag.is_some() || drag.body_giz.drag.is_some() {
        return;
    }
    if interference.rev == bv.regen.geom_rev {
        return;
    }
    let ctx = current_ctx_id(bv.active_path, bv.project);
    // the visible bodies that have an owning component, each where it stands in the context; the bridge asks the clash
    let placed: Vec<qymcad_kernel::ask::PlacedBody<'_>> = (0..bv.project.bodies.len())
        .filter(|&mi| body_shown(bv, mi))
        .filter_map(|mi| bv.project.mesh_id(mi))
        .filter_map(|body| {
            let part = bv.project.body_owner(body)?;
            let shape = live.shapes.get(&body)?;
            Some(qymcad_kernel::ask::PlacedBody { body, part, shape, place: bv.project.body_display_transform(body, ctx) })
        })
        .collect();
    // while this window rebuilds, the pairs as they were, asked again on a later frame; otherwise the kernel is waited for,
    // whoever holds it being quick
    let wait = if bv.regen.busy.is_some() { qymcad_kernel::ask::Wait::IfFree } else { qymcad_kernel::ask::Wait::Always };
    let Some(clashes) = qymcad_kernel::ask::clashes(&placed, wait) else { return };
    interference.pairs = clashes.into_iter().map(|c| (c.a, c.b)).collect();
    interference.rev = bv.regen.geom_rev;
}

#[cfg(test)]
/// The checks that the INTERFACE goes through the dictionary. They stayed with the application when the
/// dictionary moved out: what they watch is the panels, not the catalogue.
#[cfg(test)]
mod i18n_use_tests;
/// The thumbnails of the built-in library, drawn again by hand: `gui/library_thumbs.rs`.
mod library_thumbs;
/// The preview of a fillet on the private sample of a frame: `gui/blend_preview_on_sample.rs`.
mod blend_preview_on_sample;
/// The trial build on a worker: `gui/trial_in_background.rs`.
mod trial_in_background;
#[cfg(test)]
mod scheme_use_tests;
mod audit;
mod behaviour_sweep;
mod described_picks;
/// A fillet, a chamfer or a patch picked by a face reopens with that face: `gui/a_face_pick_reopens_with_its_face.rs`.
mod a_face_pick_reopens_with_its_face;
/// A fillet with a radius at a corner reopens with that corner: `gui/a_variable_fillet_reopens_with_its_corners.rs`.
mod a_variable_fillet_reopens_with_its_corners;
mod a_fillet_with_its_edges_lost_waits_for_edges;
mod expand_selection;
mod sketch_sweep;
mod input;
mod bg_rebuild;
mod focus_keys;
mod frame_cost;
mod grab_tests;
mod pick_at_zero;
mod pinned_points;
mod the_move_tool_is_clicked;
mod a_shape_is_finished_by_hand;
mod a_sketch_chamfer_takes_two_values;
mod a_sketch_fillet_by_chord_or_arc;
mod a_rectangle_is_dragged_from_its_anchor;
mod a_construction_line_is_no_side_of_a_corner;
mod the_corner_box_stands_clear_of_the_corner;
mod a_rounded_rectangle_turns_whole;
mod a_cut_corner_draws_clean;
mod no_ties_shows_no_tie;
mod the_corner_of_a_crossing_follows_the_pointer;
mod the_offset_takes_its_chain_either_way;
mod the_mirror_picks_as_every_tool;
mod the_dimensions_of_a_cut_corner_stand_outside;
mod a_corner_set_takes_its_size_whole;
mod the_box_of_fillet_all_stands_at_the_clicked_side;
mod every_field_of_a_pattern_bar_has_a_caption;
mod the_x_key_turns_the_selection;
mod a_shape_goes_with_its_centre;
mod diagnostics_wait_for_the_release;
mod the_sketch_says_what_it_did_not_solve_in_time;
mod a_big_drawing_stays_live;
mod a_pattern_across_a_pattern_is_laid_by_hand;
mod the_sketch_of_the_report_takes_a_line;
mod a_big_sketch_selected_keeps_the_3d_frame;
mod a_dimension_past_its_field_is_left_as_it_stands;
mod the_point_numbers_wait_for_their_setting;
mod graphics_settings;
mod a_drawing_tool_drops_the_selection;
mod a_circle_and_an_arc_take_a_size_only_when_typed;
mod a_rectangle_with_its_centre_fixed_is_worked_by_hand;
mod a_copy_goes_to_the_clipboard;
mod extend_is_worked_by_hand;
mod a_projection_is_sketch_geometry;
mod a_point_put_on_a_line_is_tied_to_it;
mod a_projection_follows_the_parameters;
mod an_arc_by_its_centre_ends_on_itself;
mod a_shift_box_leaves_the_sheet_still;
mod a_dimension_being_placed_follows_the_pointer;
mod the_dimensions_turn_with_their_rectangle;
mod an_automatic_parallel_stays_in_its_shape;
mod the_chain_being_drawn_marks_its_own_corners;
mod a_side_deleted_leaves_the_rectangle_lines_held;
mod the_middle_of_a_line_is_picked;
mod a_line_made_level_turns_about_its_middle;
mod a_drag_takes_what_was_pressed;
mod text_font;
mod font_row_look;
mod paths_are_one;
pub(crate) mod scene_chunks;
mod scene_cost_look;
mod joint_creation;
mod joint_flow;
mod confirm_once;
mod contours_visible;
mod dof_line;
mod expr_errors;
mod expr_fields;
mod driver_autocomplete;
mod copy_puts_the_tool_down;
mod thread_runout_look;
mod the_view_belongs_to_the_person;
mod escape_with_an_open_list;
mod the_cancel_button_cancels;
mod the_sketch_is_left_by_ctrl_enter;
mod the_two_answers_are_told_apart;
mod what_the_program_opens_with;
mod contours_are_chosen_before_the_size;
mod a_tool_that_needs_a_sketch_asks_for_one;
mod the_sketch_mirror_asks_about_what;
mod which_button_moves_the_view;
mod hints_are_readable;
mod renaming_starts_with_f2;
mod the_scale_is_stepped_not_dragged;
mod the_camera_finds_the_model_on_opening;
mod the_wheel_zooms_where_you_point;
mod the_part_menu_can_delete;
mod expr_field_behaviour;
mod params_table_gesture;
mod dim_popup_gesture;
mod feature_driver_gesture;
mod lost_refs_on_edit;
mod hidden_is_not_pickable;
mod joint_pick_highlight;
mod a_broken_joint_says_so;
mod a_group_is_made_by_hand;
mod a_width_is_made_by_hand;
mod a_tangent_is_made_by_hand;
mod a_relation_is_made_by_hand;
mod a_joint_can_hold_what_it_finds;
mod a_mechanism_can_be_watched_moving;
mod the_mate_window_stands_still_while_it_runs;
mod a_window_keeps_its_place_across_languages;
mod the_angle_field_keeps_its_width;
mod a_connector_stands_on_its_own;
mod the_gizmo_pulls_where_it_points;
mod an_anchor_belongs_to_the_part_under_the_cursor;
mod an_edited_joint_shows_its_anchors;
mod changing_an_anchor_does_not_spin_the_part;
mod rebuilding_everything_asks_for_geometry_again;
mod f1_answers_for_every_assembly_tool;
mod no_raw_keys_on_the_assembly_screen;
mod a_dead_mate_can_be_revived;
mod a_mate_takes_the_face_you_click;
mod one_list_of_mates;
mod the_mate_hud;
mod limits_hold_and_say_so;
mod a_part_can_be_dragged;
mod a_mechanism_can_be_run;
mod an_anchor_on_a_moving_part_is_refused;
mod no_widget_id_clashes;
mod every_assembly_tool_speaks;
mod every_picking_tool_highlights;
mod no_silent_refusal_on_click;
mod escape_drops_every_assembly_tool;
mod ask_then_write;
mod draw_tools_are_mutually_exclusive;
mod tools_are_mutually_exclusive;
mod no_raw_keys_in_assembly;
mod a_slider_between_facing_faces;
mod a_stuck_assembly_says_so;
mod dragging_a_joint_does_not_flicker_the_rebuild_modal;
mod a_chain_of_parts_follows_the_hand;
mod a_global_mate_is_dragged_by_its_part;
mod preparing_the_brep_never_loops;
mod a_failed_rebuild_keeps_the_brep;
mod an_export_that_panics_keeps_the_brep;
mod dragging_a_part_pulls_the_whole_chain;
mod a_part_is_dragged_by_real_mouse;
mod a_moved_part_does_not_rebuild_its_block;

mod an_edge_anchor_survives_reopening;
mod every_kind_is_made_by_hand;
mod every_relation_is_made_by_hand;
mod joint_limits_are_visible;
mod saving_is_not_silent;
mod tree_drag;
mod param_fields_stretch;
mod interference;
mod no_assembly_sketch;
mod one_extrude;
mod one_node_per_command;
mod open_keeps_bodies;
mod props_readonly;
mod recent_files;
mod redundant_flag;
mod reveal_folder;
mod regen_cancel;
mod root_name;
mod start_screen;
mod start_screen_tests;
mod delete_feature_view;
mod edit_keeps_body;
mod doc_props;
mod catalog_flow;
mod comp_array_flow;
mod help_flow;
mod help_general;
mod help_images;
mod help_raster;
mod help_map_flow;
mod help_settings;
mod help_window;
mod hotkeys;
mod hotkeys_rebind;
mod perspective_cull;
mod power_trim_flow;
mod scheme_flow;
mod search_flow;
mod screen_keys;
mod settings_sections_tests;
mod sketch_paint;
mod sketch_reopen;
mod settings_memory;
mod settings_profile;
mod measure3d;
mod dim_drag_mouse;
mod gizmo_mouse;
mod long_values;
mod modal_barrier;
mod open_keeps_brep;
mod regen_live_view;
mod the_program_ends_during_a_rebuild;
mod the_program_ends_during_a_trial;
mod recognise_then_delete;
mod extrude_arrow_mouse;
mod angle_dim_drag;
mod dim_text_drag;
mod measure_flow;
mod projection_flow;
mod push_face_flow;
mod split_flow;
mod templates_flow;
mod fillet_vertex_flow;
mod thicken_flow;
mod loft_surface_flow;
mod patch_flow;
mod stitch_flow;
mod hand;
mod layer_b;
mod trim_flow;
mod user_case;
mod tool_popup_sweep;
mod view_cache_sweep;
mod remove_face_flow;
mod sketch_ref;
mod view_state;
mod viewcube;
mod viewcube_flow;
mod fuzz;
mod geom_quality;
mod toolbar_dupes;
mod tree_row_identity;
mod tree_search;
mod tuned_constants;
mod ui_sweep;
mod unsaved_prompt;
mod clipped_text_sweep;
mod antialiasing_off_draws;
mod param_error_readable;
mod ghost_highlight;
mod look_at_a_document;
mod gpu_shot;
mod the_region_picker_keeps_its_command;
mod a_scene_lost_in_a_discarded_pass_is_sent_again;
mod smooth_shading_look;
mod card_matches_raster;
mod drag_cost_look;
mod release_build;
mod about_build_line;
mod crash_notice;
mod tests;

/// Picking and hit-testing live in `gui/pick.rs`.
mod report_problem;
mod window_title;

mod pick;

/// The axis of a circular pattern picked in the view lives in `gui/array_axis.rs`.
mod array_axis;

/// Drawing the scene and the overlays lives in `gui/render.rs`.
mod render;
mod rebuild_sign;

/// The Part commands (starting them, their parameters, applying them) live in `gui/commands.rs`.
mod command_search;
mod commands;

/// The assembly joints live in `gui/joints.rs`.
mod joints;

/// Inferring the anchor under the cursor lives in `gui/mate_infer.rs`.
mod mate_infer;

/// THE ONE list of assembly tools that everyone reads lives in `gui/assembly_tools.rs`.
mod assembly_tools;

/// The panels (the tree, the properties) live in `gui/panels.rs`.
mod panels_bars;
pub(crate) mod panels_source;
pub(crate) mod render_source;
pub(crate) mod sketch_source;
mod render_scene;
mod viewport_3d;
mod panels_props;
mod panels_tree;
mod panels_windows;
pub(crate) mod icon_themes;

/// The single expression field and its list of drivers. One for the whole project, so that a dimension's
/// field, a feature's field and a table cell all behave the same way.
mod expr_field;

/// The single header of a properties card lives in `gui/props_card.rs`.
mod props_card;
mod props_pick;
mod frame_select;
mod piece_part;
mod look_at_point;
mod orbit_about;
pub(crate) use qymcad_ui_state::{props_header, NameSlot, Notice};

/// The sketcher (geometry, dimensions, constraints) lives in `gui/sketching.rs`.
mod sketching;

/// Files and background jobs live in `gui/io_jobs.rs`.
mod io_jobs;

/// Asking for a file without stopping the frames lives in `gui/file_ask.rs`.
mod file_ask;
/// Where an export chooser opens lives in `gui/export_folder.rs`.
mod export_folder;
/// Whole frames without a screen, for the hand of the checks and for a session, live in `gui/window.rs`.
mod window;
mod key_leak;
/// The program driven from outside lives in `gui/session.rs`.
pub(crate) mod session;
mod export_menu;
mod bar_menu;
mod import_scale;
mod format_samples_look;
mod import_door;
mod an_iges_import_survives_reopening;
mod an_import_keeps_its_names_on_opening;
mod a_step_assembly_lands_as_its_tree;
mod a_component_stepped_into_is_not_lit;
mod a_broken_part_is_no_ghost_elsewhere;
mod a_part_is_cloned_from_the_tree;
mod an_imported_assembly_is_worked_by_hand;
mod a_face_of_its_own_colour_is_drawn;
mod a_coloured_ply_comes_in_coloured;
mod a_mesh_edge_stays_sharp;

/// Whether a newer version exists lives in `gui/update_ui.rs` - state and all, off the application.
mod update_ui;
mod update_notice;
mod dim_to_axis;
mod deaf_while_the_system_asks;
mod a_machine_with_no_graphics;
mod the_card_holds_its_words;

mod startup;
pub use startup::*;
mod file_doors;
pub(crate) use file_doors::*;
mod look;
pub(crate) use look::*;
mod gizmo_math;
pub(crate) use gizmo_math::*;
mod view_anim;
pub(crate) use view_anim::*;
mod panel_bits;
pub(crate) use panel_bits::*;
