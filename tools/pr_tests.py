#!/usr/bin/env python3
"""CODE COMES WITH ITS CHECKS: a pull request that changes the code of a crate and adds or changes no check is refused.

    python3 tools/pr_tests.py BASE HEAD     judge the change between two commits
    python3 tools/pr_tests.py --self-test   check that the judgement tells the cases apart

A check is any of: a file under a crate's `tests/`, a source file named `tests.rs` or `*_tests.rs`, an acceptance
contract (`crates/qymcad-acceptance/src/tools/`), the hand-driven scenario (`user_case.rs`), an added `#[test]`
anywhere, or a changed line below the first `#[cfg(test)]` of a source file - the test module at its end. A change that needs none - a comment, a word of the catalogue, a typo - is let through by the label
`no-test-needed`, which a maintainer puts on the pull request (`PR_LABELS`, a comma-separated list).

What this cannot tell is whether the check covers the change and was red before it: that is written in the pull
request and read at review.
"""
import os
import re
import subprocess
import sys

EXEMPT_LABEL = "no-test-needed"
CODE = re.compile(r"^crates/[^/]+/(src|build\.rs)")
CHECK_PATHS = [
    re.compile(r"^crates/[^/]+/tests/"),
    re.compile(r"(^|/)(tests|[a-z0-9_]+_tests)\.rs$"),
    re.compile(r"^crates/qymcad-acceptance/src/tools/"),
    re.compile(r"(^|/)user_case\.rs$"),
]
ADDED_TEST = re.compile(r"^\+\s*#\[(tokio::)?test\]")


def verdict(files, added_lines, labels, in_tests=()):
    """(passed, words) for the changed paths, the added lines of the diff of Rust files, and the labels. `in_tests` are
    the source files whose every changed line lies in the test module: they are checks, not code."""
    is_check = lambda f: any(p.search(f) for p in CHECK_PATHS) or f in in_tests
    code = [f for f in files if f.endswith(".rs") and CODE.match(f) and not is_check(f)]
    if not code:
        return True, "no code of a crate is changed outside its checks"
    checks = [f for f in files if f.endswith(".rs") and is_check(f)]
    tests_added = sum(1 for l in added_lines if ADDED_TEST.match(l))
    if checks or tests_added:
        named = checks[:5] + ([f"{tests_added} added #[test]"] if tests_added else [])
        return True, f"{len(code)} code file(s) changed, with checks: " + ", ".join(named)
    if EXEMPT_LABEL in labels:
        return True, f"{len(code)} code file(s) changed with no check; let through by the label {EXEMPT_LABEL}"
    return False, (
        f"{len(code)} code file(s) changed and no check added or changed:\n"
        + "".join(f"  {f}\n" for f in code[:20])
        + "Add a check that is red without the change and green with it - a #[test] beside the code, a file in the\n"
        "crate's tests/, or, for a tool, its acceptance contract and a step of user_case. A change that needs no\n"
        f"check (a comment, a word, a typo) is marked by a maintainer with the label {EXEMPT_LABEL}."
    )


def first_test_line(lines):
    """The 1-based number of the first `#[cfg(test)]` line, or None when the file has no test module."""
    for i, l in enumerate(lines, 1):
        if l.strip() == "#[cfg(test)]":
            return i
    return None


HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")


def only_in_tests(span, head, path):
    """Whether every line the change touches in `path` lies at or below its first `#[cfg(test)]` in `head`."""
    try:
        lines = git("show", f"{head}:{path}").splitlines()
    except subprocess.CalledProcessError:
        return False  # deleted
    start = first_test_line(lines)
    if start is None:
        return False
    for l in git("diff", "-U0", span, "--", path).splitlines():
        m = HUNK.match(l)
        if m and int(m.group(1)) < start:
            return False
    return True


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def self_test():
    labels = []
    cases = [
        ("docs only", ["README.md"], [], labels, True),
        ("code with no check", ["crates/qymcad-part/src/fillet.rs"], ["+let r = 2.0;"], labels, False),
        ("code with a #[test] beside it", ["crates/qymcad-part/src/fillet.rs"], ["+    #[test]"], labels, True),
        ("code with a file in tests/", ["crates/qymcad-core/src/a.rs", "crates/qymcad-core/tests/a.rs"], [], labels, True),
        ("code with a contract", ["crates/qymcad/src/x.rs", "crates/qymcad-acceptance/src/tools/shell.rs"], [], labels, True),
        ("code with a step of the scenario", ["crates/qymcad/src/x.rs", "crates/qymcad/src/gui/user_case.rs"], [], labels, True),
        ("only a test module changed", ["crates/qymcad-i18n/src/tests.rs"], [], labels, True),
        ("code with no check, labelled", ["crates/qymcad/src/x.rs"], [], [EXEMPT_LABEL], True),
        ("a commented-out test is no test", ["crates/qymcad/src/x.rs"], ["+// #[test]"], labels, False),
    ]
    bad = [name for name, files, added, lab, want in cases if verdict(files, added, lab)[0] != want]
    # a guard mended inside its own test module, beside code changed elsewhere
    if not verdict(["crates/qymcad/src/guard.rs", "crates/qymcad/src/x.rs"], [], labels, ["crates/qymcad/src/guard.rs"])[0]:
        bad.append("a change inside a test module is not seen as a check")
    if verdict(["crates/qymcad/src/x.rs"], [], labels, [])[0]:
        bad.append("a change above the test module is taken for a check")
    lines = ["fn a() {}", "#[cfg(test)]", "mod tests {", "}"]
    if first_test_line(lines) != 2 or first_test_line(["fn a() {}"]) is not None:
        bad.append("the test module is not found where it starts")
    if bad:
        print("the judgement is blind:\n  " + "\n  ".join(bad))
        return 1
    print(f"the judgement tells {len(cases) + 3} cases apart")
    return 0


def main():
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    base, head = sys.argv[1], sys.argv[2]
    span = f"{base}...{head}"
    files = git("diff", "--name-only", span).split()
    added = [l for l in git("diff", "-U0", span, "--", "*.rs").splitlines() if l.startswith("+") and not l.startswith("+++")]
    labels = [l.strip() for l in os.environ.get("PR_LABELS", "").split(",") if l.strip()]
    in_tests = [f for f in files if f.endswith(".rs") and CODE.match(f) and only_in_tests(span, head, f)]
    passed, words = verdict(files, added, labels, in_tests)
    print(words)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
