#!/usr/bin/env python3
"""Обновить список выпущенных версий прямых зависимостей.

СЕТЬ ЖИВЁТ ЗДЕСЬ, А НЕ В ПРОГОНЕ. Тест, ходящий в сеть, краснеет в поезде и в самолёте, и его глушат
первым же. Поэтому спрашивает crates.io этот скрипт — по команде, — а сторож
(`crates/qymcad/src/dependency_ratchet.rs`) сверяется с тем, что тут записано.

    python3 tools/check_deps.py            # показать разрыв
    python3 tools/check_deps.py --refresh  # сходить на crates.io и переписать tools/deps.toml

Заметку «чем занят в программе» пишет человек: она нужна не машине, а тому, кто будет решать,
поднимать версию или нет. Новая зависимость без заметки роняет сторож — намеренно.
"""

import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEPS = ROOT / "tools" / "deps.toml"


def declared() -> dict[str, str]:
    """Прямые зависимости из всех манифестов: имя -> объявленная версия."""
    out: dict[str, str] = {}
    for manifest in [ROOT / "Cargo.toml", *sorted((ROOT / "crates").glob("*/Cargo.toml"))]:
        text = manifest.read_text(encoding="utf-8")
        # ЛЮБОЙ раздел, чьё имя кончается на `dependencies]`, — включая привязанные к системе
        # (`[target.'cfg(windows)'.build-dependencies]`). Первая редакция знала только три имени и
        # пропустила winresource, которым под Windows вшивается значок в .exe; нашёл это сторож в прогоне.
        for block in re.findall(r"^\[[^\]]*dependencies\]\n(.*?)(?=^\[|\Z)", text, re.S | re.M):
            for line in block.splitlines():
                line = line.split("#")[0].strip()
                m = re.match(r'^([a-zA-Z0-9_-]+)\s*=\s*(.+)$', line)
                if not m:
                    continue
                name, rhs = m.group(1), m.group(2)
                if "path =" in rhs or "workspace = true" in rhs or "workspace.dependencies" in rhs:
                    continue  # свои крейты и наследование — не наша забота
                v = re.search(r'version\s*=\s*"([^"]+)"', rhs) or re.match(r'^"([^"]+)"', rhs)
                if v:
                    out[name] = v.group(1)
    return out


def read_notes() -> dict[str, dict]:
    """Разбор tools/deps.toml без внешних библиотек: он нарочно простой."""
    if not DEPS.exists():
        return {}
    notes, cur = {}, None
    for line in DEPS.read_text(encoding="utf-8").splitlines():
        line = line.split("#")[0].strip() if line.strip().startswith("#") else line
        m = re.match(r'^\[([a-zA-Z0-9_-]+)\]$', line.strip())
        if m:
            cur = m.group(1)
            notes[cur] = {}
            continue
        m = re.match(r'^([a-z_]+)\s*=\s*"(.*)"$', line.strip())
        if m and cur:
            notes[cur][m.group(1)] = m.group(2)
    return notes


def latest(name: str) -> str | None:
    url = f"https://crates.io/api/v1/crates/{name}"
    req = urllib.request.Request(url, headers={"User-Agent": "qymcad-dep-check (github.com/QymIs-Tech/QymCAD)"})
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return json.load(r)["crate"]["max_stable_version"]
    except Exception as e:  # сеть не обязана работать
        print(f"  !! {name}: {e}", file=sys.stderr)
        return None


def gap(declared_v: str, latest_v: str) -> int:
    """На сколько выпусков отстали.

    У версий `0.x` выпуском считается ВТОРОЕ число: 0.29 -> 0.36 это семь выпусков, а не ноль. Так
    договорилось само сообщество Rust, и считать иначе значит не увидеть самого крупного отставания.
    """
    d = [int(x) for x in re.findall(r"\d+", declared_v)[:2]] or [0]
    l = [int(x) for x in re.findall(r"\d+", latest_v)[:2]] or [0]
    d += [0] * (2 - len(d))
    l += [0] * (2 - len(l))
    if d[0] == 0 and l[0] == 0:
        return max(0, l[1] - d[1])
    return max(0, l[0] - d[0])


def main() -> int:
    refresh = "--refresh" in sys.argv
    have, notes = declared(), read_notes()

    if refresh:
        print(">>> спрашиваю crates.io")
        lines = [
            "# ЧТО ВЫПУЩЕНО НА СВЕТЕ — снимок, обновляемый по команде `python3 tools/check_deps.py --refresh`.",
            "#",
            "# Сторож `dependency_ratchet` сверяется с этим файлом, а не с сетью: тест, ходящий в сеть, краснеет",
            "# в поезде и в самолёте, и его глушат первым же.",
            "#",
            "# `what` пишет ЧЕЛОВЕК и обновление его не трогает. Заметка нужна не машине, а тому, кто решает,",
            "# поднимать версию или нет: разрыв в семь выпусков у отрисовщика и у разбора имён шрифтов - это",
            "# разные разговоры.",
            "",
        ]
        for name in sorted(have):
            v = latest(name)
            what = notes.get(name, {}).get("what", "")
            lines.append(f"[{name}]")
            lines.append(f'latest = "{v or notes.get(name, {}).get("latest", "?")}"')
            lines.append(f'what = "{what}"')
            lines.append("")
        DEPS.write_text("\n".join(lines), encoding="utf-8")
        print(f">>> записано в {DEPS.relative_to(ROOT)}")
        notes = read_notes()

    rows, total = [], 0
    for name in sorted(have):
        note = notes.get(name, {})
        lv = note.get("latest", "?")
        g = gap(have[name], lv) if lv != "?" else 0
        total += g
        rows.append((g, name, have[name], lv, note.get("what", "")))
    rows.sort(reverse=True)

    print(f"\n{'разрыв':>7}  {'пакет':<22} {'у нас':<10} {'вышло':<10} чем занят")
    for g, name, d, l, what in rows:
        mark = f"{g}" if g else "."
        print(f"{mark:>7}  {name:<22} {d:<10} {l:<10} {what}")
    print(f"\nвсего выпусков отставания: {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
