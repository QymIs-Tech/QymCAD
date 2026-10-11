#!/usr/bin/env python3
"""Convert a .qcad document to the record shape the code speaks today.

The project changes its format straight, without readers for the old shape: a renamed field does not
fail to load, `serde` fills a default, and the part quietly builds differently. This script is the
other half of that decision - the one-shot that brings an existing document forward.

    python3 convert_qcad.py --self-test               # check conversions and safe refusals
    python3 convert_qcad.py part.qcam                 # renames to part.qcad, keeping part.qcam.bak
    python3 convert_qcad.py part.qcad                 # in place, keeping part.qcad.bak
    python3 convert_qcad.py part.qcam converted.qcad  # leaving the original alone
    python3 convert_qcad.py --interactive part.qcad   # choose among ambiguous named dimensions

A document saved under the old extension is renamed as it is converted: the product is QymCAD and the
extension `.qcam` was left over from its former name.

A bare `.ron` is taken as the document itself, which is what the test fixtures of the repository are.
In a bundle only `document.ron` is rewritten; every other entry (meshes, faces, B-rep blobs) is copied
through byte for byte.

CONVERSIONS, newest first. Add to this list when a record changes shape - each entry is a node kind,
the keys it used to carry and the key it carries now. Every step is written so that a document at ANY
earlier stage lands on the current shape in one pass.

    NamedDim target Sketch refs                  -> target Sketch dimension: (id), bound to one constraint
    (any)    edge_refs/face_refs as bare triples   -> (id: _, at: (_, _, _), dir: (_, _, _))
    (any)    faces/edges as a bare list of ids     -> a Ref: (query: Ids([...]), expect: Some, hint: (...))
    NamedDim sketch + a + b                       -> target: Sketch(sketch: _, refs: [_, _])
    Extrude  symmetric + flip                    -> reach: Forward | Backward | BothWays
    Revolve  symmetric + flip, then turn: (...)  -> reach: Forward | Backward | BothWays
    Combine  extent: (through, flip, symmetric)  -> extent: (through: _, reach: _)
    Shell    outward + center                    -> side: Inward | Outward | Centred

A sketch target written as participant ids is converted only when exactly one dimensional constraint
in that sketch matches them. Multiple axes or kinds on the same participants cannot be distinguished
from the old target. Missing targets and duplicate stored identities stop the entire conversion before
any file or backup is written. Ambiguous targets also stop it unless `--interactive` was supplied:
that flag offers a numbered console list, not a drawing. Enter or q, EOF and Ctrl+C cancel the whole
conversion without writes, even after earlier choices. An overwritten name cannot be recovered from
that target. This remains a separate one-shot converter, not an old-format reader in the application.
"""
from dataclasses import dataclass
from enum import Enum
import pathlib
import re
import shutil
import sys
import zipfile


@dataclass(frozen=True)
class RonToken:
    spelling: str
    region: slice


@dataclass(frozen=True)
class RonField:
    key: int
    value: range


@dataclass(frozen=True)
class RonDocument:
    text: str
    tokens: list[RonToken]
    closing: dict[int, int]
    root: range


@dataclass(frozen=True)
class TextEdit:
    region: slice
    replacement: str


@dataclass(frozen=True)
class LegacyDimensionTarget:
    name: str
    sketch: int
    refs: RonField
    participants: frozenset[int]


@dataclass(frozen=True)
class DimensionCandidate:
    kind: str
    index: int
    record: range
    fields: dict[str, RonField]
    participants: frozenset[int]
    identity: int


@dataclass(frozen=True)
class SketchDimensions:
    by_sketch: dict[int, list[DimensionCandidate]]
    maximum_identity: int


@dataclass(frozen=True)
class DimensionConversion:
    text: str
    count: int


class DimensionSelection(Enum):
    UNIQUE_ONLY = "unique_only"
    ASK = "ask"


@dataclass(frozen=True)
class ConsoleExercise:
    argv: list[str]
    replies: list


@dataclass(frozen=True)
class ConsoleResult:
    code: object
    output: str
    prompts: list[str]
    error: object


RON_WORD = re.compile(r"[A-Za-z_][A-Za-z_0-9]*|[0-9][A-Za-z_0-9.]*|[^\s]")
RON_RAW = re.compile(r'r(#*)"')
U64_MAX = 18446744073709551615
DIMENSION_PARTICIPANTS = {
    "Distance": ["a", "b"],
    "Angle": ["a", "b", "c"],
    "AngleLines": ["a", "b", "c", "d"],
    "Diameter": ["c"],
    "DistancePL": ["p", "a", "b"],
    "EdgeDistance": ["c1", "c2"],
    "ArcLength": ["c", "a", "b"],
}


