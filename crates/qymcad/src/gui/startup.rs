//! THE START OF THE PROGRAM: its names and times, the commands of the system, the adapter of the graphics chosen, the window launched.

use qymcad_ui_state::Notice;
use super::*;

/// THE APPLICATION'S NAME for eframe. It is also the key of the directory eframe puts the settings file into, so
/// it lives as one constant: were those two places to diverge, the settings window would show the path of a
/// directory that does not exist.
pub(crate) const APP_ID: &str = "qymcad";

/// THE NAME A PERSON READS, as against the identifier above that a machine stores things under. The
/// window bar, the task bar and the About window all say this one.
pub(crate) const APP_NAME: &str = "QymCAD";

/// The window icon, decoded from the 256x256 PNG built into the binary.
///
/// The same image the splash screen shows. A window with no icon of its own is given the framework's,
/// and a program that wears someone else's face in the task bar looks like a stray tool rather than the
/// one that was installed.
pub(crate) fn app_icon() -> egui::IconData {
    const ICON: &[u8] = include_bytes!("../../../../assets/icons/linux/256x256.png");
    match image::load_from_memory(ICON) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData { rgba: rgba.into_raw(), width: w, height: h }
        }
        // An icon is worth nothing next to starting at all: a window without one still works.
        Err(_) => egui::IconData::default(),
    }
}

/// THE CURRENT TIME IN ISO-8601 (UTC, to the second).
///
/// By its own arithmetic, without a date crate: exactly one string is needed in the whole project, and a
/// dependency for it would have to be carried, updated and explained. The format chosen is machine-readable and
/// sortable - whoever wants to show it to a person may, but what is stored must be unambiguous.
pub(crate) fn now_iso8601() -> String {
    iso8601_from_unix(unix_secs())
}

/// SECONDS SINCE THE EPOCH, in one place.
///
/// The same incantation stood in four files - the name of a crash file, the name of a report directory,
/// the stamp of a document and the autosave clock - each spelling out its own fall back to zero.
pub(crate) fn unix_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// CONVERTING unix SECONDS INTO ISO-8601. Separate from "what time is it", because only this part is testable:
/// the current time has nothing to be compared against in a test, while a known stamp has.
pub(crate) fn iso8601_from_unix(secs: u64) -> String {
    // THE LAYOUT IS BUILT HERE, THE CALENDAR IS NOT. Leap years, the rule of centuries and the length of
    // February are a solved problem; the layout is four numbers and two separators and is held by the table
    // of known stamps beside it. Only the hard half was handed over.
    let t = time::OffsetDateTime::from_unix_timestamp(secs as i64).unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", t.year(), u8::from(t.month()), t.day(), t.hour(), t.minute(), t.second())
}

/// The folder a person's own files are in - what the "open the settings folder" button opens.
pub(crate) fn settings_dir() -> Option<std::path::PathBuf> {
    qymcad_paths::data_root()
}

/// WHAT OPENS A FOLDER - as a separate PURE function rather than a line inside a click handler.
///
/// Every system has its own file manager, and there is no other way to check this: a real process must not be
/// started in a test, and "it works on my machine" proves nothing about the other two. The function answers WHAT
/// will be launched; a test compares that answer for all three systems, and the click simply carries it out.
pub(crate) fn reveal_command(os: egui::os::OperatingSystem, dir: &std::path::Path) -> (&'static str, Vec<String>) {
    open_command(os, dir.to_string_lossy().into_owned())
}

/// WHAT OPENS AN ADDRESS IN A BROWSER. The same way as for a folder, and that is no coincidence: on all three
/// systems "open this with whatever is proper" is one and the same command, which works it out for itself.
pub(crate) fn browse_command(os: egui::os::OperatingSystem, url: &str) -> (&'static str, Vec<String>) {
    open_command(os, url.to_string())
}

pub(crate) fn open_command(os: egui::os::OperatingSystem, arg: String) -> (&'static str, Vec<String>) {
    use egui::os::OperatingSystem as OS;
    match os {
        OS::Windows => ("explorer", vec![arg]),
        OS::Mac => ("open", vec![arg]),
        // Linux, BSD and everything else: `xdg-open` is the shared way across desktop environments
        _ => ("xdg-open", vec![arg]),
    }
}

/// WHAT EVERY START OF THE PROGRAM SAYS TO THE PROCESS FIRST, once: the window's and a session's alike.
pub(crate) fn prepare_the_process() {
    // EVERY SAVED FILE SAYS WHAT WROTE IT. The core is a library and has no build of its own to name, so
    // the application tells it once here, before anything can save.
    qymcad_core::model::set_producer(crate::build_info::report_block().lines().next().unwrap_or_default());
    // THE BREADCRUMB SINK. Opening an edit records what a person just did; the crate of records does not
    // know what a crash report is, so the application plugs its trail in here, once.
    qymcad_ui_state::set_step_recorder(crate::crash::note_step);
}

