#!/usr/bin/env python3
"""Convert a .qcad document to the record shape the code speaks today.

The project changes its format straight, without readers for the old shape: a renamed field does not
fail to load, `serde` fills a default, and the part quietly builds differently. This script is the
other half of that decision - the one-shot that brings an existing document forward.

    python3 convert_qcad.py part.qcam                 # renames to part.qcad, keeping part.qcam.bak
    python3 convert_qcad.py part.qcad                 # in place, keeping part.qcad.bak
    python3 convert_qcad.py part.qcam converted.qcad  # leaving the original alone

A document saved under the old extension is renamed as it is converted: the product is QymCAD and the
extension `.qcam` was left over from its former name.

A bare `.ron` is taken as the document itself, which is what the test fixtures of the repository are.
In a bundle only `document.ron` is rewritten; every other entry (meshes, faces, B-rep blobs) is copied
through byte for byte.

CONVERSIONS, newest first. Add to this list when a record changes shape - each entry is a node kind,
the keys it used to carry and the key it carries now. Every step is written so that a document at ANY
earlier stage lands on the current shape in one pass.

    (any)    faces/edges as a bare list of ids     -> a Ref: (query: Ids([...]), expect: Some, hint: (...))
    NamedDim sketch + a + b                       -> target: Sketch(sketch: _, refs: [_, _])
    Extrude  symmetric + flip                    -> reach: Forward | Backward | BothWays
    Revolve  symmetric + flip, then turn: (...)  -> reach: Forward | Backward | BothWays
    Combine  extent: (through, flip, symmetric)  -> extent: (through: _, reach: _)
    Shell    outward + center                    -> side: Inward | Outward | Centred
"""
import pathlib
import re
import shutil
import sys
import zipfile


def kind_span(text, at):
    """The parentheses of the `kind: Name(` that starts at `at`, as (open, close) indices."""
    open_at = text.index("(", at)
    depth, i, in_string = 0, open_at, False
    while i < len(text):
        c = text[i]
        if in_string:
            if c == "\\":
                i += 2
                continue
            if c == '"':
                in_string = False
        elif c == '"':
            in_string = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return open_at, i
        i += 1
    raise ValueError("unbalanced parentheses in document.ron")


def direct_field(block, name):
    """A `name: value,` line of the block itself, not of anything nested in it."""
    return re.search(r"^(\s*)%s: ([^,\n]+),\s*$" % name, block, re.M)


def convert_shell(block):
    outward, center = direct_field(block, "outward"), direct_field(block, "center")
    if not (outward and center):
        return block, False
    side = "Centred" if center.group(2) == "true" else ("Outward" if outward.group(2) == "true" else "Inward")
    block = block.replace(outward.group(0) + "\n", "%sside: %s,\n" % (outward.group(1), side))
    return block.replace(center.group(0) + "\n", ""), True


def reach_word(symmetric, flip):
    """The three states the pair of booleans used to encode, with its fourth (both true) folded in.

    `symmetric` always won over `flip`, so `(true, true)` and `(true, false)` produced the same extent.
    """
    if symmetric == "true":
        return "BothWays"
    return "Backward" if flip == "true" else "Forward"


def pair_to_reach(block):
    """`symmetric` + `flip`, in any order and not necessarily next to each other, become one `reach`."""
    sym, flip = direct_field(block, "symmetric"), direct_field(block, "flip")
    if not (sym and flip):
        return block, False
    block = block.replace(sym.group(0) + "\n", "%sreach: %s,\n" % (sym.group(1), reach_word(sym.group(2), flip.group(2))))
    return block.replace(flip.group(0) + "\n", ""), True


def convert_extrude(block):
    return pair_to_reach(block)


def convert_revolve(block):
    block, done = pair_to_reach(block)
    if done:
        return block, True
    # the intermediate shape: the pair had already become a `turn` of its own
    turn = re.search(r"^(\s*)turn: \(symmetric: (\w+), flip: (\w+)\),\s*$", block, re.M)
    if not turn:
        return block, False
    return block.replace(turn.group(0) + "\n", "%sreach: %s,\n" % (turn.group(1), reach_word(turn.group(2), turn.group(3)))), True


