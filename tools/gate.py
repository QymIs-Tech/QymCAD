#!/usr/bin/env python3
"""THE GATE: a level of the run of the checks, run and reported.

    tools/gate.py fast      at every edit, minutes: the rules of the code, the checks of the oracles themselves, and the
                            acceptance probes that do no heavy geometry
    tools/gate.py full      before a commit: the fast level, plus every other crate - the kernel, the heavy geometry,
                            the matrices of the testkit - and every acceptance probe, the short generated chains with them
    tools/gate.py release   before the public repository: the full level with the long generated chains, the heaviest
                            matrices of the testkit (`RELEASE_ONLY`), and every
                            breakage on order (`tools/breakages.py`) caught. A whole run of it leaves its mark in
                            `target/gate-release.json` - the commit, whether the tree was clean, whether it passed - and
                            `tools/publish.py` refuses without that mark for the commit it publishes
    tools/gate.py L --step NAME     only the step of the level whose name begins with NAME (and no mark)
    tools/gate.py L --except NAME   every step of the level but that one (and no mark)
    tools/gate.py L --shard K/N     the acceptance probes and the checks of every crate split in N shares, only the K-th
                                    run (and no mark)
    tools/gate.py L --skip-probe P  the acceptance probes matching P left out (and no mark)
    tools/gate.py time              the probes of time alone, one at a time; CI sets QYMCAD_TIME_SCALE=2
    tools/gate.py --self-test       the gate's own reckoning checked on cases: the threads it picks, the times it reads

EVERY RED STOPS THE GATE: a red probe is a trouble to mend before the commit, not a row to keep beside it.

`full` leaves `target/gate-full.json` with the tree it ran on, and `tools/hooks/pre-commit` lets a commit through only
when that tree is the one being committed and the level passed (switched on by `git config core.hooksPath tools/hooks`).

Each step runs under a memory cap without swap - the acceptance set is a process a probe, as many at a time as the
machine holds (`pick_threads`; QYMCAD_TEST_THREADS sets it by hand). The report names what passed, what is red, how
long each step took, and the slowest acceptance probes.
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
CAP_GB = 16
CAP = [] if os.environ.get("CI") else ["systemd-run", "--user", "--scope", "-q", "-p", f"MemoryMax={CAP_GB}G", "-p", "MemorySwapMax=0"]

# THE ACCEPTANCE PROBES SIDE BY SIDE: as many as the machine has threads for and memory for. One number for every machine
# did not fit: on 20 threads and 31 GB the fast level took 1209 s at 6, 851 s at 12 (the scope's peak 1.38 GB) and 754 s
# at 20 (1.66 GB), while a laptop of 4 threads was given the same 6.
# The memory of one probe: the heaviest measured 670 MB (`crates/qymcad-acceptance/src/isolation.rs`, where a probe is
# stopped past 2 GB). Reckoning every probe at its 2 GB limit would give 8 under the cap of 16 GB, yet 20 side by side
# peaked at 1.66 GB together.
PROBE_MB = 700
# On a desktop a tenth of the threads, at least one, is left to the person's own programs, so the editor and the window
# stay usable while the gate runs: 18 of 20. A runner of the CI has nobody else on it.
THREADS_VAR = "QYMCAD_TEST_THREADS"
# THE TIME OF EACH PROBE, written by the runner (`isolation.rs`, the same variable) as `seconds<TAB>name` a line.
TIMES_VAR = "QYMCAD_PROBE_TIMES"
SLOWEST = 10

# THE PROBES OF HEAVY GEOMETRY, left out of the fast level: the kernel's own work - booleans, fillets, meshes, the
# exchange of files, the placing of parts - and the pictures of the window, which are the release's. A module not
# named here is in the fast level by itself, so a new one is run at every edit until it is found heavy.
HEAVY = [
    "part_", "assembly_", "gizmo_in_space", "import_export", "export_scope", "parts_library", "pictures",
    "contract::extrude", "contract::fillet",
    # the matrices of the tools of the part: every tool across bodies, picks and values, the round trips with them at
    # the level of a release
    "matrix_",
    # every rounding and chamfer of the sample PC case rolled back to, suppressed and put back - four rebuilds of the
    # whole document a node, 340 s; started late among the probes in name order it ran alone for the last 270 s of a fast
    # step of 819 s. The same check on the filter sample stays in the fast level (41 s)
    "project_checks::every_rounding_of_the_case_sample",
]

# THE STEPS BOTH LEVELS SHARE, and the acceptance run each makes.
BUILDS = ("the code builds with no warning", ["cargo", "check", "--workspace", "--all-targets"], {})
# THE APPLICATION BUILDS AS IT SHIPS: with no target of a probe the dev-dependencies are not built, so the specimens of
# the bridge (`qymcad_kernel::specimen`, asked for by dev-dependencies alone) are not there, and code of the window that
# builds a body past the door does not compile. A build with every target has them, for the probes.
SHIPPED = ("the application builds as it ships", ["cargo", "check", "--workspace", "--lib", "--bins"], {})
ORACLES = ("the oracles and the runner", ["cargo", "test", "-p", "qymcad-acceptance", "--lib"], {})
ACCEPTANCE = ["cargo", "test", "-p", "qymcad-acceptance", "--test", "acceptance", "--no-fail-fast", "--"]
# the place of the number of threads in a run of the acceptance set, filled once the machine is looked at
PICKED = "--test-threads=PICKED"

# THE PROBES OF TIME: they measure the machine as much as the program. Run apart and one at a time by the `time` level,
# which CI uses with QYMCAD_TIME_SCALE=2 - on a runner of four cores, among five other probes, a sample project of 3 MB rebuilt in
# 39.4 s against a budget of 30.
TIME = "size_and_time"
# THE PROBES OF TIME OF THE SKETCH, in the core: they pass over at every level but this one, and run one at a time. Beside
# the checks of every crate a budget of 150 ms read 165 ms on a runner of the CI.
SKETCH_TIME = (
    "the probes of time of the sketch, one at a time",
    ["cargo", "test", "-p", "qymcad-core", "--test", "the_sketch_keeps_its_time", "--", "--test-threads=1"],
    {"QYMCAD_TIER": "time"},
)

# THE PROBES OF TIME OF A THREAD, in the bridge: they hold their budgets at this level only, one at a time.
THREAD_TIME = (
    "the probes of time of a thread, one at a time",
    ["cargo", "test", "-p", "qymcad-kernel", "--lib", "thread_perf", "--", "--test-threads=1"],
    {"QYMCAD_TIER": "time"},
)

# THE PROBES OF TIME OF A BIG DRAWING, through the window: their budgets hold at this level only, one at a time.
DRAWING_TIME = (
    "the probes of time of a big drawing, one at a time",
    ["cargo", "test", "-p", "qymcad", "--lib", "a_big_drawing_stays_live", "--", "--test-threads=1"],
    {"QYMCAD_TIER": "time"},
)

# THE PROBES OF TIME OF THE WINDOW, OF A GRID AND OF THE SKETCH OF THE REPORT: run each to a process beside 17 others in
# the pool of every crate, they read the load - a grid of 200 moved in 85 ms against 60, the Line tool taken in 5.2 s,
# eight redraws of a gallery in 216 ms against 180 - so their budgets hold at this level only, one at a time.
WINDOW_TIME = (
    "the probes of time of the window, one at a time",
    ["cargo", "test", "-p", "qymcad", "--lib", "--", "the_sketch_of_the_report_takes_a_line", "selected_bundled_default_redraw_stays_responsive", "--test-threads=1"],
    {"QYMCAD_TIER": "time"},
)
GRID_TIME = (
    "the probes of time of a grid, one at a time",
    ["cargo", "test", "-p", "qymcad-core", "--test", "a_line_of_a_connected_grid_is_dragged_within_a_frame", "--", "--test-threads=1"],
    {"QYMCAD_TIER": "time"},
)
# THE TOOLS ON A BIG SKETCH: a pattern of 200 rectangles turned in 320 ms against 247 among the others of the pool
BIG_SKETCH_TIME = (
    "the probes of time of a big sketch, one at a time",
    ["cargo", "test", "-p", "qymcad-core", "--test", "every_tool_keeps_its_time_on_a_big_sketch", "--", "--test-threads=1"],
    {"QYMCAD_TIER": "time"},
)
REPORT_TIME = (
    "the probes of time of the sketch of the report, one at a time",
    ["cargo", "test", "-p", "qymcad-testkit", "--test", "every_tool_keeps_its_time_on_the_sketch_of_the_report", "--", "--test-threads=1"],
    {"QYMCAD_TIER": "time"},
)

RELEASE_MARK = os.path.join(ROOT, "target", "gate-release.json")

# THE CHECKS OF EVERY CRATE IN ONE POOL. `cargo test` runs the test programs of the workspace one after another, each
# with its own threads: of 1165 s of a run, 253 s went to five checks of one program (`matrix_names_survive_an_edit`)
# with 15 of 20 threads idle, and the five slowest programs took 68 %. The pool (`--pool`) builds them, lists their
# checks and runs each check as a process of its own, as many side by side as the acceptance probes, the slowest of the
# run before first and a check never timed before them all - so a slow one starts at once rather than last.
CRATES = ["cargo", "test", "--workspace", "--exclude", "qymcad-acceptance"]
EVERY_CRATE = ("every crate's own checks", ["python3", "tools/gate.py", "--pool", PICKED], {})
# the time of each check of the pool, `seconds<TAB>key` a line, read to order the next run
CHECK_TIMES = os.path.join(ROOT, "target", "gate-check-times.tsv")
# A CHECK STILL RUNNING AFTER THIS is stopped and red: once a check of a parallel run hung 44 min with no CPU.
CHECK_TIMEOUT_S = 1200
# THE QUICK CHECKS GO IN BATCHES. A process a check pays the start of its program every time - the libraries of OCCT
# loaded, the fonts and the words read: on a runner of four cores the step went from 696 s with `cargo test` to 973 s
# with a process for each of its 3328 checks. A check that took less than `QUICK_S` the run before is run with others of
# its program in one process, a batch of about `BATCH_S`; a slow check and one never timed still run alone.
QUICK_S = 1.0
BATCH_S = 5.0

# THE HEAVIEST MATRICES OF THE TESTKIT GO TO THE RELEASE: a bolt and its nut screwed through a whole turn (5 checks, 100 -
# 400 s each among the others of the pool) and the names held through an edit above (3 checks, 145 - 240 s). They are
# run before a release (`RELEASE_MATRICES`), not at every change; the pool of every crate leaves their programs out.
RELEASE_ONLY = ["a_bolt_screws_into_its_nut", "matrix_names_survive_an_edit"]
RELEASE_MATRICES = (
    "the heaviest matrices of the testkit",
    ["cargo", "test", "-p", "qymcad-testkit", "--no-fail-fast"] + [a for t in RELEASE_ONLY for a in ("--test", t)],
    {},
)

LEVELS = {
    "release": [
        BUILDS,
        SHIPPED,
        EVERY_CRATE,
        RELEASE_MATRICES,
        SKETCH_TIME,
        THREAD_TIME,
        DRAWING_TIME,
        WINDOW_TIME,
        GRID_TIME,
        BIG_SKETCH_TIME,
        REPORT_TIME,
        # EVERY LANGUAGE WHOLE: a string missing in a language falls back to English, so the checks of every change pass
        # with English alone and a contributor is asked for no language they do not know; before a release each
        # language must hold every key of the reference
        ("every language is whole", ["cargo", "test", "-p", "qymcad-i18n", "--test", "every_language_is_complete_for_a_release"], {"QYMCAD_TIER": "release"}),
        ORACLES,
        # the long chains: six of 150 steps a slot - see `chains::Tier`
        ("every acceptance probe", ACCEPTANCE + [PICKED], {"QYMCAD_TIER": "release"}),
        # every path of a person broken on purpose, one line at a time, must turn its probe red; the script puts its
        # own memory cap on each run and writes every file back as it read it
        ("every breakage on order caught", ["python3", "tools/breakages.py", "--fresh"], {}),
    ],
    "full": [
        BUILDS,
        SHIPPED,
        # every crate but the acceptance set: the rules of the code, the words, the help, the kernel and the heavy
        # geometry, the testkit's matrices
        EVERY_CRATE,
        ORACLES,
        ("every acceptance probe", ACCEPTANCE + [PICKED], {"QYMCAD_TIER": "full"}),
    ],
    "time": [
        ("the probes of time, one at a time", ACCEPTANCE + ["--test-threads=1", TIME], {"QYMCAD_TIER": "fast"}),
        SKETCH_TIME,
        THREAD_TIME,
        DRAWING_TIME,
        WINDOW_TIME,
        GRID_TIME,
        BIG_SKETCH_TIME,
        REPORT_TIME,
    ],
    "fast": [
        # THE LAYOUT IS RUSTFMT'S: the tree was formatted once, and a hand layout coming back would be the next
        # thousand-line diff nobody can review
        ("the layout is rustfmt's", ["cargo", "fmt", "--all", "--check"], {}),
        BUILDS,
        SHIPPED,
        # CLIPPY HAS NOTHING TO SAY: every remark is an error in the manifest, and the tree was brought to none. A crate
        # that fails is not kept as built, so a run from the cache sees again whatever was not mended.
        ("clippy has nothing to say", ["cargo", "clippy", "--workspace", "--all-targets"], {}),
        ("the rules of the code", ["cargo", "test", "-p", "qymcad", "--lib", "--", "ratchet"], {}),
        ("the words of the interface", ["cargo", "test", "-p", "qymcad-i18n"], {}),
        ("the help", ["cargo", "test", "-p", "qymcad-help"], {}),
        ORACLES,
        (
            "the acceptance probes without heavy geometry",
            ACCEPTANCE + [PICKED] + [a for h in HEAVY for a in ("--skip", h)],
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
    # the script of the breakages caps its own runs; cargo and the pool of checks are capped here, the pool as one scope
    breakages = cmd[:2] == ["python3", "tools/breakages.py"]
    capped = cmd if breakages else CAP + cmd
    out = subprocess.run(capped, cwd=ROOT, capture_output=True, text=True, env={**os.environ, **env})
    said = out.stdout + out.stderr
    if breakages:
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


def listing_of(cmd):
    """THE COMMAND THAT LISTS THE PROBES of an acceptance run: its own, the number of threads left out - still the
    placeholder `PICKED` there, which libtest refused, and every share ran every probe: the four machines of CI each
    ran all 2890 (35 - 40 min), then the eight all 2892."""
    return [a for a in cmd if a != PICKED] + ["--list", "--format", "terse"]


def shard_of(cmd, env, k, n):
    """THE K-TH OF N SHARES of an acceptance run: its probes listed (the skips of the level applied), every N-th taken
    from the K-th on, and named exactly. Round the list rather than cut it in blocks: the probes of one module stand
    together and cost alike, so a block would put the slow modules on one machine."""
    listed = subprocess.run(CAP + listing_of(cmd), cwd=ROOT, capture_output=True, text=True, env={**os.environ, **env})
    if listed.returncode != 0:
        # the build failed; the step itself runs the same build and reports it
        return cmd
    names = sorted(l[: -len(": test")] for l in listed.stdout.splitlines() if l.endswith(": test"))
    mine = names[k - 1 :: n]
    print(f"share {k}/{n}: {len(mine)} of {len(names)} acceptance probes")
    return ACCEPTANCE + [PICKED, "--exact"] + mine


class TestProgram:
    """A TEST PROGRAM CARGO BUILT: the file to run, the folder of its crate it runs in (cargo runs a test there), the
    crate's name, and the key its checks are timed under."""

    def __init__(self, exe, crate_dir, crate, key):
        self.exe, self.crate_dir, self.crate, self.key = exe, crate_dir, crate, key