/// THE PROGRAM AS IT COMES UP, before the first frame: fonts, the settings a previous run kept in `storage`,
/// what to open with, the view. One function for the window and for a session driven from outside, so a
/// session starts exactly the program a person starts; the window adds only the card that draws.
pub(crate) fn start_the_program(ctx: &egui::Context, storage: Option<&dyn eframe::Storage>) -> App {
    // the Phosphor icon font (without it the icon glyphs render as empty boxes)
    install_fonts(ctx);

    let mut app = App::default();
    // A CRASH FILE NOBODY IS TOLD ABOUT IS THE SAME AS NO CRASH FILE. EVERY unseen report from
    // earlier runs is picked up here, and one window answers for all of them: taking only the
    // newest meant the next start found the next one and showed the window again.
    app.disk.crash_report = crate::crash::unseen_reports();
    // restore the previous session's settings (the view preferences)
    if let Some(storage) = storage {
        // THE SETTINGS COME AS ONE RECORD: loaded and assigned. There is no separate "apply" step, and
        // there is nowhere to lose a setting along the way - except the theme, which egui does not remember itself.
        let stored = eframe::get_value::<Settings>(storage, "settings");
        let first_start = stored.is_none(); // nothing kept from an earlier run: the program opens for the first time
        if let Some(v) = stored {
            adopt_settings(&mut app.regen, &mut app.scheme, &mut app.set, &mut app.status, v, ctx);
        } else {
            let factory = app.set.clone(); // the factory defaults: the language from the locale, the theme, the scale
            adopt_settings(&mut app.regen, &mut app.scheme, &mut app.set, &mut app.status, factory, ctx);
        }
        // the previous session's project is reopened LAZILY: the splash with the logo is shown first (the
        // opening frames), and only then does the loading start, so a heavy STEP reparse does not hold an
        // empty window.
        // WHAT TO OPEN WITH is decided by `qymcad_ui_state::opening`, out of the settings, rather
        // than here. The decision used to be "always reopen the last project" with nothing to say
        // otherwise, and it lived in this closure - which no check can run, so it was a decision
        // nobody measured.
        let last = eframe::get_value::<Option<String>>(storage, "last_project").flatten();
        match qymcad_ui_state::opening(&app.set, last.as_deref()) {
            qymcad_ui_state::Opening::LastProject(path) => {
                app.disk.io.startup = Some(path); // loaded asynchronously on the first frame (the splash with the spinner)
            }
            qymcad_ui_state::Opening::Empty { start_screen } => {
                // AN EMPTY DOCUMENT IS EMPTY: the root and nothing else. `App::default` hands out a
                // document with a part in it, which is right for a check that needs somewhere to
                // draw and wrong for the first minute of a person - reported as "a person opens the
                // CAD and sees an empty part".
                // A FIRST START MEETS A PERSON WITH A CUBE: a part of a sketch and its extrusion, to step into and see
                // how the work goes here (decided 25.09). Later an empty document is empty - a fresh one, not the
                // default's part left standing under a root made active.
                app.project = Project::default();
                if first_start {
                    app.project.cube_sample();
                } else {
                    app.project.new_empty_document();
                }
                // the keys of "saved" and "committed" were taken of the document `App::default` made; the empty one
                // is another document, and without taking them again a fresh start called itself unsaved
                app.disk.edits.committed_key = doc_key(&app.project);
                app.disk.edits.saved_key = edit_key(&app.draw_ctx());
                app.win.show_start(start_screen);
            }
        }
    }
    // The start is ALWAYS isometric: 3D mode with an isometric camera (init=false means it frames itself to
    // the contents on the first frame while keeping the orientation). Independent of the previous session.
    app.viewing.mode_3d = true;
    app.viewing.cam = Cam3::default();
    app
}

