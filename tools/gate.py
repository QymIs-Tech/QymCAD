#!/usr/bin/env python3
"""THE GATE: a level of the run of the checks, run and reported.

    tools/gate.py fast      at every edit, minutes: the rules of the code, the checks of the oracles themselves, and the
                            acceptance probes that do no heavy geometry
    tools/gate.py full      before a commit: the fast level, plus every other crate - the kernel, the heavy geometry,
                            the matrices of the testkit - and every acceptance probe, the short generated chains with them
    tools/gate.py release   before the public repository: the full level with the long generated chains, and every
                            breakage on order (`tools/breakages.py`) caught. A whole run of it leaves its mark in
                            `target/gate-release.json` - the commit, whether the tree was clean, whether it passed - and
                            `tools/publish.py` refuses without that mark for the commit it publishes
    tools/gate.py L --step NAME     only the step of the level whose name begins with NAME (and no mark)
    tools/gate.py L --except NAME   every step of the level but that one (and no mark)
    tools/gate.py L --shard K/N     the acceptance probes of the level split in N shares, only the K-th run (and no mark)
    tools/gate.py L --skip-probe P  the acceptance probes matching P left out (and no mark)
    tools/gate.py time              the probes of time alone, one at a time; CI sets QYMCAD_TIME_SCALE=2

EVERY RED STOPS THE GATE: a red probe is a trouble to mend before the commit, not a row to keep beside it.

`full` leaves `target/gate-full.json` with the tree it ran on, and `tools/hooks/pre-commit` lets a commit through only
when that tree is the one being committed and the level passed (switched on by `git config core.hooksPath tools/hooks`).

Each step runs under a memory cap without swap - the acceptance set is a process a probe, six at a time. The report
names what passed, what is red, and how long each step took.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# THE CAP IS FOR A DESKTOP: without it a run that ate the memory took the editor down with it, twice. A CI runner is a
# machine of its own with no user session of systemd to put a scope in, and the machine is the bound there.
CAP = [] if os.environ.get("CI") else ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=16G", "-p", "MemorySwapMax=0"]

# THE PROBES OF HEAVY GEOMETRY, left out of the fast level: the kernel's own work - booleans, fillets, meshes, the
# exchange of files, the placing of parts - and the pictures of the window, which are the release's. A module not
# named here is in the fast level by itself, so a new one is run at every edit until it is found heavy.
HEAVY = [
    "part_", "assembly_", "gizmo_in_space", "import_export", "export_scope", "parts_library", "pictures",
    "contract::extrude", "contract::fillet",
    # the matrices of the tools of the part: every tool across bodies, picks and values, the round trips with them at
    # the level of a release
    "matrix_",
]

# THE STEPS BOTH LEVELS SHARE, and the acceptance run each makes.
BUILDS = ("the code builds with no warning", ["cargo", "check", "--workspace", "--all-targets"], {})
ORACLES = ("the oracles and the runner", ["cargo", "test", "-p", "qymcad-acceptance", "--lib"], {})
ACCEPTANCE = ["cargo", "test", "-p", "qymcad-acceptance", "--test", "acceptance", "--no-fail-fast", "--", "--test-threads=6"]

# THE PROBES OF TIME: they measure the machine as much as the program. Run apart and one at a time by the `time` level,
# which CI uses with QYMCAD_TIME_SCALE=2 - on a runner of four cores, among five other probes, the robot sample rebuilt in
# 39.4 s against a budget of 30.
TIME = "size_and_time"

RELEASE_MARK = os.path.join(ROOT, "target", "gate-release.json")

LEVELS = {
    "release": [
        BUILDS,
        ("every crate's own checks", ["cargo", "test", "--workspace", "--exclude", "qymcad-acceptance", "--no-fail-fast"], {}),
        ORACLES,
        # the long chains: six of 150 steps a slot - see `chains::Tier`
        ("every acceptance probe", ACCEPTANCE, {"QYMCAD_TIER": "release"}),
        # every path of a person broken on purpose, one line at a time, must turn its probe red; the script puts its
        # own memory cap on each run and writes every file back as it read it
        ("every breakage on order caught", ["python3", "tools/breakages.py", "--fresh"], {}),
    ],
    "full": [
        BUILDS,
        # every crate but the acceptance set: the rules of the code, the words, the help, the kernel and the heavy
        # geometry, the testkit's matrices
        ("every crate's own checks", ["cargo", "test", "--workspace", "--exclude", "qymcad-acceptance", "--no-fail-fast"], {}),
        ORACLES,
        ("every acceptance probe", ACCEPTANCE, {"QYMCAD_TIER": "full"}),
    ],
    "time": [
        ("the probes of time, one at a time", ACCEPTANCE[:-1] + ["--test-threads=1", TIME], {"QYMCAD_TIER": "fast"}),
    ],
    "fast": [
        BUILDS,
        ("the rules of the code", ["cargo", "test", "-p", "qymcad", "--lib", "--", "ratchet"], {}),
        ("the words of the interface", ["cargo", "test", "-p", "qymcad-i18n"], {}),
        ("the help", ["cargo", "test", "-p", "qymcad-help"], {}),
        ORACLES,
        (
            "the acceptance probes without heavy geometry",
            ACCEPTANCE + [a for h in HEAVY for a in ("--skip", h)],
            {"QYMCAD_TIER": "fast"},
        ),
    ],
}

TEST_LINE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)$")
FULL_MARK = os.path.join(ROOT, "target", "gate-full.json")


def tree_of_working_copy():
    """THE TREE THE LEVEL RUNS ON: what `git write-tree` would give were everything in the working copy - untracked
    files not ignored as well - added. Made in an index of its own, so the real one is not touched."""
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        env = {**os.environ, "GIT_INDEX_FILE": os.path.join(d, "index")}
        subprocess.run(["git", "read-tree", "HEAD"], cwd=ROOT, env=env, check=True)
        subprocess.run(["git", "add", "-A"], cwd=ROOT, env=env, check=True)
        return subprocess.run(["git", "write-tree"], cwd=ROOT, env=env, capture_output=True, text=True, check=True).stdout.strip()


def run_step(name, cmd, env):
    """Run one step; answer (seconds, passed names, red names, whether the step itself failed without a red test)."""
    began = time.time()
    # a script of ours caps its own runs; cargo is capped here
    capped = cmd if cmd[0] == "python3" else CAP + cmd
    out = subprocess.run(capped, cwd=ROOT, capture_output=True, text=True, env={**os.environ, **env})
    said = out.stdout + out.stderr
    if cmd[0] == "python3":
        # the breakages speak in lines of their own: "ok <name>" caught, anything else not
        caught = [l.strip()[3:] for l in said.splitlines() if l.startswith("  ok ")]
        missed = [l.strip() for l in said.splitlines() if l.startswith("  -- ") or l.startswith("  ?  ")]
        return time.time() - began, caught, missed, out.returncode != 0 and not missed, said
    passed, red = [], []
    for line in said.splitlines():
        m = TEST_LINE.match(line)
        if m:
            (passed if m.group(2) == "ok" else red if m.group(2) == "FAILED" else []).append(m.group(1))
    # THE CODE HAS NO WARNING, not merely no error: the rule of the project is zero, and a build that warns still
    # exits well - so every warning is a red of its own, named by its first line
    for line in said.splitlines():
        if line.startswith("warning:") and "generated" not in line:
            red.append(f"{name}: {line}")
    # a step that failed with no red test of its own - it did not build, or it is not a run of tests at all
    broken = out.returncode != 0 and not red
    # the same name in two crates is two checks, so the passed are counted as they come; a red probe of the acceptance
    # set is named twice - its own process repeats its line in the words the probe fails with - so the red are not
    return time.time() - began, passed, sorted(set(red)), broken, said


def shard_of(cmd, env, k, n):
    """THE K-TH OF N SHARES of an acceptance run: its probes listed (the skips of the level applied), every N-th taken
    from the K-th on, and named exactly. Round the list rather than cut it in blocks: the probes of one module stand
    together and cost alike, so a block would put the slow modules on one machine."""
    listed = subprocess.run(CAP + cmd + ["--list", "--format", "terse"], cwd=ROOT, capture_output=True, text=True, env={**os.environ, **env})
    if listed.returncode != 0:
        # the build failed; the step itself runs the same build and reports it
        return cmd
    names = sorted(l[: -len(": test")] for l in listed.stdout.splitlines() if l.endswith(": test"))
    mine = names[k - 1 :: n]
    print(f"share {k}/{n}: {len(mine)} of {len(names)} acceptance probes")
    return ACCEPTANCE + ["--exact"] + mine


def main():
    ap = argparse.ArgumentParser(description="Run a level of the checks and report it.")
    ap.add_argument("level", choices=sorted(LEVELS))
    ap.add_argument("--step", help="run only the step whose name begins with this")
    ap.add_argument("--except", dest="skip", help="run every step but the one whose name begins with this")
    ap.add_argument("--skip-probe", action="append", default=[], help="leave the acceptance probes matching this out (repeatable)")
    ap.add_argument("--shard", help="K/N: of the acceptance probes, run only the K-th of N shares (1-based)")
    args = ap.parse_args()
    steps = [
        s for s in LEVELS[args.level]
        if (args.step is None or s[0].startswith(args.step)) and (args.skip is None or not s[0].startswith(args.skip))
    ]
    if not steps:
        print(f"the {args.level} level has no such step: " + "; ".join(s[0] for s in LEVELS[args.level]))
        return 2
    if args.skip_probe:
        extra = [a for p in args.skip_probe for a in ("--skip", p)]
        steps = [(name, cmd + extra, env) if cmd[:len(ACCEPTANCE)] == ACCEPTANCE else (name, cmd, env) for name, cmd, env in steps]
    if args.shard:
        try:
            k, n = (int(x) for x in args.shard.split("/"))
            assert 1 <= k <= n
        except (ValueError, AssertionError):
            print(f"--shard takes K/N with 1 <= K <= N, not {args.shard!r}")
            return 2
        steps = [(name, shard_of(cmd, env, k, n), env) if cmd[:len(ACCEPTANCE)] == ACCEPTANCE else (name, cmd, env) for name, cmd, env in steps]
    whole = args.step is None and args.skip is None and args.shard is None and not args.skip_probe
    total_began = time.time()
    all_red, broken_steps = [], []
    for name, cmd, env in steps:
        seconds, passed, red, broken, said = run_step(name, cmd, env)
        # THE WHOLE OF WHAT A STEP SAID is kept: a red chain comes back shrunk and written out as a probe, and that is
        # in the words, not in the report
        log = os.path.join(ROOT, "target", f"gate-{args.level}-" + re.sub(r"[^a-z]+", "-", name.lower()).strip("-") + ".log")
        os.makedirs(os.path.dirname(log), exist_ok=True)
        with open(log, "w", encoding="utf-8") as f:
            f.write(said)
        state = "did not run" if broken else (f"{len(red)} red" if red else "green")
        print(f"{name}: {len(passed)} passed, {state} ({seconds:.0f} s); its words in {os.path.relpath(log, ROOT)}")
        if broken:
            broken_steps.append(name)
            print("\n".join("    " + l for l in said.splitlines()[-15:]))
        all_red.extend(red)
    print(f"\nthe {args.level} level: {time.time() - total_began:.0f} s; {len(all_red)} red" + (f", {len(broken_steps)} steps did not run" if broken_steps else ""))
    for r in all_red:
        print(f"  red: {r}")
    passed = not all_red and not broken_steps
    if args.level == "release" and whole:
        # THE MARK OF A WHOLE RELEASE RUN, for `tools/publish.py`: the commit it ran on, and whether the tree held
        # nothing uncommitted - a run over uncommitted changes vouches for no commit at all
        head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
        clean = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True).stdout.strip() == ""
        os.makedirs(os.path.dirname(RELEASE_MARK), exist_ok=True)
        with open(RELEASE_MARK, "w", encoding="utf-8") as f:
            json.dump({"commit": head, "clean": clean, "passed": passed, "red": all_red, "seconds": round(time.time() - total_began)}, f, ensure_ascii=False, indent=1)
        print(f"the mark of the release level is left in {RELEASE_MARK}: {'passed' if passed else 'not passed'}, commit {head[:9]}, tree {'clean' if clean else 'with uncommitted changes'}")
    if args.level == "full" and whole:
        # THE MARK OF A WHOLE FULL RUN, for the hook before a commit: the tree it ran on, and whether it passed
        os.makedirs(os.path.dirname(FULL_MARK), exist_ok=True)
        with open(FULL_MARK, "w", encoding="utf-8") as f:
            json.dump({"tree": tree_of_working_copy(), "passed": passed, "red": all_red}, f, ensure_ascii=False, indent=1)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
