#!/usr/bin/env python3
"""Paint background.png and background@2x.png, the picture behind the icons of the QymCAD disk image.

Run from the root of the repository, with Pillow installed:

    python3 packaging/macos/dmg/draw_background.py --regular /path/to/LiberationSans-Regular.ttf

The two pictures are kept in the repository, so a build never runs this; it is here so the picture can be
made again after the layout in settings.py or the logo changes. Every place - the arrow's ends, the panel
under the notes - is taken from settings.py, the same file Finder's layout is written from.

THE PICTURE IS DRAWN AT FOUR TIMES THE 1x SIZE AND SCALED DOWN. Pillow draws lines and shapes without
smoothing; scaled down by Lanczos from 4x they come out with smooth edges at both 2x and 1x.

THE FONTS ARE LIBERATION SANS, under the SIL Open Font License: the bold face is the one already in
assets/fonts, the regular one is given by `--regular`. A system font would put Apple's lettering into a
file this repository ships.
"""

import argparse
import math
import os
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
SCALE = 4

# The two ends of the logo's gradient, sampled from assets/logo.png.
BLUE = (18, 112, 183)
MAGENTA = (198, 78, 152)
INK = (26, 34, 48)
MUTED = (104, 116, 133)


def layout():
    """The places from settings.py, executed the way dmgbuild executes it."""
    scope = {"defines": {"stage": "", "art": HERE}}
    with open(os.path.join(HERE, "settings.py"), encoding="utf-8") as f:
        exec(compile(f.read(), "settings.py", "exec"), scope, scope)
    return scope


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def px(v):
    return round(v * SCALE)


def ground(width, height):
    """A pale vertical wash with a faint isometric grid - the cube of the logo, drawn as a drafting sheet.
    The grid fades out from the centre so it never runs under a label."""
    w, h = px(width), px(height)
    img = Image.new("RGB", (w, h))
    top, bottom = (252, 253, 255), (236, 241, 248)
    d = ImageDraw.Draw(img)
    for y in range(h):
        d.line([(0, y), (w, y)], fill=mix(top, bottom, y / (h - 1)))

    grid = Image.new("L", (w, h), 0)
    g = ImageDraw.Draw(grid)
    step = px(28)
    slope = math.tan(math.radians(30))
    for x in range(-h * 2, w + h * 2, step):
        g.line([(x, 0), (x + h / slope, h)], fill=255, width=SCALE)
        g.line([(x, 0), (x - h / slope, h)], fill=255, width=SCALE)
    for x in range(0, w, round(step * slope * 2)):
        g.line([(x, 0), (x, h)], fill=255, width=SCALE)

    # Strongest at the top corners, gone towards the middle and the panel.
    fade = Image.new("L", (w, h), 0)
    f = ImageDraw.Draw(fade)
    for i in range(48):
        t = i / 47
        f.ellipse([w * (0.5 - 0.9 * (1 - t)), -h * (1 - t) * 0.6, w * (0.5 + 0.9 * (1 - t)), h * (0.2 + 1.1 * (1 - t))], fill=round(255 * t))
    fade = ImageChops.invert(fade).filter(ImageFilter.GaussianBlur(px(30)))
    alpha = ImageChops.multiply(grid, fade).point(lambda v: round(v * 0.07))
    img.paste(Image.new("RGB", (w, h), BLUE), (0, 0), alpha)
    return img.convert("RGBA")


def soft_shadow(size, shape, blur, opacity, offset):
    """A blurred dark copy of `shape` (a function drawing on a mask), moved by `offset`."""
    mask = Image.new("L", size, 0)
    shape(ImageDraw.Draw(mask))
    mask = mask.filter(ImageFilter.GaussianBlur(blur)).point(lambda v: round(v * opacity))
    layer = Image.new("RGBA", size, (16, 28, 48, 0))
    layer.putalpha(mask)
    moved = Image.new("RGBA", size, (0, 0, 0, 0))
    moved.paste(layer, offset)
    return moved


def header(img, width, bold, regular):
    """The logo beside the name, and one line under them, centred as a group above the icons."""
    logo = Image.open(os.path.join(ROOT, "assets", "logo.png")).convert("RGBA")
    side = px(54)
    logo = logo.resize((side, side), Image.LANCZOS)
    name = ImageFont.truetype(bold, px(28))
    line = ImageFont.truetype(regular, px(12.5))
    d = ImageDraw.Draw(img)
    text_w = d.textlength("QymCAD", font=name)
    gap = px(12)
    x = (px(width) - (side + gap + text_w)) / 2
    top = px(22)
    img.alpha_composite(logo, (round(x), top))
    # The name sits on a baseline just above the logo's middle, the line under it hangs from just below it.
    d.text((x + side + gap, top + side / 2 + px(3)), "QymCAD", font=name, fill=INK, anchor="ls")
    d.text((x + side + gap + px(1), top + side / 2 + px(12)), "Parametric associative CAD", font=line, fill=MUTED, anchor="lt")


def wells(img, centres):
    """A soft light pool under the program and under Applications, so the two read as a pair."""
    size = img.size
    glow = Image.new("L", size, 0)
    g = ImageDraw.Draw(glow)
    for cx, cy in centres:
        r = px(66)
        g.ellipse([px(cx) - r, px(cy) - r, px(cx) + r, px(cy) + r], fill=255)
    glow = glow.filter(ImageFilter.GaussianBlur(px(18))).point(lambda v: round(v * 0.85))
    img.paste(Image.new("RGBA", size, (255, 255, 255, 255)), (0, 0), glow)


def bezier(p0, p1, p2, t):
    u = 1 - t
    return (u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0], u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1])


