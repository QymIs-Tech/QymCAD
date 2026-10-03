//! A CLICK LEAVES A TRAIL, and the trail says how deep it went.
//!
//! A program that closes itself without saying where leaves one question and no way to answer it. This is
//! what answers it: the name of every function a click entered, indented by how deep, written to a file
//! beside the program.
#[cfg(test)]
mod tests {
use super::super::*;

/// THE FOLDER THE CHECK'S OWN TRACE GOES INTO, so a run of the checks leaves nothing beside the binaries.
fn trace_folder(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("qymcad-trace-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder of its own for the trace");
    dir
}

/// ONE CLICK IN THE VIEWPORT LEAVES THE FUNCTIONS IT WENT THROUGH, DEEPER AND DEEPER.
///
/// The indentation is the whole of it. A flat list of names would say that `pick_edge_3d` was called; only
/// the depth says that it was called FROM a click, which is what turns a name into a path.
#[test]
fn a_click_in_the_viewport_leaves_a_trail() {
    let dir = trace_folder("click");
    qymcad_trace::write_into(Some(&dir));

    let mut app = App::default();
    crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    app.start_feat_cmd(5); // the chamfer: the tool whose edge picking is the question
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());

    let body = app.project.mesh_id(0).expect("the cube is in the document");
    qymcad_ui_state::select_body(&mut app.project, &mut app.chosen.sel, &mut app.viewing.view, body);
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
    app.viewport_3d_click_at(egui::pos2(400.0, 300.0), rect, &app.viewing.cam.basis());

    let path = qymcad_trace::path().expect("the trace has a place");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("the trace is unreadable at {}: {e}", path.display()));

    assert!(text.contains("viewport_3d_click_at"), "the click is not in the trace at all:\n{text}");
    // THE PICKING IS BELOW THE CLICK, not beside it: the two lines must not stand at the same depth.
    let click = text.find("viewport_3d_click_at").expect("the click is traced");
    let deep = text.find("  ").map(|i| i).expect("the trace is indented at all");
    assert!(deep > click, "every line stands at the same depth, so the file is a list and not a path:\n{text}");

    qymcad_trace::write_into(None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A FUNCTION THAT NEVER COMES BACK IS THE LAST THING IN THE FILE. That is what a fault report is read
/// against, so it is checked here rather than assumed: the depth must go back down on its own.
#[test]
fn the_depth_goes_back_down_when_a_function_returns() {
    let dir = trace_folder("depth");
    qymcad_trace::write_into(Some(&dir));

    qymcad_trace::trace!("outer");
    {
        qymcad_trace::trace!("inner");
        qymcad_trace::trace_line!("the middle");
    }
    qymcad_trace::trace_line!("the end");

    let path = qymcad_trace::path().expect("the trace has a place");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();

    // A LINE IS ITS CLOCK AND THEN ITS TEXT, and the text is what is being looked for - so the clock is
    // cut off rather than trimmed: trimming leading spaces would take the indent with it, which is the very
    // thing this check is about.
    fn body(line: &str) -> &str {
        let (clock, rest) = line.split_once(' ').unwrap_or(("", line));
        assert!(clock.chars().all(|c| c.is_ascii_digit()), "a trace line does not begin with its clock: {line:?}");
        rest
    }
    let names: Vec<&str> = lines.iter().map(|l| body(l).trim_start()).collect();
    let indent = |n: &str| -> usize {
        let line = body(lines[names.iter().position(|x| *x == n).unwrap_or_else(|| panic!("the trace has no line {n:?}:\n{text}"))]);
        line.len() - line.trim_start().len()
    };

    assert!(names.contains(&"outer") && names.contains(&"inner"), "the traced functions are not in the file:\n{text}");
    assert!(names.iter().position(|n| *n == "outer") < names.iter().position(|n| *n == "inner"), "the inner line came before the outer one:\n{text}");
    assert!(indent("inner") > indent("outer"), "the inner function is not drawn deeper than the outer one:\n{text}");
    // THE DEPTH GOES BACK DOWN WHEN THE FUNCTION RETURNS. The line after the inner one is still inside the
    // OUTER one, so it stands where the inner one stands - and not one step deeper, which is what a depth
    // that only ever grew would leave behind.
    assert_eq!(indent("the end"), indent("inner"), "the depth did not go back down when the function returned:\n{text}");

    qymcad_trace::write_into(None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A TRACE THAT CANNOT BE WRITTEN MUST NOT TAKE THE PROGRAM WITH IT. A file of ours failing to open is a
/// nuisance; the fault this whole module exists for is a nuisance of its own, and the two must not meet.
#[test]
fn a_trace_that_cannot_be_written_is_silent() {
    // a FILE where a folder should be: every write under it fails
    let dir = trace_folder("blocked");
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"x").expect("a plain file is written");
    qymcad_trace::write_into(Some(&blocked));

    qymcad_trace::trace!("into a place that cannot take it");
    qymcad_trace::trace_line!("and a line under it");

    qymcad_trace::write_into(None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// NO LINE IS EVER CUT IN HALF, however many threads are writing at once.
///
/// The line was formatted and written a field at a time, and two threads doing that into one file welded
/// their clocks into a single unreadable number - measured on a live run. A trace with a torn line in it is
/// worse than no trace: the reader cannot tell where one line ends, so every depth after it is suspect.
#[test]
fn no_line_is_cut_in_half_by_two_threads_writing_at_once() {
    let dir = trace_folder("threads");
    qymcad_trace::write_into(Some(&dir));

    let path = qymcad_trace::path().expect("the trace has a place");
    let mut handles = Vec::new();
    for t in 0..4u32 {
        // THE FOLDER IS NAMED IN EVERY THREAD. Where the trace goes belongs to the thread that writes it -
        // one place for the whole process would have four threads cutting each other's lines, which is the
        // very thing this check is about.
        let here = dir.clone();
        handles.push(std::thread::spawn(move || {
            qymcad_trace::write_into(Some(&here));
            for i in 0..200 {
                qymcad_trace::trace!("thread {t} step {i}");
                qymcad_trace::trace_line!("  thread {t} value {}", i * 3);
            }
        }));
    }
    for h in handles {
        h.join().expect("the writing thread finished");
    }

    let text = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(text.lines().count(), 4 * 200 * 2, "some lines were lost or welded together:\n{text}");
    for (n, line) in text.lines().enumerate() {
        let (clock, rest) = line.split_once(' ').unwrap_or_else(|| panic!("line {n} has no clock in it: {line:?}"));
        assert!(clock.chars().all(|c| c.is_ascii_digit()) && clock.len() >= 13, "line {n} does not begin with a whole clock: {line:?}");
        assert!(rest.contains("thread "), "line {n} does not carry the whole of its text: {line:?}");
    }

    qymcad_trace::write_into(None);
    let _ = std::fs::remove_dir_all(&dir);
}

}
