#!/usr/bin/env python3
"""BREAKAGES ON ORDER: every path a person takes is broken on purpose, one line at a time, and the probe that stands
guard over it must go red.

The list is `tools/breakages.json`: for each breakage, the file, the exact text it is made in (found exactly once),
what that text becomes, and the probe expected to catch it. For every breakage the script checks the probe is green
without it, puts the breakage in, runs the probe, and takes the breakage out again - by writing the file back as it
was read, verified by its hash. A probe still green with the breakage in has missed it.

Safety, since the script edits the program's own sources:
  * a file with changes of its own that are not committed is not touched at all - the run refuses to start;
  * the file is written back in `finally`, whatever happened, and its hash must match the one read;
  * every cargo run goes under a memory cap without swap.

Results go line by line into `target/breakages.jsonl`, so a run cut short goes on where it stopped.

    tools/breakages.py            run every breakage not yet in the results
    tools/breakages.py --check    only see that every anchor is found exactly once
    tools/breakages.py --only N   run the breakage whose name is N, whatever the results say
    tools/breakages.py --fresh    forget the results and run everything
    tools/breakages.py --old "-p qymcad --lib gui::user_case"
                                  THE MEASURE BEFORE AN OLD CHECK IS RETIRED: with every breakage in, the old checks
                                  those arguments of `cargo test` pick are run as well, and each of them is told:
                                  which breakages it catches, and whether the new set misses any of those - an old
                                  check that catches what the new set misses stays. Its results are kept apart, in
                                  `target/breakages-old.jsonl`.
"""
import shlex
import argparse
import hashlib
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIST = os.path.join(ROOT, "tools", "breakages.json")
RESULTS = os.path.join(ROOT, "target", "breakages.jsonl")
CAP = ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=16G", "-p", "MemorySwapMax=0"]


def run_probe(probe):
    """Run one probe of the acceptance set; answer how it went: 'green', 'red', 'no build', 'no probe'."""
    cmd = CAP + ["cargo", "test", "-p", "qymcad-acceptance", "--test", "acceptance", "--", "--exact", probe, "--test-threads=1"]
    out = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    said = out.stdout + out.stderr
    if "could not compile" in said or "error[E" in said:
        return "no build", said
    # a failed probe carries its child's own result line in its words: only the last line is the run's
    results = [l for l in said.splitlines() if l.startswith("test result:")]
    last = results[-1] if results else ""
    if " 1 passed" in last:
        return "green", said
    if " 1 failed" in last:
        return "red", said
    return "no probe", said


TEST_LINE = __import__("re").compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)$")


def run_old(args):
    """Run the old checks `cargo test <args>` picks; answer (the green ones, the red ones, whether it built at all)."""
    out = subprocess.run(CAP + ["cargo", "test"] + args, cwd=ROOT, capture_output=True, text=True)
    said = out.stdout + out.stderr
    if "could not compile" in said or "error[E" in said:
        return set(), set(), False
    green, red = set(), set()
    for line in said.splitlines():
        m = TEST_LINE.match(line)
        if m:
            (green if m.group(2) == "ok" else red if m.group(2) == "FAILED" else set()).add(m.group(1))
    return green, red, True


def digest(data):
    return hashlib.sha256(data).hexdigest()


def committed(path):
    """Is the file as it is committed - nothing of its own that a restore could lose?"""
    out = subprocess.run(["git", "status", "--porcelain", "--", path], cwd=ROOT, capture_output=True, text=True)
    return out.returncode == 0 and out.stdout.strip() == ""


def load_results():
    done = {}
    if os.path.exists(RESULTS):
        for line in open(RESULTS, encoding="utf-8"):
            if line.strip():
                r = json.loads(line)
                done[r["name"]] = r
    return done


def record(result):
    os.makedirs(os.path.dirname(RESULTS), exist_ok=True)
    with open(RESULTS, "a", encoding="utf-8") as f:
        f.write(json.dumps(result, ensure_ascii=False) + "\n")