def test_programs(json_lines):
    """The test programs of the words `cargo test --no-run --message-format=json...` wrote on its standard output: the
    artifacts built for a test that have a file to run."""
    progs = []
    for line in json_lines.splitlines():
        if not line.startswith("{"):
            continue
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") != "compiler-artifact" or not msg.get("executable") or not msg.get("profile", {}).get("test"):
            continue
        target = msg.get("target", {})
        # `path+file:///.../qymcad-core#0.1.0` or `...#qymcad-core@0.1.0`: the crate's name is in the manifest's folder
        crate_dir = os.path.dirname(msg.get("manifest_path", ""))
        crate = os.path.basename(crate_dir)
        kind = "+".join(target.get("kind", []))
        progs.append(TestProgram(msg["executable"], crate_dir, crate, f"{crate}/{target.get('name', '')}/{kind}"))
    return progs


class Check:
    """ONE CHECK OF THE POOL: its program, its name there, and the key it is timed under."""

    def __init__(self, prog, name):
        self.prog, self.name, self.key = prog, name, f"{prog.key}::{name}"


def listed(prog, ignored):
    """The names of the checks of `prog` (`ignored`: only those marked to be passed over), as libtest lists them."""
    args = [prog.exe, "--list", "--format", "terse"] + (["--ignored"] if ignored else [])
    out = subprocess.run(args, cwd=prog.crate_dir, capture_output=True, text=True, env=check_env(prog))
    return [l[: -len(": test")] for l in out.stdout.splitlines() if l.endswith(": test")]