pub fn launch() -> eframe::Result<()> {
    prepare_the_process();
    // AND THE CRASH STOPS DISAPPEARING. Installed before anything can panic: an autosave can fire in the very
    // first seconds of a session.
    crate::crash::install();
    // THE JOURNAL OF THIS RUN, and a report of the one before when it stopped without closing
    let previous = crate::crash::begin_journal();
    // A START AFTER A START THAT DIED DRAWING TAKES A STEP DOWN (`safe_graphics`)
    let step = crate::safe_graphics::take(previous.journal.as_deref());

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            // WITHOUT THESE THE WINDOW BORROWS SOMEBODY ELSE'S FACE: the framework's own icon and the
            // storage identifier `qymcad` as a title. Both are what a person sees first, in a task bar
            // beside three other windows, and neither said anything about this program or its document.
            .with_title(APP_NAME)
            .with_icon(app_icon()),
        // the GPU viewport: egui on wgpu gives access to wgpu_render_state for the bodies' 3D paint callback.
        // glow stays compiled in as a fallback (the renderer is switched here).
        renderer: eframe::Renderer::Wgpu,
        // WHERE THE SETTINGS ARE KEPT IS OUR DECISION, not the framework's. Left to itself it derives a
        // folder from the application id and keeps a person's settings apart from their schemes, templates,
        // parts and crash reports - measured on Windows as `AppData\Roaming\qymcad\data` beside
        // `AppData\Roaming\qymis\qymcad\data`, and on macOS as `qymcad` beside `tech.qymis.qymcad`.
        persistence_path: qymcad_paths::settings_file(),
        ..Default::default()
    };
    choose_the_adapter_ourselves(&mut options);
    options.wgpu_options.on_surface_status = crate::crash::journaled_surface_status(options.wgpu_options.on_surface_status.clone());
    let started = eframe::run_native(
        APP_ID,
        options,
        Box::new(|cc| {
            let mut app = start_the_program(&cc.egui_ctx, cc.storage);
            // the GPU viewport: when the wgpu backend is active, the bodies pass's GPU resources are installed.
            // Otherwise (on the glow fallback) the CPU raster remains.
            if let Some(rs) = &cc.wgpu_render_state {
                // ANTIALIASING COMES BEFORE THE PIPELINES ARE CREATED: they bake the sample count into themselves,
                // so the setting takes effect on a restart (which is what the window says).
                crate::viewport_gpu::set_msaa(crate::safe_graphics::samples(app.set.msaa));
                app.gpu_ok = crate::viewport_gpu::install(rs);
                crate::crash::watch_the_device(&rs.device);
                // WHICH ADAPTER IS DRAWING - half the complaints about a viewport are answered by this line
                // and by nothing else, and it cannot be guessed from a screenshot.
                let i = rs.adapter.get_info();
                crate::diagnostics::note_gpu(format!("wgpu {:?}, {} ({:?}), driver {} {}", i.backend, i.name, i.device_type, i.driver, i.driver_info));
            } else {
                // The glow fallback: no adapter to ask, but WHICH PATH is drawing is itself the answer to
                // "the viewport is slow" and "the viewport is black".
                crate::diagnostics::note_gpu("glow (the CPU fallback path)".into());
            }
            // DRAWING ON THE PROCESSOR IS SAID OUT LOUD, once, in the status line. Everything works and
            // nothing is broken - but turning a model feels like wading, and a person owed no explanation
            // reports it as a fault in the program rather than as a graphics driver they have not installed.
            if let Some(name) = crate::diagnostics::drawing_on_the_processor() {
                app.status = Notice::warning(crate::i18n::tr1("gpu-on-the-processor", "name", &name));
            }
            // A STEP DOWN IS SAID, with why: otherwise the person sees a rougher picture and no reason for it
            if let Some(key) = crate::safe_graphics::said(step) {
                app.status = Notice::warning(crate::i18n::tr(key));
            }
            Ok(Box::new(app))
        }),
    );
    // A START THAT NEVER HAPPENED USED TO LEAVE NOTHING BEHIND. The framework hands this back as an
    // ordinary error rather than a panic, so the crash hook never saw it, and the message went to standard
    // error - which a windowed build on Windows has nowhere to print to. What a person saw was a window
    // that blinked and closed, and the crash folder was empty.
    if let Err(e) = &started {
        let path = crate::crash::note_failed_start(&e.to_string());
        crate::diagnostics::note_start_failure(&e.to_string(), path.as_deref());
    }
    qymcad_ui_state::icons::stop_all_watcher_threads();
    icon_themes::discovery::stop_all_discovery_workers();
    crate::crash::end_journal();
    started
}