def read_ron(text):
    """Keep token spans while skipping strings' contents and nested comments."""
    tokens, closing, stack = [], {}, []
    position = 0
    pairs = {"(": ")", "[": "]", "{": "}"}
    while position < len(text):
        if text[position].isspace():
            position += 1
            continue
        if text.startswith("//", position):
            newline = text.find("\n", position)
            position = len(text) if newline < 0 else newline + 1
            continue
        if text.startswith("/*", position):
            depth = 1
            position += 2
            while depth and position < len(text):
                if text.startswith("/*", position):
                    depth += 1
                    position += 2
                elif text.startswith("*/", position):
                    depth -= 1
                    position += 2
                else:
                    position += 1
            if depth:
                raise ValueError("unclosed comment in document.ron")
            continue
        start = position
        raw = RON_RAW.match(text, position)
        if raw:
            terminator = '"' + raw.group(1)
            end = text.find(terminator, raw.end())
            if end < 0:
                raise ValueError("unclosed raw string in document.ron")
            position = end + len(terminator)
        elif text[position] == '"':
            position += 1
            while position < len(text) and text[position] != '"':
                position += 2 if text[position] == "\\" else 1
            if position >= len(text):
                raise ValueError("unclosed string in document.ron")
            position += 1
        else:
            word = RON_WORD.match(text, position)
            if not word:
                raise ValueError("unreadable token in document.ron")
            position = word.end()
        token = RonToken(text[start:position], slice(start, position))
        index = len(tokens)
        tokens.append(token)
        if token.spelling in pairs:
            stack.append(index)
        elif token.spelling in pairs.values():
            if not stack or pairs[tokens[stack[-1]].spelling] != token.spelling:
                raise ValueError("unbalanced delimiters in document.ron")
            closing[stack.pop()] = index
    if stack:
        raise ValueError("unclosed delimiter in document.ron")
    root = 0
    while root + 2 < len(tokens) and [token.spelling for token in tokens[root:root + 3]] == ["#", "!", "["]:
        root = closing[root + 2] + 1
    if root + 1 < len(tokens) and tokens[root].spelling != "(" and tokens[root + 1].spelling == "(":
        root += 1
    if root >= len(tokens) or tokens[root].spelling != "(" or closing[root] != len(tokens) - 1:
        raise ValueError("document.ron must contain one root record")
    return RonDocument(text, tokens, closing, range(root, len(tokens)))


def ron_container(document, value, opening):
    start = value.start
    if start + 1 < value.stop and document.tokens[start].spelling != opening and document.tokens[start + 1].spelling == opening:
        start += 1
    if start >= value.stop or document.tokens[start].spelling != opening or document.closing[start] != value.stop - 1:
        raise ValueError("expected a %s container in document.ron" % opening)
    return range(start, value.stop)


def ron_items(document, container):
    """Return only immediate items; delimiters inside a value never split it."""
    items = []
    start = position = container.start + 1
    end = container.stop - 1
    while position < end:
        spelling = document.tokens[position].spelling
        if spelling == ",":
            if start < position:
                items.append(range(start, position))
            start = position + 1
        elif position in document.closing:
            position = document.closing[position]
        position += 1
    if start < end:
        items.append(range(start, end))
    return items


def ron_fields(document, record):
    fields = {}
    for item in ron_items(document, record):
        if item.start + 2 >= item.stop or document.tokens[item.start + 1].spelling != ":":
            raise ValueError("expected named fields in document.ron")
        name = document.tokens[item.start].spelling
        if name in fields:
            raise ValueError("duplicate %s field in document.ron" % name)
        fields[name] = RonField(item.start, range(item.start + 2, item.stop))
    return fields


def ron_uint(document, value):
    if len(value) != 1:
        raise ValueError("expected one unsigned id in document.ron")
    spelling = document.tokens[value.start].spelling.replace("_", "")
    if not re.fullmatch(r"(?:[0-9]+|0[xX][0-9a-fA-F]+|0[oO][0-7]+|0[bB][01]+)", spelling):
        raise ValueError("expected an unsigned id, got %s" % spelling)
    number = int(spelling, 0 if spelling.lower().startswith(("0x", "0o", "0b")) else 10)
    if number > U64_MAX:
        raise ValueError("id exceeds u64 in document.ron")
    return number


def dimension_identity(document, value):
    record = ron_container(document, value, "(")
    values = ron_items(document, record)
    if len(values) != 1:
        raise ValueError("dimension identity must contain one id")
    return ron_uint(document, values[0])


def value_region(document, value):
    return slice(document.tokens[value.start].region.start, document.tokens[value.stop - 1].region.stop)


def field_region(document, field):
    return slice(document.tokens[field.key].region.start, document.tokens[field.value.stop - 1].region.stop)


def remove_field(document, field):
    region = field_region(document, field)
    if document.tokens[field.value.stop].spelling == ",":
        region = slice(region.start, document.tokens[field.value.stop].region.stop)
    return TextEdit(region, "")


def apply_text_edits(text, edits):
    pieces, position = [], 0
    for edit in sorted(edits, key=lambda edit: edit.region.start):
        if edit.region.start < position:
            raise ValueError("overlapping edits in document.ron")
        pieces.append(text[position:edit.region.start])
        pieces.append(edit.replacement)
        position = edit.region.stop
    pieces.append(text[position:])
    return "".join(pieces)


def root_named_dimensions(document):
    fields = ron_fields(document, document.root)
    if "named_dims" not in fields:
        return []
    records = ron_container(document, fields["named_dims"].value, "[")
    return [ron_fields(document, ron_container(document, item, "(")) for item in ron_items(document, records)]


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
    document = read_ron(text)
    edits, count = [], 0
    for fields in root_named_dimensions(document):
        if "target" in fields or not any(key in fields for key in ("sketch", "a", "b")):
            continue
        if not all(key in fields for key in ("sketch", "a", "b")):
            raise ValueError("incomplete old named dimension target in document.ron")
        sketch = ron_uint(document, fields["sketch"].value)
        a = ron_uint(document, fields["a"].value)
        b = ron_uint(document, fields["b"].value)
        edits.append(TextEdit(field_region(document, fields["sketch"]), "target: Sketch(sketch: %d, refs: [%d, %d])" % (sketch, a, b)))
        edits.extend([remove_field(document, fields["a"]), remove_field(document, fields["b"])])
        count += 1
    return apply_text_edits(text, edits), count