def convert_combine(block):
    # the oldest shape: three loose keys in the record itself
    through, flip, sym = direct_field(block, "through"), direct_field(block, "flip"), direct_field(block, "symmetric")
    if through and flip and sym:
        extent = "%sextent: (through: %s, reach: %s),\n" % (through.group(1), through.group(2), reach_word(sym.group(2), flip.group(2)))
        block = block.replace(through.group(0) + "\n", extent)
        block = block.replace(flip.group(0) + "\n", "")
        return block.replace(sym.group(0) + "\n", ""), True
    # the intermediate shape: an `extent` that still carried the pair
    ext = re.search(r"^(\s*)extent: \(through: (\w+), flip: (\w+), symmetric: (\w+)\),\s*$", block, re.M)
    if not ext:
        return block, False
    same = "%sextent: (through: %s, reach: %s),\n" % (ext.group(1), ext.group(2), reach_word(ext.group(4), ext.group(3)))
    return block.replace(ext.group(0) + "\n", same), True


# Only these two fields of a FEATURE are references; the same words elsewhere (the name table keeps its own
# `edges: []`) are plain lists and must not be touched.
LIST_REF_FIELDS = ("faces", "edges")


def one_list_to_ref(text, name, at):
    """Rewrite `name: [ids]` at `at` into a `Ref`, or return None if it is not a plain list of ids."""
    close = text.index("]", at)
    ids = [x.strip() for x in text[at + len(name) + 3 : close].strip("[] \n").replace("\n", " ").split(",") if x.strip()]
    if not all(i.isdigit() for i in ids):
        return None, close + 1
    indent = " " * (at - text.rfind("\n", 0, at) - 1)
    query = "Id(%s)" % (ids[0] if ids else "0") if len(ids) <= 1 else "Ids([%s])" % ", ".join(ids)
    expect = "Any" if not ids else "Some"
    ref = ("%s: (\n%s    query: %s,\n%s    expect: %s,\n%s    hint: (centroid: (0.0, 0.0, 0.0), normal: (0.0, 0.0, 0.0)),\n%s)"
           % (name, indent, query, indent, expect, indent, indent))
    return ref, close + 1


def named_dims_to_targets(text):
    """A named dimension used to name the two entities it hangs on; now it names a TARGET.

    Old: `(name: "14", sketch: 4024, a: 4029, b: 4026)`. A pair of entities only ever covered a distance
    between two points, which is why the field became a `DimTarget`: a sketch dimension carries however many
    references its kind needs, and a feature parameter is a node plus a key.
    """
    at = text.find("named_dims: [")
    if at < 0:
        return text, 0
    open_at = text.index("[", at)
    depth, i = 0, open_at
    while i < len(text):
        if text[i] == "[":
            depth += 1
        elif text[i] == "]":
            depth -= 1
            if depth == 0:
                break
        i += 1
    block, n = text[open_at : i + 1], 0
    out, pos = [], 0
    while True:
        m = re.search(r"(\s*)name: (\"[^\"]*\"),\s*\n\s*sketch: (\d+),\s*\n\s*a: (\d+),\s*\n\s*b: (\d+),", block[pos:])
        if not m:
            out.append(block[pos:])
            break
        ind = m.group(1).lstrip("\n")
        out.append(block[pos : pos + m.start()])
        out.append("%sname: %s,\n%starget: Sketch(sketch: %s, refs: [%s, %s])," % (m.group(1), m.group(2), ind, m.group(3), m.group(4), m.group(5)))
        pos += m.end()
        n += 1
    return text[:open_at] + "".join(out) + text[i + 1 :], n