def main():
    ap = argparse.ArgumentParser(description="Breakages on order: each path broken on purpose must turn its probe red.")
    ap.add_argument("--check", action="store_true", help="only check that every anchor is found exactly once")
    ap.add_argument("--only", help="run the breakage of this name")
    ap.add_argument("--fresh", action="store_true", help="forget the results of earlier runs")
    ap.add_argument("--old", help="arguments of `cargo test` that pick old checks to measure alongside")
    args = ap.parse_args()
    global RESULTS
    old_args = shlex.split(args.old) if args.old else None
    if old_args is not None:
        RESULTS = os.path.join(ROOT, "target", "breakages-old.jsonl")

    breakages = json.load(open(LIST, encoding="utf-8"))
    wrong = []
    for b in breakages:
        text = open(os.path.join(ROOT, b["file"]), encoding="utf-8").read()
        if text.count(b["find"]) != 1:
            wrong.append(f"{b['name']}: its text is found {text.count(b['find'])} times in {b['file']}")
        # the probe is still there under its name: a renamed probe ran nothing and read as "red without the breakage"
        module, _, fn = b["probe"].rpartition("::")
        probe_file = os.path.join(ROOT, "crates", "qymcad-acceptance", "tests", "acceptance", module.split("::")[0] + ".rs")
        if "::" not in module and not (os.path.exists(probe_file) and f"fn {fn}(" in open(probe_file, encoding="utf-8").read()):
            wrong.append(f"{b['name']}: its probe {b['probe']} is not in {os.path.relpath(probe_file, ROOT)}")
    if wrong:
        print("the list does not fit the sources:\n  " + "\n  ".join(wrong))
        return 2
    if args.check:
        print(f"{len(breakages)} breakages, every one found exactly once")
        return 0

    if args.fresh and os.path.exists(RESULTS):
        os.remove(RESULTS)
    done = {} if args.only else load_results()
    todo = [b for b in breakages if (args.only is None or b["name"] == args.only) and b["name"] not in done]
    touched = sorted({b["file"] for b in todo})
    dirty = [f for f in touched if not committed(f)]
    if dirty:
        print("these files have changes of their own that are not committed, and a breakage would be made in them:\n  " + "\n  ".join(dirty))
        return 2

    # the old checks green without any breakage: only those can be said to catch one
    old_green = set()
    if old_args is not None:
        old_green, old_red, built = run_old(old_args)
        if not built:
            print("the old checks asked for do not build")
            return 2
        print(f"{len(old_green)} old checks green without any breakage" + (f", {len(old_red)} red already - left out" if old_red else ""))

    for b in todo:
        before, _ = run_probe(b["probe"])
        if before != "green":
            result = dict(name=b["name"], kind=b["kind"], bench=b["bench"], probe=b["probe"], outcome=f"{before} without the breakage")
            print(f"  ?  {b['name']}: the probe is {before} without the breakage, and proves nothing")
            record(result)
            continue
        path = os.path.join(ROOT, b["file"])
        original = open(path, "rb").read()
        sum_before = digest(original)
        try:
            broken = original.decode("utf-8").replace(b["find"], b["into"], 1)
            open(path, "w", encoding="utf-8").write(broken)
            after, said = run_probe(b["probe"])
            old_caught = []
            if old_args is not None:
                _, red_now, _ = run_old(old_args)
                old_caught = sorted(red_now & old_green)
        finally:
            # written back whatever happened - an interrupted run included
            open(path, "wb").write(original)
        if digest(open(path, "rb").read()) != sum_before:
            print(f"!! {b['file']} did not come back as it was - stop and look at it")
            return 3
        outcome = {"red": "caught", "green": "missed"}.get(after, after)
        result = dict(name=b["name"], kind=b["kind"], bench=b["bench"], probe=b["probe"], outcome=outcome)
        if old_args is not None:
            result["old_caught"] = old_caught
        if outcome != "caught":
            result["said"] = "\n".join(said.splitlines()[-30:])
        record(result)
        mark = {"caught": "ok", "missed": "--"}.get(outcome, "?")
        print(f"  {mark:2} {b['name']} ({b['kind']}): {outcome} by {b['probe']}")

    everything = load_results()
    if old_args is not None:
        # THE MEASURE: every old check with the breakages it catches, and the ones among them the new set misses
        by_old = {}
        for r in everything.values():
            for t in r.get("old_caught", []):
                by_old.setdefault(t, []).append(r)
        keep = {t: [r["name"] for r in rs if r["outcome"] != "caught"] for t, rs in by_old.items()}
        print("\nthe old checks measured:")
        for t in sorted(old_green):
            caught_here = [r["name"] for r in by_old.get(t, [])]
            if keep.get(t):
                print(f"  keep   {t}: catches what the new set misses - " + "; ".join(keep[t]))
            elif caught_here:
                print(f"  may go {t}: what it catches the new set catches too - " + "; ".join(caught_here))
            else:
                print(f"  ?      {t}: caught none of the breakages - the list does not reach what it guards")
    caught = [r for r in everything.values() if r["outcome"] == "caught"]
    others = [r for r in everything.values() if r["outcome"] != "caught"]
    print(f"\n{len(caught)} of {len(breakages)} breakages caught" + (f"; not caught: " + ", ".join(f"{r['name']} ({r['outcome']})" for r in others) if others else ""))
    return 0 if len(caught) == len(breakages) else 1


if __name__ == "__main__":
    sys.exit(main())