def check_env(prog):
    """THE ENVIRONMENT CARGO GIVES A TEST it runs: the crate's folder and name, and the folders of the libraries built
    beside the programs on the path of the loader."""
    deps = os.path.dirname(prog.exe)
    lib_path = os.pathsep.join(p for p in (deps, os.path.dirname(deps), os.environ.get("LD_LIBRARY_PATH", "")) if p)
    return {**os.environ, "CARGO_MANIFEST_DIR": prog.crate_dir, "CARGO_PKG_NAME": prog.crate, "LD_LIBRARY_PATH": lib_path}


def read_times(text):
    """The times of the run before, `seconds<TAB>key` a line, as {key: seconds}; a line not of that shape is passed over."""
    times = {}
    for line in text.splitlines():
        seconds, _, key = line.partition("\t")
        try:
            times[key] = float(seconds)
        except ValueError:
            continue
    return times


class Job:
    """ONE PROCESS OF THE POOL: checks of one program run together, and the time they are expected to take - none for a
    check never timed."""

    def __init__(self, checks, expected):
        self.checks, self.expected = checks, expected


def jobs_of(checks, times):
    """THE PROCESSES OF THE POOL, in the order they start: a check never timed alone and first, as it may be the slowest;
    then the slowest first - a check that took `QUICK_S` or more alone, the quicker ones of a program together in
    batches of about `BATCH_S`. The name breaks a tie, so the order is the same from run to run."""
    jobs, open_batch = [], {}
    for c in sorted(checks, key=lambda c: c.key):
        took = times.get(c.key)
        if took is None or took >= QUICK_S:
            jobs.append(Job([c], took))
            continue
        batch = open_batch.setdefault(c.prog.exe, Job([], 0.0))
        batch.checks.append(c)
        batch.expected += took
        if batch.expected >= BATCH_S:
            jobs.append(open_batch.pop(c.prog.exe))
    jobs.extend(open_batch.values())
    return sorted(jobs, key=lambda j: (j.expected is not None, -(j.expected or 0.0), j.checks[0].key))


