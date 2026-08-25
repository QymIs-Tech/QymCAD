//! WHAT THE MACHINE WAS, when the trouble happened.
//!
//! Half of the complaints about a viewport - black, torn, upside down - are answered by the graphics
//! adapter and the drawing path and by nothing else, and neither can be guessed from a screenshot. The
//! rest of it is cheap to collect and expensive to ask for afterwards: asking costs a round trip to a
//! person who has already moved on.
//!
//! Everything here is gathered once and kept in globals rather than read from `App`, because the other
//! reader is the panic hook: it runs while the frame still holds the application and may not reach it.
//!
//! THE BLOCK IS IN ENGLISH whatever language the window speaks. It is read by whoever picks the report
//! up, not by whoever sends it.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// The graphics adapter and the drawing path, as one line. Empty until the window has started.
static GPU: Mutex<Option<String>> = Mutex::new(None);

/// The canvas, in points, and the scale - three numbers, so no allocation happens per frame.
static VIEW_W: AtomicU32 = AtomicU32::new(0);
static VIEW_H: AtomicU32 = AtomicU32::new(0);
/// The scale times a hundred: an integer, because atomics hold no floats.
static VIEW_PPP: AtomicU32 = AtomicU32::new(0);

/// WHICH ADAPTER IS DRAWING. Called once, when the window has chosen its backend.
pub fn note_gpu(line: String) {
    if let Ok(mut g) = GPU.lock() {
        *g = Some(line);
    }
}

/// The canvas of the current frame. Called every frame, so it costs three atomic stores and nothing
/// else - a string built per frame would be measurable at this rate.
pub fn note_viewport(size: egui::Vec2, points_per_pixel: f32) {
    VIEW_W.store(size.x as u32, Ordering::Relaxed);
    VIEW_H.store(size.y as u32, Ordering::Relaxed);
    VIEW_PPP.store((points_per_pixel * 100.0) as u32, Ordering::Relaxed);
}

/// The name of the system, as its own people write it: `Ubuntu 24.04.1 LTS`.
///
/// Read once from `/etc/os-release`, which every current Linux distribution carries. Elsewhere there is
/// no such file and no equally cheap answer, so the family and the architecture are all this says - and
/// saying less honestly beats naming a version nobody measured.
fn system() -> String {
    static ONCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let arch = std::env::consts::ARCH;
        let pretty = std::fs::read_to_string("/etc/os-release").ok().and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string()))
                .filter(|v| !v.is_empty())
        });
        match pretty {
            Some(name) => format!("{name} ({} {arch})", std::env::consts::OS),
            None => format!("{} {arch}", std::env::consts::OS),
        }
    })
    .clone()
}

/// THE BLOCK A REPORT CARRIES. Every line answers a question that would otherwise cost a conversation.
pub fn block() -> String {
    let mut s = crate::build_info::report_block();
    s.push_str(&format!("\nSystem: {}\n", system()));

    let gpu = GPU.lock().ok().and_then(|g| g.clone()).unwrap_or_else(|| "(the window has not started)".into());
    s.push_str(&format!("Graphics: {gpu}\n"));

    let (w, h) = (VIEW_W.load(Ordering::Relaxed), VIEW_H.load(Ordering::Relaxed));
    let ppp = VIEW_PPP.load(Ordering::Relaxed) as f32 / 100.0;
    if w > 0 && h > 0 {
        s.push_str(&format!("Window: {w}x{h} at {ppp:.2}x\n"));
    }

    s.push_str(&format!("Language: {}\n", crate::i18n::language()));

    // The kernel's own words about the last thing it refused. Untranslated and naming internals - which
    // is why they belong here and never in a window.
    if let Some(why) = qymcad_kernel::refusal_for_report() {
        s.push_str(&format!("Last kernel refusal: {why}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    /// THE BLOCK ANSWERS THE QUESTIONS IT EXISTS FOR, and carries nothing that names a person.
    #[test]
    fn the_block_says_what_the_machine_is() {
        super::note_gpu("wgpu Vulkan, NVIDIA GeForce RTX 3060".into());
        super::note_viewport(egui::vec2(1280.0, 800.0), 1.5);

        let b = super::block();
        assert!(b.starts_with("QymCAD "), "the block does not name the program: {b}");
        assert!(b.contains("System: "), "the block does not name the system: {b}");
        assert!(b.contains("Graphics: wgpu Vulkan"), "the block does not name the adapter: {b}");
        assert!(b.contains("Window: 1280x800 at 1.50x"), "the block does not name the canvas: {b}");
        assert!(b.contains("Language: "), "the block does not name the language: {b}");

        // IT GOES INTO A PUBLIC TRACKER. A home directory carries the name of whoever ran the program.
        let home = directories::UserDirs::new().expect("a home directory").home_dir().to_string_lossy().into_owned();
        assert!(!b.contains(&home), "the block carries a personal path ({home}): {b}");
    }

    /// Before the window starts there is no adapter, and the block must still be a block rather than a
    /// hole: the panic hook can fire on the first second of a run.
    #[test]
    fn it_is_whole_before_the_window_starts() {
        // Deliberately without `note_gpu`: whichever test runs first, the line has to read as words.
        let b = super::block();
        assert!(b.contains("Graphics: "), "the block lost the graphics line: {b}");
        assert!(b.lines().count() >= 4, "the block came out empty: {b}");
    }
}