def lists_to_refs(text):
    """A reference to faces or edges used to be a bare list of ids; now it is a `Ref` with a query.

    The oldest documents hold `faces: [12, 13,]`. What the code reads today is a struct: the query, how many
    are expected, and a fingerprint for healing. A hand-picked set is exactly `Ref::picks` - one `Id` when
    there is one, `Ids` when there are several, `Some` expected either way.

    ONLY INSIDE A FEATURE'S `kind:`. The first edition rewrote every `edges: [...]` in the file and broke the
    name table, where that word means a plain list of numbers. A rule that does not know where it is does more
    harm than the shape it fixes.
    """
    n, pos, out = 0, 0, []
    while True:
        at = text.find("kind: ", pos)
        if at < 0:
            out.append(text[pos:])
            break
        try:
            open_at, close_at = kind_span(text, at)
        except ValueError:
            out.append(text[pos:])
            break
        body, bpos, bout = text[open_at : close_at + 1], 0, []
        while True:
            hits = [(body.find("%s: [" % f, bpos), f) for f in LIST_REF_FIELDS]
            hits = [(i, f) for i, f in hits if i >= 0]
            if not hits:
                bout.append(body[bpos:])
                break
            i, f = min(hits)
            ref, after = one_list_to_ref(body, f, i)
            bout.append(body[bpos:i])
            if ref is None:
                bout.append(body[i:after])
            else:
                bout.append(ref)
                n += 1
            bpos = after
        out.append(text[pos:open_at])
        out.append("".join(bout))
        pos = close_at + 1
    return "".join(out), n


CONVERSIONS = [("Shell", convert_shell), ("Extrude", convert_extrude), ("Revolve", convert_revolve), ("Combine", convert_combine)]


def convert_document(text):
    counts = {}
    text, n = lists_to_refs(text)
    if n:
        counts["Ref"] = n
    text, n = named_dims_to_targets(text)
    if n:
        counts["NamedDim"] = n
    for kind, rewrite in CONVERSIONS:
        pieces, pos = [], 0
        while True:
            at = text.find("kind: %s(" % kind, pos)
            if at < 0:
                pieces.append(text[pos:])
                break
            open_at, close_at = kind_span(text, at)
            block, done = rewrite(text[open_at + 1 : close_at])
            counts[kind] = counts.get(kind, 0) + (1 if done else 0)
            pieces.append(text[pos : open_at + 1])
            pieces.append(block)
            pieces.append(")")
            pos = close_at + 1
        text = "".join(pieces)
    return text, counts


def report(src, dst, counts):
    if not counts or not any(counts.values()):
        print("nothing to convert: %s already speaks the current shape" % src)
    else:
        print("converted %s -> %s: %s" % (src, dst, ", ".join("%s %d" % (k, n) for k, n in counts.items() if n)))


def main(argv):
    if len(argv) not in (2, 3):
        print(__doc__)
        return 2
    src = argv[1]
    # A document under the OLD extension is renamed as it is converted; the original is kept beside it under
    # its own name, so nothing of the owner's is lost to a rename.
    default_dst = src[: -len(".qcam")] + ".qcad" if src.endswith(".qcam") else src
    dst = argv[2] if len(argv) == 3 else default_dst
    if src.endswith(".ron"):
        text, counts = convert_document(pathlib.Path(src).read_text())
        pathlib.Path(dst).write_text(text)
        report(src, dst, counts)
        return 0
    renamed = dst != src and len(argv) == 2
    if src == dst:
        # A second conversion must not overwrite the first backup: that one holds the ORIGINAL, and the
        # numbered ones hold each intermediate shape.
        backup, n = src + ".bak", 0
        while pathlib.Path(backup).exists():
            n += 1
            backup = "%s.bak.%d" % (src, n)
        shutil.copyfile(src, backup)
        print("the original is kept as %s" % backup)
    with zipfile.ZipFile(src) as bundle:
        entries = [(info, bundle.read(info.filename)) for info in bundle.infolist()]
    counts = {}
    for i, (info, data) in enumerate(entries):
        if info.filename == "document.ron":
            text, counts = convert_document(data.decode("utf-8"))
            entries[i] = (info, text.encode("utf-8"))
    with zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED) as out:
        for info, data in entries:
            out.writestr(info, data)
    report(src, dst, counts)
    if renamed:
        print("the old file stays as %s" % src)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