def run_job(job):
    """Run the checks of `job` as one process; answer [(key, name, seconds, state, the words said)] a check, the time of
    the process shared among them. A check libtest lists as failed is "FAILED", and
    so is every check of a process that ended badly with no list of its failures."""
    prog = job.checks[0].prog
    began = time.time()
    try:
        out = subprocess.run(
            [prog.exe, "--exact"] + [c.name for c in job.checks] + ["--test-threads=1"],
            cwd=prog.crate_dir, capture_output=True, text=True, env=check_env(prog), timeout=CHECK_TIMEOUT_S,
        )
        said, ended_well = out.stdout + out.stderr, out.returncode == 0
    except subprocess.TimeoutExpired as e:
        said, ended_well = f"{e.stdout or ''}{e.stderr or ''}\nstopped: still running after {CHECK_TIMEOUT_S} s", False
    share = (time.time() - began) / len(job.checks)
    states = states_of([c.name for c in job.checks], said, ended_well)
    return [(c.key, c.name, share, states[c.name], said) for c in job.checks]


def states_of(names, said, ended_well):
    """THE STATE OF EACH OF `names` run in one process that said `said`: all "ok" when it ended well; else "FAILED" for
    those libtest lists as failed at its end - read off that list, not off the line of each check, as OCCT prints into
    that line (the IGES reader's count of entities) and the "ok" lands on a line of its own - and for all of them when
    it ended badly with no list: it stopped, it crashed, it aborted after its checks."""
    failed = set(re.findall(r"^    (\S+)$", said.rpartition("\nfailures:\n")[2], re.M)) if "\nfailures:\n" in said else set()
    return {n: "ok" if ended_well or (failed and n not in failed) else "FAILED" for n in names}


