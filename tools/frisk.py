#!/usr/bin/env python3
"""NOTHING PERSONAL GOES OUT: tracked files and commit messages are searched for a home directory, a personal
mail address and a reference to a document that stays private.

The repository is public and every pushed commit is read by everybody, so this runs on every pull request and
push (CI) and before every push by hand. The search holds nothing personal itself: the general shapes are
here, and the words that name one person - a login, an address - come from a file outside the tree
(`QYMCAD_FRISK_EXTRA`, by default `../qymcad-internal/frisk-extra.txt`, one regular expression per line).
Without that file only the general shapes are searched.

    tools/frisk.py                     every tracked file
    tools/frisk.py --commits A..B      the messages of the commits in a range too
    tools/frisk.py --self-test         the search catches what it is for, and lets placeholders pass
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile

# Names examples and tests write paths with. They point at nobody; a search that went red on them would be
# switched off within a week, together with what it is for.
PLACEHOLDER_HOMES = {"user", "username", "you", "someone", "runner", "root", "runneradmin", "name", "me"}

# Domains of personal mailboxes. An address of a project or of a library author in a licence notice is not
# personal in this sense and is not searched for.
PERSONAL_MAIL = r"(?:gmail|googlemail|yandex|ya|mail|inbox|list|bk|outlook|hotmail|live|icloud|me|proton|protonmail)"

GENERAL = [
    (re.compile(r"/home/([a-z][a-z0-9_-]*)", re.I), "a home directory", lambda m: m.group(1).lower() not in PLACEHOLDER_HOMES),
    (re.compile(r"\bC:\\{1,2}Users\\{1,2}([A-Za-z][A-Za-z0-9_.-]*)", re.I), "a Windows home directory", lambda m: m.group(1).lower() not in PLACEHOLDER_HOMES),
    (re.compile(r"/Users/([A-Za-z][A-Za-z0-9_.-]*)/"), "a macOS home directory", lambda m: m.group(1).lower() not in PLACEHOLDER_HOMES | {"shared"}),
    (re.compile(r"[A-Za-z0-9._%+-]+@" + PERSONAL_MAIL + r"\.[a-z]{2,}", re.I), "a personal mail address", None),
    (re.compile(r"docs/" + "archive"), "a reference to the private document archive", None),
    (re.compile(r"qymcad-" + "internal"), "a reference to the private repository", None),
]

# Binary files are not searched: a match in them is chance.
BINARY = {".png", ".jpg", ".jpeg", ".gif", ".ico", ".icns", ".ttf", ".otf", ".woff", ".woff2", ".zip", ".gz",
          ".xz", ".msi", ".exe", ".dll", ".so", ".a", ".bin", ".qcad", ".qpart", ".glb", ".3mf", ".stl"}


def extra_patterns(root):
    path = os.environ.get("QYMCAD_FRISK_EXTRA") or os.path.join(root, "..", "qymcad-internal", "frisk-extra.txt")
    if not os.path.exists(path):
        return []
    out = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if line and not line.startswith("#"):
            out.append((re.compile(line, re.I), "a word named in the personal list", None))
    return out


def search(text, where, patterns):
    found = []
    for i, line in enumerate(text.splitlines(), 1):
        for pattern, why, still_bad in patterns:
            for m in pattern.finditer(line):
                if still_bad is None or still_bad(m):
                    found.append(f"{where}:{i}: {why} -> {m.group(0)!r}")
    return found


def tracked_files(root):
    out = subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True).stdout
    return [p for p in out.decode().split("\0") if p]


# Files that never belong in the tree, whatever they hold.
NEVER_TRACKED = {"CLAUDE.local.md", "frisk-extra.txt"}


def search_files(root, patterns):
    found = []
    for rel in tracked_files(root):
        if os.path.basename(rel) in NEVER_TRACKED:
            found.append(f"{rel}: a personal file is tracked")
            continue
        if os.path.splitext(rel)[1].lower() in BINARY or rel == "tools/frisk.py":
            continue
        try:
            data = open(os.path.join(root, rel), "rb").read()
        except OSError:
            continue
        if b"\0" in data:
            continue
        found += search(data.decode("utf-8", "replace"), rel, patterns)
    return found


def search_commits(root, span, patterns):
    out = subprocess.run(["git", "log", "--format=%H%x00%B%x01", span], cwd=root, capture_output=True, text=True, check=True).stdout
    found = []
    for entry in out.split("\x01"):
        if "\x00" not in entry:
            continue
        sha, body = entry.strip("\n").split("\x00", 1)
        # the attribution lines name a tool account, not a person
        body = "\n".join(l for l in body.splitlines() if not l.startswith(("Co-Authored-By:", "Signed-off-by:")))
        found += search(body, f"commit {sha[:9]}", patterns)
    return found


def self_test():
    """The search catches each shape it is for and lets a placeholder pass."""
    must_catch = [
        "/home/alice/projects/x",
        r"C:\Users\Alice\AppData",
        "/Users/alice/Library",
        "write to alice.smith@gmail.com",
        "see " + "docs/" + "archive/OLD.md",
        "kept in qymcad-" + "internal",
    ]
    must_pass = ["/home/user/project", r"C:\Users\runneradmin\.cargo", "/Users/Shared/x", "noreply@anthropic.com",
                 "Copyright (c) Some Author <author@freetype.org>", "docs/help/en/part/01-extrude.md"]
    patterns = GENERAL
    bad = []
    for line in must_catch:
        if not search(line, "case", patterns):
            bad.append(f"not caught: {line!r}")
    for line in must_pass:
        if search(line, "case", patterns):
            bad.append(f"caught a placeholder: {line!r}")
    with tempfile.TemporaryDirectory() as d:
        extra = os.path.join(d, "extra.txt")
        open(extra, "w").write("# a personal word\nsomelogin\n")
        os.environ["QYMCAD_FRISK_EXTRA"] = extra
        if not search("author: somelogin", "case", extra_patterns(d)):
            bad.append("a word of the personal list was not caught")
    if bad:
        print("the search is blind:\n  " + "\n  ".join(bad))
        return 1
    print(f"the search catches {len(must_catch) + 1} shapes and lets {len(must_pass)} placeholders pass")
    return 0


def main():
    ap = argparse.ArgumentParser(description="search tracked files and commit messages for anything personal")
    ap.add_argument("--commits", help="also search the messages of the commits in this range, e.g. origin/main..HEAD")
    ap.add_argument("--self-test", action="store_true", help="check that the search catches what it is for")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
    patterns = GENERAL + extra_patterns(root)
    found = search_files(root, patterns)
    if args.commits:
        found += search_commits(root, args.commits, patterns)
    if found:
        print("something personal would go out:")
        for f in found[:60]:
            print("  " + f)
        if len(found) > 60:
            print(f"  ... and {len(found) - 60} more")
        return 1
    print("nothing personal found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