def candidate_label(document, candidate):
    label = "%d:%s" % (candidate.index, candidate.kind)
    value_key = "deg" if candidate.kind in ("Angle", "AngleLines") else ("len" if candidate.kind == "ArcLength" else "d")
    if value_key in candidate.fields:
        field = candidate.fields[value_key]
        label += " value=" + document.text[value_region(document, field.value)]
    if candidate.kind == "Distance":
        axis = document.text[value_region(document, candidate.fields["axis"].value)] if "axis" in candidate.fields else "0"
        word = "X" if axis == "1" else ("Y" if axis == "2" else "aligned")
        label += " axis %s (axis=%s)" % (word, axis)
    if candidate.kind == "Diameter":
        diam = document.text[value_region(document, candidate.fields["diam"].value)] if "diam" in candidate.fields else "false"
        label += " diameter" if diam == "true" else " radius"
    if candidate.kind == "EdgeDistance":
        for key in ("m1", "m2"):
            if key in candidate.fields:
                label += " %s=%s" % (key, document.text[value_region(document, candidate.fields[key].value)])
    driven = document.text[value_region(document, candidate.fields["driven"].value)] if "driven" in candidate.fields else "false"
    label += " driven=" + driven
    return label


def dimension_failure(target, reason):
    return ValueError("named dimension %s, sketch %d: %s" % (target.name, target.sketch, reason))


def choose_dimension(document, target, candidates):
    """Choose a record from the immutable input; no geometry or document is changed here."""
    print("Choose a constraint for named dimension %s, sketch %d:" % (target.name, target.sketch))
    for number, candidate in enumerate(candidates, 1):
        print("  %d. %s" % (number, candidate_label(document, candidate)))
    while True:
        try:
            answer = input("Choose 1..%d (Enter or q cancels): " % len(candidates)).strip()
        except (EOFError, KeyboardInterrupt):
            raise dimension_failure(target, "conversion cancelled; no files were written") from None
        if not answer or answer.lower() == "q":
            raise dimension_failure(target, "conversion cancelled; no files were written")
        if re.fullmatch(r"[0-9]+", answer):
            try:
                number = int(answer)
            except ValueError:
                number = 0
            if 1 <= number <= len(candidates):
                return candidates[number - 1]
        print("Enter a number from 1 to %d, or cancel with Enter or q." % len(candidates))


def sketch_dimensions(document, target):
    """Read all seven dimensional kinds and validate identities within each sketch."""
    root = ron_fields(document, document.root)
    result, maximum = {}, 0
    if "sketches" not in root:
        return SketchDimensions(result, maximum)
    sketches = ron_container(document, root["sketches"].value, "[")
    for sketch in ron_items(document, sketches):
        fields = ron_fields(document, ron_container(document, sketch, "("))
        if "id" not in fields:
            raise dimension_failure(target, "sketch record has no id")
        sketch_id = ron_uint(document, fields["id"].value)
        if sketch_id in result:
            raise dimension_failure(target, "duplicate sketch id %d" % sketch_id)
        candidates, stored = [], {}
        if "constraints" in fields:
            constraints = ron_container(document, fields["constraints"].value, "[")
            for index, constraint in enumerate(ron_items(document, constraints)):
                kind = document.tokens[constraint.start].spelling
                if kind not in DIMENSION_PARTICIPANTS:
                    continue
                record = ron_container(document, constraint, "(")
                values = ron_fields(document, record)
                missing = [key for key in DIMENSION_PARTICIPANTS[kind] if key not in values]
                if missing:
                    raise dimension_failure(target, "constraint %d:%s in sketch %d has no participants %s" % (index, kind, sketch_id, ", ".join(missing)))
                participants = frozenset(ron_uint(document, values[key].value) for key in DIMENSION_PARTICIPANTS[kind])
                identity = dimension_identity(document, values["dimension"].value) if "dimension" in values else 0
                candidate = DimensionCandidate(kind, index, record, values, participants, identity)
                if identity:
                    if identity in stored:
                        raise dimension_failure(target, "duplicate stored identity %d in sketch %d, candidates [%s, %s]" % (identity, sketch_id, candidate_label(document, stored[identity]), candidate_label(document, candidate)))
                    stored[identity] = candidate
                    maximum = max(maximum, identity)
                candidates.append(candidate)
        result[sketch_id] = candidates
    return SketchDimensions(result, maximum)


def bind_dimension_field(document, candidate, identity):
    if "dimension" in candidate.fields:
        return TextEdit(value_region(document, candidate.fields["dimension"].value), "(%d)" % identity)
    after_open = document.tokens[candidate.record.start].region.stop
    before_field = document.tokens[candidate.record.start + 1].region.start
    gap = document.text[after_open:before_field]
    indentation = gap.rsplit("\n", 1)[-1]
    if "\n" in gap and not indentation.strip():
        prefix = "\n" + indentation
    else:
        prefix = ""
    suffix = "" if gap else " "
    return TextEdit(slice(after_open, after_open), "%sdimension: (%d),%s" % (prefix, identity, suffix))