def pool(threads, share=(1, 1)):
    """THE CHECKS OF EVERY CRATE BUT THE ACCEPTANCE SET, run as one pool of processes (`EVERY_CRATE`). Speaks as `cargo
    test` does - the build's diagnostics, then `test NAME ... ok|FAILED|ignored` a check - so the gate reads it the same
    way; the words of a red check follow its line. Answers the code to exit with."""
    from concurrent.futures import ThreadPoolExecutor, as_completed

    build = subprocess.run(CRATES + ["--no-run", "--message-format=json-render-diagnostics"], cwd=ROOT, capture_output=True, text=True)
    # the diagnostics as cargo renders them: a warning among them is red to the gate
    sys.stdout.write(build.stderr)
    if build.returncode != 0:
        return build.returncode
    # the matrices kept for the release are not run here (`RELEASE_ONLY`)
    progs = [p for p in test_programs(build.stdout) if not (p.crate == "qymcad-testkit" and p.key.split("/")[1] in RELEASE_ONLY)]
    checks = []
    with ThreadPoolExecutor(max_workers=threads) as ex:
        # the lists asked of every program side by side: one after another they were 762 starts of a program
        lists = list(ex.map(lambda p: (p, listed(p, ignored=False), set(listed(p, ignored=True))), progs))
    for prog, names, passed_over in lists:
        for name in names:
            if name in passed_over:
                print(f"test {name} ... ignored")
            else:
                checks.append(Check(prog, name))
    # A SHARE OF THE CHECKS, for one of several machines: every k-th of them by name, so each check is in exactly one
    # share whatever times each machine remembers
    k, n = share
    checks = [c for i, c in enumerate(sorted(checks, key=lambda c: c.key)) if i % n == k - 1]
    try:
        with open(CHECK_TIMES, encoding="utf-8") as f:
            before = read_times(f.read())
    except OSError:
        before = {}
    jobs = jobs_of(checks, before)
    print(f"{len(checks)} checks of {len(progs)} programs in {len(jobs)} processes, {threads} side by side", flush=True)
    took, red = {}, 0
    with ThreadPoolExecutor(max_workers=threads) as ex:
        for done in as_completed([ex.submit(run_job, j) for j in jobs]):
            for key, name, seconds, state, said in done.result():
                took[key] = seconds
                print(f"test {name} ... {state}", flush=True)
                if state == "FAILED":
                    red += 1
                    print(f"---- {key} ----\n{said}", flush=True)
    # the times of this run over those of the run before: a share times its own checks and keeps the others' as they were
    os.makedirs(os.path.dirname(CHECK_TIMES), exist_ok=True)
    with open(CHECK_TIMES, "w", encoding="utf-8") as f:
        f.writelines(f"{t:.2f}\t{key}\n" for key, t in sorted({**before, **took}.items(), key=lambda kv: -kv[1]))
    if k != 1:
        return 1 if red else 0
    # THE EXAMPLES IN THE DOCUMENTATION, run by `cargo test` after the programs (by the first share): none today, and a
    # new one is run
    doc = subprocess.run(CRATES + ["--doc"], cwd=ROOT, capture_output=True, text=True)
    sys.stdout.write(doc.stdout + doc.stderr)
    return 1 if red or doc.returncode != 0 else 0


