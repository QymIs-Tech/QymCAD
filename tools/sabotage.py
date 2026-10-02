#!/usr/bin/env python3
"""BREAK THE PATH OF A PERSON ON PURPOSE, one breakage at a time, and see which acceptance checks catch it.

A check that stays green while the thing it is about is broken checks nothing. Each breakage below is a small edit
of the program on the path a person takes - a click that goes nowhere, a button that does nothing, a number typed
and lost. The script puts one in, runs the acceptance checks, takes it out again by writing the file back as it was,
and says which checks turned red that were not red before. A breakage no check catches is a hole in the checks, and
the script fails on it.

    tools/sabotage.py door              every breakage of the door
    tools/sabotage.py door --only NAME  one of them

The files are written back even when the run is interrupted. Nothing is committed.
"""
import argparse
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Each breakage: its name, the edits that make it - (file, text found exactly once, text put in its place) - and, when
# it is aimed at one check in particular, words that check fails with, which must be in the output of the run.
SETS = {
    "door": [
        (
            "a click in the 3D view goes nowhere",
            [(
                "crates/qymcad/src/gui/viewport_3d.rs",
                "        if !resp.clicked() {\n            return;\n        }\n        let Some(pos) = resp.interact_pointer_pos() else { return };",
                "        if !resp.clicked() || true {\n            return;\n        }\n        let Some(pos) = resp.interact_pointer_pos() else { return };",
            )],
        ),
        (
            "a tool button does nothing",
            [(
                "crates/qymcad-ui-state/src/lib.rs",
                "    ui.add_sized(egui::vec2(40.0, 34.0), btn).on_hover_text(tip).clicked()",
                "    ui.add_sized(egui::vec2(40.0, 34.0), btn).on_hover_text(tip).clicked() && false",
            )],
        ),
        (
            "a number typed at the geometry does not reach the command",
            [(
                "crates/qymcad-part/src/lib.rs",
                "                    let te = o.resp;\n                    match qymcad_core::expr::eval(&p.txt, &vars) {\n                        Ok(v) => p.val = v.clamp(p.lo, p.hi),",
                "                    let te = o.resp;\n                    match qymcad_core::expr::eval(&p.txt, &vars) {\n                        Ok(v) => {\n                            let _ = v;\n                        }",
            )],
        ),
        (
            "Enter does not apply a command",
            [
                (
                    "crates/qymcad-part/src/lib.rs",
                    "                    if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {\n                        enter = true;",
                    "                    if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {\n                        enter = false;",
                ),
                (
                    "crates/qymcad/src/gui/input.rs",
                    "            } else {\n                crate::gui::commands::apply_feat_cmd(&mut self.part_ctx());",
                    "            } else if false {\n                crate::gui::commands::apply_feat_cmd(&mut self.part_ctx());",
                ),
            ],
        ),
        (
            "a drag on the sketch takes what is under the pointer when the drag is recognised, not what was pressed",
            [(
                "crates/qymcad-sketch/src/lib.rs",
                "ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos())",
                "resp.interact_pointer_pos().or(ctx.input(|i| i.pointer.press_origin()))",
            )],
        ),
    ],
    "contract": [
        (
            "the fillet is made with another radius than the one typed",
            [(
                "crates/qymcad-kernel/src/lib.rs",
                "qym_shape_fillet_edges(self.ptr, r, idx.as_ptr()",
                "qym_shape_fillet_edges(self.ptr, r * 1.1, idx.as_ptr()",
            )],
        ),
        (
            "the step of undo of a fillet is named after another tool",
            [(
                "crates/qymcad-part/src/lib.rs",
                '        4 => qymcad_i18n::tr("f-fillet"),',
                '        4 => qymcad_i18n::tr("f-chamfer"),',
            )],
        ),
        (
            "a reopened extrusion shows the height the tool starts with, not its own",
            [(
                "crates/qymcad-part/src/lib.rs",
                '            if down.abs() > 1e-9 {\n                pc.cmd.down = down;\n            }\n            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-length", "height", height, 0.1, 10000.0)];\n        }\n        FeatureKind::Combine',
                '            if down.abs() > 1e-9 {\n                pc.cmd.down = down;\n            }\n            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-length", "height", 10.0, 0.1, 10000.0)];\n        }\n        FeatureKind::Combine',
            )],
        ),
        (
            "a symmetric extrusion grows one way",
            [(
                "crates/qymcad-core/src/feature.rs",
                "        Reach::BothWays => (-h / 2.0, h),",
                "        Reach::BothWays => (0.0, h),",
            )],
        ),
    ],
    "oracles": [
        (
            "a key of the catalogue stands on screen instead of its words",
            [(
                "crates/qymcad-i18n/src/lib.rs",
                "pub fn tr_args(key: &str, args: Option<&FluentArgs>) -> String {\n",
                "pub fn tr_args(key: &str, args: Option<&FluentArgs>) -> String {\n    if key == \"wb-finish\" {\n        return key.to_string();\n    }\n",
            )],
            "a key of the catalogue stands on screen",
        ),
        (
            "a word is drawn with a letter the font does not have",
            [("i18n/en/main.ftl", "wb-finish = Finish\n", "wb-finish = Finish \u4e2d\n")],
            "a letter the font does not have",
        ),
        (
            "a command in hand puts up no bar",
            [("crates/qymcad/src/gui.rs", '            "feat_cmd_bar" => self.tools.armed.commanding(),', '            "feat_cmd_bar" => false,')],
            "waits for a click, and no bar of options says so",
        ),
        (
            "the bar of the sketch's tools stands outside a sketch",
            [("crates/qymcad/src/gui.rs", '            "sk_tool_opts" => edit_si(&self.project, &self.sketch_ses).is_some(),', '            "sk_tool_opts" => true,')],
            "a bar of options stands with nothing in hand",
        ),
        (
            "applying a command holds the frame for over three seconds",
            [(
                "crates/qymcad-part/src/lib.rs",
                "pub fn apply_feat_cmd(pc: &mut qymcad_ui_state::PartCtx) {\n",
                "pub fn apply_feat_cmd(pc: &mut qymcad_ui_state::PartCtx) {\n    std::thread::sleep(std::time::Duration::from_millis(3200));\n",
            )],
            "a frame took",
        ),
        (
            "redo does nothing",
            [("crates/qymcad/src/gui.rs", "        if let Some(step) = self.disk.edits.redo.pop() {", "        if let Some(step) = self.disk.edits.redo.pop().filter(|_| false) {")],
            "undo and redo changed the document",
        ),
        (
            "rebuilding everything drops the last node of a longer timeline",
            [("crates/qymcad/src/gui.rs", "        self.project.mark_all_dirty();\n", "        self.project.mark_all_dirty();\n        if self.project.timeline.len() > 2 {\n            self.project.timeline.pop();\n        }\n")],
            "rebuilt from the start, the document is",
        ),
        (
            "saving renames the faces of the bodies",
            [(
                "crates/qymcad/src/gui/io_jobs.rs",
                "    proj.regen_edges.clear();\n",
                "    proj.regen_edges.clear();\n    for b in proj.bodies.iter_mut() {\n        for f in b.faces.iter_mut() {\n            f.id = f.id.wrapping_add(1000);\n        }\n    }\n",
            )],
            "saved as",
        ),
    ],
}