def sketch_refs_to_dimension_ids(text, *, selection=DimensionSelection.UNIQUE_ONLY):
    """Only a unique participant match can recover the record an old target named."""
    document = read_ron(text)
    targets, reserved = [], 0
    for fields in root_named_dimensions(document):
        if "target" not in fields or document.tokens[fields["target"].value.start].spelling != "Sketch":
            continue
        values = ron_fields(document, ron_container(document, fields["target"].value, "("))
        if "dimension" in values:
            reserved = max(reserved, dimension_identity(document, values["dimension"].value))
        if "refs" not in values:
            continue
        if "name" not in fields:
            raise ValueError("a legacy Sketch refs target has no dimension name")
        name = document.text[value_region(document, fields["name"].value)]
        if "sketch" not in values:
            raise ValueError("named dimension %s has no sketch field" % name)
        sketch = ron_uint(document, values["sketch"].value)
        refs = ron_container(document, values["refs"].value, "[")
        participants = frozenset(ron_uint(document, item) for item in ron_items(document, refs))
        target = LegacyDimensionTarget(name, sketch, values["refs"], participants)
        if "dimension" in values:
            raise dimension_failure(target, "mixed participant and identity target")
        targets.append(target)
    if not targets:
        return DimensionConversion(text, 0)
    root = ron_fields(document, document.root)
    if "next_id" not in root:
        raise dimension_failure(targets[0], "missing next_id allocator")
    sketches = sketch_dimensions(document, targets[0])
    next_id = max(ron_uint(document, root["next_id"].value), sketches.maximum_identity, reserved)
    edits, bound = [], {}
    for target in targets:
        if target.sketch not in sketches.by_sketch:
            raise dimension_failure(target, "missing sketch")
        matches = [candidate for candidate in sketches.by_sketch[target.sketch] if candidate.participants == target.participants]
        if len(matches) > 1 and selection is DimensionSelection.ASK:
            candidate = choose_dimension(document, target, matches)
        elif len(matches) != 1:
            labels = ", ".join(candidate_label(document, candidate) for candidate in matches)
            raise dimension_failure(target, "expected exactly one dimensional constraint, found %d, candidates [%s]" % (len(matches), labels))
        else:
            candidate = matches[0]
        identity = candidate.identity or bound.get(candidate.record.start, 0)
        if not identity:
            if next_id == U64_MAX:
                raise dimension_failure(target, "next_id allocator is exhausted")
            next_id += 1
            identity = next_id
            bound[candidate.record.start] = identity
            edits.append(bind_dimension_field(document, candidate, identity))
        edits.append(TextEdit(field_region(document, target.refs), "dimension: (%d)" % identity))
    if next_id != ron_uint(document, root["next_id"].value):
        edits.append(TextEdit(value_region(document, root["next_id"].value), str(next_id)))
    return DimensionConversion(apply_text_edits(text, edits), len(targets))


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


# A snapshot as it was written before it had a name: an id and two coordinate triples.
SNAPSHOT = re.compile(r"\((\d+), \(([^()]*)\), \(([^()]*)\)\)")

def snapshots_to_records(text):
    """A place snapshot used to be a bare triple; now it is a record with three named fields.

    `edge_refs` and `face_refs` hold the witness taken when a reference was made: the element's id, where it
    stood, and which way it pointed. As `(15, (2.0, 2.0, 11.0), (0.0, 0.0, -1.0))` there was no telling the
    second array from the third without going to find the doc comment on the field - for an edge they are the
    midpoint and the direction, for a face the centre and the normal.

    ONLY INSIDE THOSE TWO MAPS. A triple of `(number, triple, triple)` is a shape that occurs elsewhere in a
    document, and a rule that does not know where it is does more harm than the shape it fixes - the same
    lesson as `lists_to_refs` above.
    """
    n, out, pos = 0, [], 0
    while True:
        hits = [text.find("%s: {" % f, pos) for f in ("edge_refs", "face_refs")]
        hits = [i for i in hits if i >= 0]
        if not hits:
            out.append(text[pos:])
            break
        at = min(hits)
        open_at = text.index("{", at)
        depth, i = 0, open_at
        while i < len(text):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        body, k = text[open_at : i + 1], 0
        body, k = SNAPSHOT.subn(r"(id: \1, at: (\2), dir: (\3))", body)
        n += k
        out.append(text[pos:open_at])
        out.append(body)
        pos = i + 1
    return "".join(out), n


CONVERSIONS = [("Shell", convert_shell), ("Extrude", convert_extrude), ("Revolve", convert_revolve), ("Combine", convert_combine)]


def convert_document(text, *, selection=DimensionSelection.UNIQUE_ONLY):
    counts = {}
    text, n = lists_to_refs(text)
    if n:
        counts["Ref"] = n
    text, n = named_dims_to_targets(text)
    if n:
        counts["NamedDim"] = n
    dimensions = sketch_refs_to_dimension_ids(text, selection=selection)
    text = dimensions.text
    if dimensions.count:
        counts["DimensionId"] = dimensions.count
    text, n = snapshots_to_records(text)
    if n:
        counts["ElemSnapshot"] = n
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


def report(src, dst, counts, renamed):
    """SAY WHAT HAPPENED, not what did not.

    The first version printed "nothing to convert" and stopped there - while a converted copy had just
    appeared beside the original under the new extension. Reported behaviour: the script "did not rename
    anything", because nothing it said mentioned the file it had made. A message that names only what was
    skipped reads as "nothing was done".
    """
    changed = ", ".join("%s %d" % (k, n) for k, n in counts.items() if n) if counts else ""
    if changed and renamed:
        print("converted and renamed: %s -> %s" % (src, dst))
        print("  what changed: %s" % changed)
        print("  the old file stays as %s" % src)
    elif changed:
        print("converted in place: %s" % dst)
        print("  what changed: %s" % changed)
    elif renamed:
        print("renamed: %s -> %s" % (src, dst))
        print("  the document already spoke the current shape - only the extension was old")
        print("  the old file stays as %s" % src)
    else:
        print("nothing to do: %s already speaks the current shape, and the extension is current too" % src)