/// WHICH CARD DRAWS, DECIDED HERE AND WRITTEN DOWN.
///
/// Left to itself the framework asks for an adapter and, finding none, fails with an error nobody sees.
/// Two things are wanted instead, and both need the list of adapters in hand: every one of them recorded
/// (a report from a machine that would not start is otherwise a blank page), and the ONE ON THE PROCESSOR
/// accepted rather than refused when it is all there is.
///
/// Reported behaviour: on Windows 11 with an old card and no driver installed the program would not start;
/// there was no crash file and no falling back to drawing on the processor either. On the same version a
/// virtual machine and a modern card both started without trouble.
///
/// Software rendering is slow, and on a big assembly it is very slow - but it is a program that runs and
/// says why it is slow, against one that closes without a word.
pub(crate) fn choose_the_adapter_ourselves(options: &mut eframe::NativeOptions) {
    use eframe::wgpu;
    // THE FRAMEWORK'S OWN SETTINGS ARE KEPT and only two of them changed. Building the record from scratch
    // would drop the display handle it fills in for us, which some systems will not open a window without.
    let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup else { return };
    // THE BACKENDS ARE LEFT AS THEY COME: the framework already asks for the primary ones plus GL, and on
    // a desktop there is nothing else to add. What changes here is WHICH of the adapters they enumerate is
    // taken, and that the list is written down either way.
    setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>| {
        let seen: Vec<String> = adapters.iter().map(describe_adapter).collect();
        crate::diagnostics::note_adapters(&seen);
        // a card that cannot draw into THIS window is no use, whatever else it can do
        // THE ADAPTERS THAT CAN DRAW INTO THIS WINDOW, best first, and the first of them that really draws: a card can
        // open, take every command and draw a transparent picture (`viewport_gpu::draws`)
        let mut fit: Vec<&wgpu::Adapter> = adapters.iter().filter(|a| surface.is_none_or(|s| a.is_surface_supported(s))).collect();
        let wanted = crate::safe_graphics::wanted_adapter();
        fit.sort_by_key(|a| std::cmp::Reverse(rank_adapter(a, wanted.as_deref())));
        let choice = crate::safe_graphics::first_that_draws(fit.len(), |i, samples| self_test(fit[i], samples));
        crate::safe_graphics::take_choice(choice);
        let best = fit.get(choice.index).copied();
        match best {
            Some(a) => {
                let info = a.get_info();
                crate::diagnostics::note_gpu(format!("wgpu {:?}, {} ({:?}), driver {} {}", info.backend, info.name, info.device_type, info.driver, info.driver_info));
                // A CPU ADAPTER IS A WORKING PROGRAM, and the person is told rather than left to wonder
                // why turning a model feels like wading.
                if info.device_type == wgpu::DeviceType::Cpu {
                    crate::diagnostics::note_drawing_on_the_processor(&info.name);
                }
                Ok(a.clone())
            }
            // THE MESSAGE IS THE REPORT. This string is what reaches the file and the message box, so it
            // says what was looked for and what was found rather than "no suitable adapter".
            None => Err({
                crate::diagnostics::note_no_adapter();
                format!(
                    "no graphics adapter this window can draw on. Offered {}: {}",
                    adapters.len(),
                    if seen.is_empty() { "none at all - the machine has no working graphics driver".to_string() } else { seen.join("; ") }
                )
            }),
        }
    }));
}

/// THE SELF-TEST OF ONE ADAPTER with `samples` a pixel, its answer written into the journal.
pub(crate) fn self_test(adapter: &eframe::wgpu::Adapter, samples: u32) -> bool {
    let began = std::time::Instant::now();
    let drew = pollster::block_on(adapter.request_device(&eframe::wgpu::DeviceDescriptor::default())).is_ok_and(|(device, queue)| crate::viewport_gpu::draws(&device, &queue, samples));
    crate::crash::journal(&format!("self-test: {} with {samples} sample(s): {} in {} ms", describe_adapter(adapter), if drew { "drew" } else { "drew nothing" }, began.elapsed().as_millis()));
    drew
}

/// One adapter in one line, for the report.
pub(crate) fn describe_adapter(a: &eframe::wgpu::Adapter) -> String {
    let i = a.get_info();
    format!("{:?}/{:?} {} (driver {} {})", i.backend, i.device_type, i.name, i.driver, i.driver_info)
}

/// HOW GOOD AN ADAPTER IS, highest first. A real card beats a shared one, a shared one beats a virtual one,
/// and everything beats the processor - which is taken only when nothing else answers; the step of a start after a
/// start that died moves the backend and, at the last, the processor (`safe_graphics::rank`).
pub(crate) fn rank_adapter(a: &eframe::wgpu::Adapter, wanted: Option<&str>) -> u16 {
    let i = a.get_info();
    let offered = crate::safe_graphics::Offered { kind: i.device_type, backend: i.backend, wanted: wanted == Some(describe_adapter(a).as_str()) };
    crate::safe_graphics::rank(crate::safe_graphics::step(), offered, crate::safe_graphics::avoided().as_deref())
}

/// The ranking itself, apart from any adapter: a machine with no graphics is exactly the machine a test
/// cannot be run on, so the order is checked here instead.
///
/// NOTHING SCORES ZERO. The processor is the worst choice and the one that must still be TAKEN when it is
/// the only one - refusing it is the program that would not start.
pub(crate) fn rank_device_type(t: eframe::wgpu::DeviceType) -> u8 {
    use eframe::wgpu::DeviceType;
    match t {
        DeviceType::DiscreteGpu => 5,
        DeviceType::IntegratedGpu => 4,
        DeviceType::VirtualGpu => 3,
        DeviceType::Other => 2,
        DeviceType::Cpu => 1,
    }
}