# Runs heavy work under a ceiling of memory with no swap: a run over it is stopped, not the machine.
RUN = ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=16G", "-p", "MemorySwapMax=0",
       "cargo", "test", "-p", "qymcad-acceptance", "--test", "acceptance"]


def red_checks():
    """Run the acceptance checks; answer the names of the red ones and all they said, or None when the build failed."""
    out = subprocess.run(RUN, cwd=ROOT, capture_output=True, text=True)
    text = out.stdout + out.stderr
    if "test result:" not in text:
        sys.stdout.write(text[-4000:])
        return None
    return set(re.findall(r"^test (\S+) \.\.\. FAILED$", text, re.M)), text


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("set", choices=sorted(SETS))
    ap.add_argument("--only")
    args = ap.parse_args()
    chosen = [b for b in SETS[args.set] if args.only in (None, b[0])]
    if not chosen:
        sys.exit(f"no breakage is named {args.only!r}")

    print("baseline run ...", flush=True)
    run = red_checks()
    if run is None:
        sys.exit("the checks do not build as they stand")
    known = run[0]
    print(f"red before any breakage: {len(known)}")
    for name in sorted(known):
        print(f"  {name}")

    holes = []
    for breakage in chosen:
        name, edits = breakage[0], breakage[1]
        expect = breakage[2] if len(breakage) > 2 else None
        originals = {}
        try:
            for path, find, put in edits:
                full = os.path.join(ROOT, path)
                text = originals.setdefault(full, open(full, encoding="utf-8").read())
                current = open(full, encoding="utf-8").read()
                if current.count(find) != 1:
                    raise SystemExit(f"{name}: the text to break is found {current.count(find)} times in {path}, not once")
                with open(full, "w", encoding="utf-8") as f:
                    f.write(current.replace(find, put))
            print(f"\nbreakage: {name} ...", flush=True)
            run = red_checks()
        finally:
            for full, text in originals.items():
                with open(full, "w", encoding="utf-8") as f:
                    f.write(text)
        if run is None:
            holes.append(f"{name}: the program did not build with it")
            continue
        red, said = run
        caught = sorted(red - known)
        print(f"caught by {len(caught)}:")
        for check in caught:
            print(f"  {check}")
        if not caught:
            holes.append(name)
        elif expect is not None and expect not in said:
            holes.append(f"{name}: checks went red, but none said {expect!r}")

    print()
    if holes:
        print("NOT CAUGHT:")
        for h in holes:
            print(f"  {h}")
        sys.exit(1)
    print(f"every breakage caught: {len(chosen)} of {len(chosen)}")


if __name__ == "__main__":
    main()