def main(argv):
    arguments = list(argv[1:])
    selection = DimensionSelection.UNIQUE_ONLY
    if arguments.count("--interactive") == 1:
        arguments.remove("--interactive")
        selection = DimensionSelection.ASK
    if len(arguments) not in (1, 2) or "--interactive" in arguments:
        print(__doc__)
        return 2
    src = arguments[0]
    # A document under the OLD extension is renamed as it is converted; the original is kept beside it under
    # its own name, so nothing of the owner's is lost to a rename.
    default_dst = src[: -len(".qcam")] + ".qcad" if src.endswith(".qcam") else src
    dst = arguments[1] if len(arguments) == 2 else default_dst
    if src.endswith(".ron"):
        original = pathlib.Path(src).read_text()
        text, counts = convert_document(original, selection=selection)
        if text != original or dst != src:
            pathlib.Path(dst).write_text(text)
        report(src, dst, counts, False)
        return 0
    renamed = dst != src and len(arguments) == 1

    # READ AND CONVERT FIRST, DECIDE AFTERWARDS. The earlier order made a backup and rewrote the file
    # before it knew whether anything needed doing, so a document that was already current came away with
    # a `.bak` beside it and a fresh timestamp for nothing.
    with zipfile.ZipFile(src) as bundle:
        entries = [(info, bundle.read(info.filename)) for info in bundle.infolist()]
    counts = {}
    for i, (info, data) in enumerate(entries):
        if info.filename == "document.ron":
            text, counts = convert_document(data.decode("utf-8"), selection=selection)
            entries[i] = (info, text.encode("utf-8"))
    changed = any(counts.values()) if counts else False

    if not changed and src == dst:
        report(src, dst, counts, renamed)
        return 0

    if src == dst:
        # A second conversion must not overwrite the first backup: that one holds the ORIGINAL, and the
        # numbered ones hold each intermediate shape.
        backup, n = src + ".bak", 0
        while pathlib.Path(backup).exists():
            n += 1
            backup = "%s.bak.%d" % (src, n)
        shutil.copyfile(src, backup)
        print("the original is kept as %s" % backup)

    with zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED) as out:
        for info, data in entries:
            out.writestr(info, data)
    report(src, dst, counts, renamed)
    return 0