class Machine:
    """What a run of the acceptance set has to work with: logical CPUs, memory it may take in megabytes, and whether a
    person works at it."""

    def __init__(self, cpus, memory_mb, desktop):
        self.cpus, self.memory_mb, self.desktop = cpus, memory_mb, desktop


class Pick:
    """How many acceptance probes run side by side, and the words that say why."""

    def __init__(self, threads, why):
        self.threads, self.why = threads, why


def pick_threads(machine, asked):
    """THE NUMBER OF PROBES SIDE BY SIDE on `machine`: what `asked` (the value of QYMCAD_TEST_THREADS, or None) says,
    else the fewer of the threads - less a tenth on a desktop - and the memory over the memory of one probe; at least
    one. A value asked for that is not a whole number above zero is refused: ValueError with the words."""
    if asked is not None and asked.strip():
        try:
            threads = int(asked)
        except ValueError:
            threads = 0
        if threads < 1:
            raise ValueError(f"{THREADS_VAR} takes a whole number above zero, not {asked!r}")
        return Pick(threads, f"{THREADS_VAR}={threads}")
    left = max(1, machine.cpus // 10) if machine.desktop else 0
    by_cpus = max(1, machine.cpus - left)
    by_memory = max(1, machine.memory_mb // PROBE_MB)
    spare = f" less {left} for the desktop" if left else ""
    why = f"{machine.cpus} threads{spare} -> {by_cpus}; {machine.memory_mb / 1024:.1f} GB / {PROBE_MB} MB a probe -> {by_memory}"
    return Pick(min(by_cpus, by_memory), why)


def read_mb(text, key):
    """The value of `key` in the text of /proc/meminfo, in megabytes; None when it is not there."""
    for line in text.splitlines():
        if line.startswith(key + ":"):
            return int(line.split()[1]) // 1024
    return None


def this_machine():
    """THE MACHINE THE GATE RUNS ON: the CPUs this process may use, and the memory a run may take - what the system
    has available, no more than the cap the gate puts on a desktop, nor than the memory limit of the group the process
    runs in (a container of the CI)."""
    cpus = len(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else (os.cpu_count() or 1)
    bounds = []
    try:
        with open("/proc/meminfo", encoding="ascii") as f:
            available = read_mb(f.read(), "MemAvailable")
        if available:
            bounds.append(available)
    except OSError:
        pass
    if CAP:
        bounds.append(CAP_GB * 1024)
    try:
        with open("/sys/fs/cgroup/memory.max", encoding="ascii") as f:
            limit = f.read().strip()
        if limit.isdigit():
            bounds.append(int(limit) // (1024 * 1024))
    except OSError:
        pass
    return Machine(cpus, min(bounds) if bounds else CAP_GB * 1024, bool(CAP))


def slowest(text, count):
    """The `count` slowest probes of the times the runner wrote (`seconds<TAB>name` a line), slowest first, as
    (seconds, name); a line that is not of that shape is passed over."""
    times = []
    for line in text.splitlines():
        seconds, _, name = line.partition("\t")
        try:
            times.append((float(seconds), name))
        except ValueError:
            continue
    return sorted(times, key=lambda t: -t[0])[:count]


def self_test():
    """THE GATE'S OWN RECKONING on cases, each with the answer it must give."""
    wrong = []

    def expect(what, got, want):
        if got != want:
            wrong.append(f"{what}: {got!r}, not {want!r}")

    gb = 1024
    expect("the owner's desktop, 20 threads, under the cap of 16 GB", pick_threads(Machine(20, 16 * gb, True), None).threads, 18)
    expect("a runner of the CI, 4 threads, 15 GB", pick_threads(Machine(4, 15 * gb, False), None).threads, 4)
    expect("a laptop, 4 threads, 2 GB available", pick_threads(Machine(4, 2 * gb, True), None).threads, 2)
    expect("one thread on a desktop", pick_threads(Machine(1, 16 * gb, True), None).threads, 1)
    expect("no memory to speak of", pick_threads(Machine(8, 100, False), None).threads, 1)
    expect("asked by hand", pick_threads(Machine(20, 16 * gb, True), "6").threads, 6)
    expect("asked empty is not asked", pick_threads(Machine(20, 16 * gb, True), " ").threads, 18)
    for bad in ("0", "-2", "many"):
        try:
            pick_threads(Machine(20, 16 * gb, True), bad)
            wrong.append(f"{THREADS_VAR}={bad!r} was taken")
        except ValueError:
            pass
    interleaved = "running 1 test\ntest iges::read ... \x1b[32;1mTotal number of loaded entities 49.\n\x1b[0m\nok\n\ntest result: ok. 1 passed"
    expect("a check OCCT printed into the line of", states_of(["iges::read"], interleaved, True), {"iges::read": "ok"})
    listed = "test a ... ok\ntest b ... FAILED\n\nfailures:\n\n---- b stdout ----\npanicked\n\nfailures:\n    b\n\ntest result: FAILED"
    expect("the checks libtest lists as failed", states_of(["a", "b"], listed, False), {"a": "ok", "b": "FAILED"})
    expect("a process that aborted after its checks", states_of(["a", "b"], "test a ... ok\ntest b ... ok\n", False), {"a": "FAILED", "b": "FAILED"})
    expect("the listing of the probes of a share has no placeholder of threads", PICKED in listing_of(ACCEPTANCE + [PICKED, "--skip", "part_"]), False)
    meminfo = "MemTotal:       32000000 kB\nMemAvailable:   18061028 kB\n"
    expect("the available memory of /proc/meminfo", read_mb(meminfo, "MemAvailable"), 17637)
    expect("a key that is not there", read_mb(meminfo, "SwapTotal"), None)
    times = "1.5\tpart::a\n30.2\tsketch::b\nnonsense\n7.0\tasm::c\n"
    expect("the slowest two", slowest(times, 2), [(30.2, "sketch::b"), (7.0, "asm::c")])
    expect("the times of the run before", read_times(times), {"part::a": 1.5, "sketch::b": 30.2, "asm::c": 7.0})
    built = "\n".join([
        "not json",
        json.dumps({"reason": "compiler-artifact", "manifest_path": "/w/crates/qymcad-core/Cargo.toml", "target": {"name": "feature", "kind": ["test"]}, "profile": {"test": True}, "executable": "/w/target/debug/deps/feature-1"}),
        json.dumps({"reason": "compiler-artifact", "manifest_path": "/w/crates/qymcad-core/Cargo.toml", "target": {"name": "qymcad_core", "kind": ["lib"]}, "profile": {"test": False}, "executable": None}),
        json.dumps({"reason": "build-finished", "success": True}),
    ])
    progs = test_programs(built)
    expect("the test programs of the build", [(p.exe, p.crate_dir, p.crate, p.key) for p in progs], [("/w/target/debug/deps/feature-1", "/w/crates/qymcad-core", "qymcad-core", "qymcad-core/feature/test")])
    a, b, c, d, e = (Check(progs[0], n) for n in ("a", "b", "c", "d", "e"))
    known = {a.key: 3.0, b.key: 90.0, d.key: 0.2, e.key: 0.3}
    expect(
        "the processes of the pool: the untimed alone first, then the slowest, the quick ones together",
        [[x.name for x in j.checks] for j in jobs_of([a, b, c, d, e], known)],
        [["c"], ["b"], ["a"], ["d", "e"]],
    )
    for w in wrong:
        print(f"  wrong: {w}")
    print("the gate's reckoning: " + ("right" if not wrong else f"{len(wrong)} wrong"))
    return 0 if not wrong else 1


def main():
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    if sys.argv[1:2] == ["--pool"]:
        # the pool of the checks of every crate, run by the step `EVERY_CRATE` with the threads picked, and a share of it
        share = next((tuple(int(x) for x in a.partition("=")[2].split("/")) for a in sys.argv[3:] if a.startswith("--share=")), (1, 1))
        return pool(int(sys.argv[2].partition("=")[2]), share)
    ap = argparse.ArgumentParser(description="Run a level of the checks and report it.")
    ap.add_argument("level", choices=sorted(LEVELS))
    ap.add_argument("--step", help="run only the step whose name begins with this")
    ap.add_argument("--except", dest="skip", help="run every step but the one whose name begins with this")
    ap.add_argument("--skip-probe", action="append", default=[], help="leave the acceptance probes matching this out (repeatable)")
    ap.add_argument("--shard", help="K/N: of the acceptance probes and of the checks of every crate, run only the K-th of N shares (1-based)")
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
        steps = [
            (name, shard_of(cmd, env, k, n), env) if cmd[: len(ACCEPTANCE)] == ACCEPTANCE else (name, cmd + [f"--share={k}/{n}"], env) if "--pool" in cmd else (name, cmd, env)
            for name, cmd, env in steps
        ]
    if any(PICKED in cmd for _, cmd, _ in steps):
        try:
            pick = pick_threads(this_machine(), os.environ.get(THREADS_VAR))
        except ValueError as e:
            print(e)
            return 2
        print(f"acceptance probes and checks side by side: {pick.threads} ({pick.why})", flush=True)
        steps = [(name, [f"--test-threads={pick.threads}" if a == PICKED else a for a in cmd], env) for name, cmd, env in steps]
    whole = args.step is None and args.skip is None and args.shard is None and not args.skip_probe
    total_began = time.time()
    all_red, broken_steps = [], []
    for name, cmd, env in steps:
        times = None
        if cmd[: len(ACCEPTANCE)] == ACCEPTANCE:
            times = os.path.join(ROOT, "target", f"gate-{args.level}-probe-times.tsv")
            os.makedirs(os.path.dirname(times), exist_ok=True)
            open(times, "w", encoding="utf-8").close()
            env = {**env, TIMES_VAR: times}
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
        if "--pool" in cmd:
            times = CHECK_TIMES
        if times:
            with open(times, encoding="utf-8") as f:
                for took, probe in slowest(f.read(), SLOWEST):
                    print(f"    {took:6.1f} s  {probe}")
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