def arrow(img, start, end, lift):
    """A curved stroke from the program to Applications, running from the logo's blue into its magenta,
    ending in a rounded head that points along the curve. A soft shadow under it lifts it off the page."""
    p0 = (px(start[0]), px(start[1]))
    p2 = (px(end[0]), px(end[1]))
    p1 = ((p0[0] + p2[0]) / 2, (p0[1] + p2[1]) / 2 - px(lift))
    head = px(18)
    width = px(6)
    # Dense enough that neighbouring discs overlap by far more than a pixel at 4x.
    steps = 4000

    # The stroke stops where the head begins, so the round cap does not show through the point.
    pts = [bezier(p0, p1, p2, i / steps) for i in range(steps + 1)]
    tail_end = len(pts) - 1
    while tail_end > 0 and math.dist(pts[tail_end], p2) < head * 0.7:
        tail_end -= 1
    tx, ty = p2[0] - pts[tail_end - 80][0], p2[1] - pts[tail_end - 80][1]
    n = math.hypot(tx, ty)
    tx, ty = tx / n, ty / n
    nx, ny = -ty, tx
    base = (p2[0] - tx * head, p2[1] - ty * head)
    tri = [p2, (base[0] + nx * head * 0.62, base[1] + ny * head * 0.62), (base[0] - nx * head * 0.62, base[1] - ny * head * 0.62)]

    # A stroke of joined line segments shows a notch at every joint; a run of overlapping discs does not.
    def shape(d, colour=255):
        r = width / 2
        for x, y in pts[: tail_end + 1]:
            d.ellipse([x - r, y - r, x + r, y + r], fill=colour)
        d.polygon(tri, fill=colour)
        d.line(tri + [tri[0]], fill=colour, width=px(2.5), joint="curve")

    img.alpha_composite(soft_shadow(img.size, shape, px(5), 0.22, (0, px(4))))

    # The colour runs along the arrow: a mask of the arrow over a left-to-right gradient.
    mask = Image.new("L", img.size, 0)
    shape(ImageDraw.Draw(mask))
    paint = Image.new("RGBA", img.size)
    d = ImageDraw.Draw(paint)
    left, right = p0[0], p2[0]
    for x in range(img.size[0]):
        t = min(max((x - left) / (right - left), 0), 1)
        d.line([(x, 0), (x, img.size[1])], fill=mix(BLUE, MAGENTA, t) + (255,))
    paint.putalpha(mask)
    img.alpha_composite(paint)


def panel(img, width, top, bottom, notes, bold, regular):
    """The card under the notes: what to do before the first launch, and under each note's name the name
    of its language, written in that language - the file name alone says "read me", not whose."""
    box = [px(40), px(top), px(width - 40), px(bottom)]
    radius = px(14)
    img.alpha_composite(soft_shadow(img.size, lambda d: d.rounded_rectangle(box, radius, fill=255), px(8), 0.10, (0, px(3))))
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.rounded_rectangle(box, radius, fill=(255, 255, 255, 190), outline=(214, 222, 233, 255), width=px(1))
    img.alpha_composite(layer)

    d = ImageDraw.Draw(img)
    cx = px(width / 2)
    head = ImageFont.truetype(bold, px(12))
    body = ImageFont.truetype(regular, px(12))
    y = px(top + 20)
    d.text((cx, y), "Before the first launch, open the note in your language", font=head, fill=INK, anchor="mm")
    d.text((cx, y + px(17)), "Прочтите  \u00b7  Прочитайте  \u00b7  Оқыңыз", font=body, fill=MUTED, anchor="mm")

    # Under Finder's label of each note: about 4 points below the icon, then a 16-point line of 13-point text.
    lang = ImageFont.truetype(regular, px(11))
    for (x, y_note), language, half in notes:
        d.text((px(x), px(y_note + half + 4 + 16 + 9)), language, font=lang, fill=MUTED, anchor="mm")

    # A thin rule of the logo's gradient along the card's top edge: the one place the colour repeats.
    rule = Image.new("RGBA", img.size, (0, 0, 0, 0))
    r = ImageDraw.Draw(rule)
    x0, x1 = px(width / 2 - 60), px(width / 2 + 60)
    for x in range(x0, x1):
        t = (x - x0) / (x1 - x0)
        a = round(255 * math.sin(math.pi * t))
        r.line([(x, px(top) - px(1)), (x, px(top) + px(1))], fill=mix(BLUE, MAGENTA, t) + (a,))
    img.alpha_composite(rule)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--regular", required=True, help="LiberationSans-Regular.ttf")
    args = ap.parse_args()
    bold = os.path.join(ROOT, "assets", "fonts", "LiberationSans-Bold.ttf")

    s = layout()
    width, height = s["WIDTH"], s["HEIGHT"]
    # The picture runs on under the status bar's strip; the design stays within `height`.
    picture = height + s["STATUS_BAR"]
    app, apps = s["APP"], s["APPLICATIONS"]
    half = s["icon_size"] / 2
    notes = [(place, s["LANGUAGES"][name], half) for name, place in s["NOTES"].items()]

    img = ground(width, picture)
    header(img, width, bold, args.regular)
    wells(img, [app, apps])
    arrow(img, (app[0] + half + 16, app[1] - 2), (apps[0] - half - 16, apps[1] - 2), 34)
    panel(img, width, s["NOTE_Y"] - 100, height - 10, notes, bold, args.regular)

    img = img.convert("RGB")
    img.resize((width * 2, picture * 2), Image.LANCZOS).save(os.path.join(HERE, "background@2x.png"), optimize=True)
    img.resize((width, picture), Image.LANCZOS).save(os.path.join(HERE, "background.png"), optimize=True)
    print(f"background.png {width}x{picture}, background@2x.png {width * 2}x{picture * 2}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