def dimension_self_tests(tmp):
    """Bind a unique old target, or refuse before touching any file or backup."""
    bad = []
    unique = (
        '(next_id: 20, sketches: [(id: 3, constraints: [Distance(a: 1, b: 2, d: 40.0)])], '
        'named_dims: [(name: "d5", target: Sketch(sketch: 3, refs: [2, 1]))])\n'
    )
    converted, counts = convert_document(unique)
    if converted.count("dimension: (21)") != 2 or "next_id: 21" not in converted or "refs:" in converted:
        bad.append("a unique old target did not become the identity of its constraint")
    if counts.get("DimensionId") != 1:
        bad.append("a converted dimension identity was not reported")
    again, second_counts = convert_document(converted)
    if again != converted or any(second_counts.values()):
        bad.append("a second dimension conversion was not byte-identical and idle")

    pretty = (
        "(\n    next_id: 20,\n    sketches: [\n        (\n            id: 3,\n            constraints: [\n"
        "                Distance(\n                    dimension: (0),\n                    a: 1,\n"
        "                    b: 2,\n                    d: 40.0,\n                ),\n            ],\n"
        "        ),\n    ],\n    named_dims: [\n        (\n            name: \"d5\",\n"
        "            target: Sketch(\n                sketch: 3,\n                refs: [2, 1],\n"
        "            ),\n        ),\n    ],\n)\n"
    )
    converted, _ = convert_document(pretty)
    if converted.count("dimension: (21)") != 2 or "dimension: (0)" in converted:
        bad.append("a pretty document with an explicit unbound identity was not converted")

    for old in (
        unique.replace('target: Sketch(sketch: 3, refs: [2, 1])', "sketch: 3, a: 2, b: 1"),
        pretty.replace("            target: Sketch(\n                sketch: 3,\n                refs: [2, 1],\n            )", "            sketch: 3,\n            a: 2,\n            b: 1"),
    ):
        converted, counts = convert_document(old)
        if converted.count("dimension: (21)") != 2 or counts.get("NamedDim") != 1 or counts.get("DimensionId") != 1:
            bad.append("an oldest a/b target did not traverse both conversion steps")

    for constraint in (
        "Distance(a: 1, b: 2, d: 40.0)",
        "Angle(a: 1, b: 2, c: 3, deg: 40.0)",
        "AngleLines(a: 1, b: 2, c: 3, d: 4, deg: 40.0)",
        "Diameter(c: 1, d: 40.0)",
        "DistancePL(p: 1, a: 2, b: 3, d: 40.0)",
        "EdgeDistance(c1: 1, c2: 2, d: 40.0, m1: 0, m2: 0)",
        "ArcLength(c: 1, a: 2, b: 3, ccw: true, len: 40.0)",
    ):
        participants = re.findall(r"(?:a|b|c|d|p|c1|c2): (\d+)(?=[,)])", constraint)
        body = unique.replace("Distance(a: 1, b: 2, d: 40.0)", constraint).replace("refs: [2, 1]", "refs: [%s]" % ", ".join(reversed(participants)))
        converted, _ = convert_document(body)
        if converted.count("dimension: (21)") != 2:
            bad.append("the participants of %s were not matched" % constraint.split("(", 1)[0])

    bound = unique.replace("Distance(a:", "Distance(dimension: (57), a:")
    converted, _ = convert_document(bound)
    if converted.count("dimension: (57)") != 2 or "next_id: 57" not in converted or "dimension: (21)" in converted:
        bad.append("an existing positive identity was not reused with a safe allocator")

    orphan = unique.replace('named_dims: [', 'named_dims: [(name: "lost", target: Sketch(sketch: 3, dimension: (21))), ')
    converted, _ = convert_document(orphan)
    if converted.count("dimension: (22)") != 2 or converted.count("dimension: (21)") != 1 or "next_id: 22" not in converted:
        bad.append("a fresh identity reused an orphan modern target and retargeted its name")

    feature = unique.replace("Sketch(sketch: 3, refs: [2, 1])", 'Feature(node: 9, key: "height")')
    converted, counts = convert_document(feature)
    if converted != feature or any(counts.values()):
        bad.append("a feature parameter target was rewritten")

    decoys = unique.replace("next_id: 20", 'meta: (comment: "named_dims: [ refs: [999] ] dimension: (123)"), /* outer [ /* nested ] */ ] */ next_id: 20')
    decoys = decoys.replace('name: "d5"', r'name: "bracket ] and escaped \"quote\""')
    converted, _ = convert_document(decoys)
    if converted.count("dimension: (21)") != 2 or 'comment: "named_dims: [ refs: [999] ] dimension: (123)"' not in converted:
        bad.append("quoted delimiters or nested comments changed the target's scope")

    set_match = unique.replace("Distance(a: 1, b: 2, d: 40.0)", "Angle(a: 1, b: 2, c: 2, deg: 40.0)")
    converted, _ = convert_document(set_match)
    if converted.count("dimension: (21)") != 2:
        bad.append("legacy participant matching did not use the old unique-set comparison")

    ambiguity = unique.replace("Distance(a: 1, b: 2, d: 40.0)", "Distance(a: 1, b: 2, d: 40.0, axis: 1), Distance(a: 1, b: 2, d: 30.0, axis: 2)")
    failures = {
        "ambiguous axes": ambiguity,
        "missing sketch": unique.replace("Sketch(sketch: 3", "Sketch(sketch: 4"),
        "no candidate": unique.replace("refs: [2, 1]", "refs: [8, 9]"),
        "duplicate identities": bound.replace("d: 40.0)", "d: 40.0), Diameter(dimension: (57), c: 5, d: 10.0)"),
        "missing allocator": unique.replace("next_id: 20, ", ""),
        "exhausted allocator": unique.replace("next_id: 20", "next_id: 18446744073709551615"),
    }
    for reason, body in failures.items():
        try:
            convert_document(body)
        except ValueError as error:
            if "d5" not in str(error) or "sketch" not in str(error):
                bad.append("%s refusal did not identify the driver and sketch: %s" % (reason, error))
            if reason == "ambiguous axes" and not all(word in str(error) for word in ("Distance", "axis=1", "axis=2", "value=40.0", "value=30.0", "0", "1")):
                bad.append("an ambiguous target refusal did not list both candidate axes, values and indices")
        else:
            bad.append("%s was not refused" % reason)

    def write_bundle(path, document):
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("document.ron", document)
            archive.writestr("sources/6.ipt", bytes(range(256)))
            archive.writestr("brep/7.bin", b"\x00\xffunchanged")

    saved_stdout = sys.stdout
    import io
    sys.stdout = io.StringIO()
    try:
        accepted = tmp / "dimensions.qcad"
        write_bundle(accepted, unique)
        original = accepted.read_bytes()
        main(["convert_qcad.py", str(accepted)])
        backup = tmp / "dimensions.qcad.bak"
        if not backup.exists() or backup.read_bytes() != original:
            bad.append("a dimension conversion did not preserve the exact original backup")
        with zipfile.ZipFile(accepted) as archive:
            if archive.read("sources/6.ipt") != bytes(range(256)) or archive.read("brep/7.bin") != b"\x00\xffunchanged":
                bad.append("a dimension conversion changed another ZIP entry payload")
        first = accepted.read_bytes()
        main(["convert_qcad.py", str(accepted)])
        if accepted.read_bytes() != first or (tmp / "dimensions.qcad.bak.1").exists():
            bad.append("an idle second dimension conversion rewrote the archive or created a backup")

        rejected = tmp / "ambiguous.qcad"
        write_bundle(rejected, ambiguity)
        original = rejected.read_bytes()
        kept_backup = tmp / "ambiguous.qcad.bak"
        kept_backup.write_bytes(b"existing backup")
        try:
            main(["convert_qcad.py", str(rejected)])
        except ValueError:
            pass
        else:
            bad.append("an ambiguous archive was not refused")
        if rejected.read_bytes() != original or kept_backup.read_bytes() != b"existing backup" or (tmp / "ambiguous.qcad.bak.1").exists():
            bad.append("an ambiguous conversion changed the original or a backup before refusing")

        bare = tmp / "ambiguous.ron"
        bare.write_text(ambiguity)
        destination = tmp / "untouched.ron"
        destination.write_text("existing destination\n")
        try:
            main(["convert_qcad.py", str(bare), str(destination)])
        except ValueError:
            pass
        else:
            bad.append("an ambiguous bare document was not refused")
        if bare.read_text() != ambiguity or destination.read_text() != "existing destination\n":
            bad.append("an ambiguous bare conversion changed a file before refusing")
    finally:
        sys.stdout = saved_stdout
    return bad


def interactive_dimension_self_tests(tmp):
    """A console choice binds that record; cancellation leaves the entire conversion untouched."""
    import contextlib
    import io
    from unittest.mock import patch

    bad = []
    x_constraint = "Distance(a: 1, b: 2, d: 40.0, axis: 1)"
    y_constraint = "Distance(a: 1, b: 2, d: 30.0, axis: 2)"
    ambiguous = (
        "(next_id: 20, sketches: [(id: 3, constraints: [%s, %s])], " % (x_constraint, y_constraint)
        + 'named_dims: [(name: "d5", target: Sketch(sketch: 3, refs: [2, 1]))])\n'
    )

    def bundle(path, body):
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("document.ron", body)
            archive.writestr("sources/original.bin", bytes(range(32)))

    def exercise(scenario):
        output, prompts = io.StringIO(), []
        replies = iter(scenario.replies)

        def reply(prompt):
            prompts.append(prompt)
            answer = next(replies, EOFError())
            if isinstance(answer, BaseException):
                raise answer
            return answer

        code, error = None, None
        with contextlib.redirect_stdout(output), patch("builtins.input", reply):
            try:
                code = main(scenario.argv)
            except (Exception, KeyboardInterrupt) as caught:
                error = caught
        return ConsoleResult(code, output.getvalue(), prompts, error)

    for option in ("1", "2"):
        path = tmp / ("choice-%s.qcad" % option)
        bundle(path, ambiguous)
        original = path.read_bytes()
        result = exercise(ConsoleExercise(["convert_qcad.py", "--interactive", str(path)], [option]))
        if result.code != 0 or result.error is not None or len(result.prompts) != 1:
            bad.append("interactive choice %s failed: %s" % (option, result.error))
        with zipfile.ZipFile(path) as archive:
            after = archive.read("document.ron").decode()
            if archive.read("sources/original.bin") != bytes(range(32)):
                bad.append("an interactive conversion changed another ZIP payload")
        chosen = x_constraint if option == "1" else y_constraint
        other = y_constraint if option == "1" else x_constraint
        expected = chosen.replace("Distance(", "Distance(dimension: (21), ")
        if expected not in after or other not in after or after.count("dimension: (21)") != 2 or "next_id: 21" not in after:
            bad.append("choice %s did not bind only the selected constraint" % option)
        if not all(word in result.output for word in ('"d5"', "sketch 3", "1.", "2.", "Distance", "axis X", "axis Y", "value=40.0", "value=30.0")):
            bad.append("the selector did not identify the driver, sketch and both numbered candidates")
        backup = pathlib.Path(str(path) + ".bak")
        if not backup.exists() or backup.read_bytes() != original:
            bad.append("an interactive conversion did not keep the exact original backup")
        first = path.read_bytes()
        idle = exercise(ConsoleExercise(["convert_qcad.py", str(path), "--interactive"], [AssertionError("idle conversion prompted")]))
        if idle.code != 0 or idle.error is not None or idle.prompts or path.read_bytes() != first or pathlib.Path(str(path) + ".bak.1").exists():
            bad.append("an idle interactive conversion prompted, rewrote, or created another backup")

    invalid = tmp / "invalid-choice.qcad"
    bundle(invalid, ambiguous)
    result = exercise(ConsoleExercise(["convert_qcad.py", "--interactive", str(invalid)], ["not a number", "0", "3", "2"]))
    with zipfile.ZipFile(invalid) as archive:
        after = archive.read("document.ron").decode()
    if result.code != 0 or result.error is not None or len(result.prompts) != 4 or y_constraint.replace("Distance(", "Distance(dimension: (21), ") not in after:
        bad.append("invalid input was not safely retried until a valid choice")

    angles = tmp / "angle-lines-choice.qcad"
    first_angle = "AngleLines(a: 1, b: 2, c: 3, d: 4, deg: 45.0)"
    second_angle = "AngleLines(a: 4, b: 3, c: 2, d: 1, deg: 75.0)"
    body = ambiguous.replace(x_constraint, first_angle).replace(y_constraint, second_angle).replace("refs: [2, 1]", "refs: [1, 2, 3, 4]")
    bundle(angles, body)
    result = exercise(ConsoleExercise(["convert_qcad.py", "--interactive", str(angles)], ["2"]))
    with zipfile.ZipFile(angles) as archive:
        after = archive.read("document.ron").decode()
    if result.code != 0 or result.error is not None or first_angle not in after or second_angle.replace("AngleLines(", "AngleLines(dimension: (21), ") not in after:
        bad.append("an angle-line choice did not bind the selected record")
    if "value=45.0" not in result.output or "value=75.0" not in result.output:
        bad.append("angle-line candidates showed a point id instead of the stored angle")

    for cancellation in ("empty", "q", "eof", "interrupt", "after-one"):
        source = tmp / ("cancel-%s.qcad" % cancellation)
        destination = tmp / ("cancel-%s-output.qcad" % cancellation)
        body = ambiguous
        if cancellation == "after-one":
            body = body.replace(y_constraint + "]", y_constraint + ", Distance(a: 3, b: 4, d: 10.0, axis: 1), Distance(a: 3, b: 4, d: 20.0, axis: 2)]")
            body = body.replace('named_dims: [', 'named_dims: [(name: "first", target: Sketch(sketch: 3, refs: [3, 4])), ')
        bundle(source, body)
        original = source.read_bytes()
        destination.write_bytes(b"existing destination")
        backup = pathlib.Path(str(source) + ".bak")
        backup.write_bytes(b"existing backup")
        responses = {"empty": [""], "q": ["q"], "eof": [EOFError()], "interrupt": [KeyboardInterrupt()], "after-one": ["1", "q"]}
        result = exercise(ConsoleExercise(["convert_qcad.py", "--interactive", str(source), str(destination)], responses[cancellation]))
        if not isinstance(result.error, ValueError) or "cancel" not in str(result.error).lower():
            bad.append("%s did not cancel with a clear refusal" % cancellation)
        if len(result.prompts) != (2 if cancellation == "after-one" else 1):
            bad.append("%s did not reach the expected choice before cancelling" % cancellation)
        if source.read_bytes() != original or destination.read_bytes() != b"existing destination" or backup.read_bytes() != b"existing backup" or pathlib.Path(str(source) + ".bak.1").exists():
            bad.append("%s changed a source, destination or backup before all choices were accepted" % cancellation)

    batch = tmp / "must-not-prompt.qcad"
    bundle(batch, ambiguous)
    original = batch.read_bytes()
    result = exercise(ConsoleExercise(["convert_qcad.py", str(batch)], [AssertionError("batch conversion prompted")]))
    if not isinstance(result.error, ValueError) or result.prompts or batch.read_bytes() != original or pathlib.Path(str(batch) + ".bak").exists():
        bad.append("without --interactive an ambiguous conversion prompted or touched a file")

    unique = tmp / "interactive-unique.qcad"
    bundle(unique, ambiguous.replace(x_constraint + ", ", ""))
    result = exercise(ConsoleExercise(["convert_qcad.py", "--interactive", str(unique)], [AssertionError("unique conversion prompted")]))
    with zipfile.ZipFile(unique) as archive:
        after = archive.read("document.ron").decode()
    if result.code != 0 or result.error is not None or result.prompts or after.count("dimension: (21)") != 2:
        bad.append("a unique interactive target was not automatically converted without a prompt")
    return bad


def self_test():
    """WHAT THE SCRIPT SAYS IS PART OF WHAT IT DOES.

    Reported behaviour: run on a document under the old extension, the script answered "nothing to
    convert" and the person concluded it had renamed nothing - while a converted copy had just appeared
    beside the original. The words are the whole interface here: there is no window to look at.

    Run with `python3 convert_qcad.py --self-test`.
    """
    import io
    import tempfile

    # THE SAMPLES LOOK LIKE A REAL DOCUMENT: one field per line, indented. The rules match a field of the
    # block ITSELF, not of anything nested in it, and they replace it together with its newline - a sample
    # written on one line is a sample no rule can touch.
    #
    # The reference is already in its current shape in both, so only the thing under test moves. A bare
    # `faces: []` would be rewritten into a `Ref` as well - correctly, mirroring `Ref::picks`, where an
    # empty pick set is `Query::Id(0)` - and then a sample meant to need nothing would need something.
    def doc(tail):
        return (
            "(\n    timeline: [\n        (\n            kind: Shell(\n"
            "                faces: (query: Id(5), expect: Some, hint: (centroid: (0.0, 0.0, 0.0), normal: (0.0, 0.0, 0.0))),\n"
            "                thickness: 1.0,\n"
            + tail
            + "            ),\n        ),\n    ],\n)\n"
        )

    old_shape = doc("                outward: true,\n                center: false,\n")
    current_shape = doc("                side: Outward,\n")

    def bundle(path, body):
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
            z.writestr("document.ron", body)

    def run(*args):
        out = io.StringIO()
        keep = sys.stdout
        sys.stdout = out
        try:
            main(["convert_qcad.py", *args])
        finally:
            sys.stdout = keep
        return out.getvalue()

    bad = []
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)

        # 1. The old extension over a document that is already current: renamed, and SAID so.
        a = tmp / "a.qcam"
        bundle(a, current_shape)
        said = run(str(a))
        if "renamed:" not in said or not (tmp / "a.qcad").exists():
            bad.append("an old extension over a current document: %r" % said)
        if "nothing to convert" in said:
            bad.append("it still says nothing was done although it renamed the file")

        # 2. A current document under the current extension: nothing to do, and NO litter beside it.
        b = tmp / "b.qcad"
        bundle(b, current_shape)
        said = run(str(b))
        if "nothing to do" not in said:
            bad.append("a document needing nothing: %r" % said)
        if (tmp / "b.qcad.bak").exists():
            bad.append("it left a backup for a file it did not change")

        # 3. A document that really needs converting: says what changed.
        c = tmp / "c.qcad"
        bundle(c, old_shape)
        said = run(str(c))
        if "converted in place" not in said or "Shell" not in said:
            bad.append("a document that needed converting: %r" % said)
        if not (tmp / "c.qcad.bak").exists():
            bad.append("it changed a file in place and kept no original")
        with zipfile.ZipFile(c) as z:
            after = z.read("document.ron").decode()
        if "side: Outward" not in after or "outward:" in after:
            bad.append("the document did not come out in the current shape: %r" % after)

        # 4. Both at once: converted AND renamed.
        d = tmp / "d.qcam"
        bundle(d, old_shape)
        said = run(str(d))
        if "converted and renamed" not in said or not (tmp / "d.qcad").exists():
            bad.append("a document that needed both: %r" % said)

        # 5. THE SNAPSHOTS, and the half that matters is what the rule must NOT touch. A triple of
        # `(number, triple, triple)` occurs elsewhere in a document; a rule that rewrote every one of them
        # would quietly corrupt the rest of the file, and nothing would say so until a part built wrong.
        snaps = (
            "(\n    timeline: [],\n"
            "    edge_refs: {\n        72: [\n"
            "            (15, (2.0, 2.0, 11.0), (0.0, 0.0, -1.0)),\n"
            "            (8, (0.0, 128.0, 10.0), (0.0, 0.0, 1.0)),\n"
            "        ],\n    },\n"
            "    face_refs: {\n        9: [\n            (3, (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)),\n        ],\n    },\n"
            "    elsewhere: [\n        (7, (1.0, 2.0, 3.0), (4.0, 5.0, 6.0)),\n    ],\n)\n"
        )
        e = tmp / "e.qcad"
        bundle(e, snaps)
        said = run(str(e))
        if "ElemSnapshot" not in said:
            bad.append("the snapshots were not converted: %r" % said)
        with zipfile.ZipFile(e) as z:
            after = z.read("document.ron").decode()
        if after.count("(id: ") != 3:
            bad.append("expected three snapshots to be named, got %d" % after.count("(id: "))
        if "(7, (1.0, 2.0, 3.0), (4.0, 5.0, 6.0))" not in after:
            bad.append("the rule reached outside the two maps and rewrote a triple that is not a snapshot")

        bad.extend(dimension_self_tests(tmp))
        bad.extend(interactive_dimension_self_tests(tmp))

    if bad:
        print("SELF-TEST FAILED:")
        for line in bad:
            print("  " + line)
        return 1
    print("self-test: document conversions, dimension identities, console choices and safe refusals passed")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    try:
        sys.exit(main(sys.argv))
    except ValueError as error:
        print("refused: %s" % error, file=sys.stderr)
        sys.exit(2)
