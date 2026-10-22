#!/usr/bin/env python3
"""Paint background.png and background@2x.png, the picture behind the icons of the QymCAD disk image.

Run from the root of the repository, with Pillow installed:

    python3 packaging/macos/dmg/draw_background.py --regular /path/to/LiberationSans-Regular.ttf

The two pictures are kept in the repository, so a build never runs this; it is here so the picture can be
made again after the layout in settings.py or the logo changes. Every place - the arrow's ends, the card
under them, the steps of the first launch - is taken from settings.py, the same file Finder's layout is
written from. The picture holds no language's words: the one word in it is the system's "Open Anyway",
which every note gives in brackets.

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
TAGLINE = "Parametric associative CAD"


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
    # CROPPED TO WHAT IS DRAWN. logo.png keeps about a fifth of its width as a transparent margin on each side;
    # placed whole, that margin counted into the group's width while showing nothing, and the visible group
    # stood 5 points right of the centre, measured on the picture. The row is `side` tall; the logo's ink is
    # 37 points of it, the size it had inside the uncropped 54-point square.
    logo = Image.open(os.path.join(ROOT, "assets", "logo.png")).convert("RGBA")
    logo = logo.crop(logo.getbbox())
    side = px(54)
    ink_h = px(37)
    ink_w = round(logo.width * ink_h / logo.height)
    logo = logo.resize((ink_w, ink_h), Image.LANCZOS)
    name = ImageFont.truetype(bold, px(28))
    line = ImageFont.truetype(regular, px(12.5))
    d = ImageDraw.Draw(img)
    # The group is centred on its widest line: centring it on the name alone put it 22 points right of the
    # window's centre, the tagline being the wider of the two - measured on the screenshot of the window.
    text_w = max(d.textlength("QymCAD", font=name), px(1) + d.textlength(TAGLINE, font=line))
    gap = px(20)
    x = (px(width) - (ink_w + gap + text_w)) / 2
    top = px(22)
    img.alpha_composite(logo, (round(x), top + (side - ink_h) // 2))
    # The name sits on a baseline just above the row's middle, the line under it hangs from just below it.
    text_x = x + ink_w + gap
    d.text((text_x, top + side / 2 + px(3)), "QymCAD", font=name, fill=INK, anchor="ls")
    d.text((text_x + px(1), top + side / 2 + px(12)), TAGLINE, font=line, fill=MUTED, anchor="lt")


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


def card(img, box, divider_x):
    """The card under the program and Applications, holding the README folder on its left and the way to the
    first launch on its right, with a thin rule between them."""
    box = [px(v) for v in box]
    radius = px(14)
    img.alpha_composite(soft_shadow(img.size, lambda d: d.rounded_rectangle(box, radius, fill=255), px(8), 0.10, (0, px(3))))
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.rounded_rectangle(box, radius, fill=(255, 255, 255, 190), outline=(214, 222, 233, 255), width=px(1))
    d.line([(px(divider_x), box[1] + px(26)), (px(divider_x), box[3] - px(26))], fill=(214, 222, 233, 255), width=px(1))
    img.alpha_composite(layer)

    # A thin rule of the logo's gradient along the card's top edge: the one place the colour repeats.
    rule = Image.new("RGBA", img.size, (0, 0, 0, 0))
    r = ImageDraw.Draw(rule)
    mid = (box[0] + box[2]) / 2
    x0, x1 = round(mid - px(60)), round(mid + px(60))
    for x in range(x0, x1):
        t = (x - x0) / (x1 - x0)
        a = round(255 * math.sin(math.pi * t))
        r.line([(x, box[1] - px(1)), (x, box[1] + px(1))], fill=mix(BLUE, MAGENTA, t) + (a,))
    img.alpha_composite(rule)


TILE = 60


def tile(img, centre, top, bottom):
    """A rounded square in the shape of a macOS settings icon, shaded from `top` to `bottom`, with its shadow."""
    cx, cy = px(centre[0]), px(centre[1])
    h = px(TILE / 2)
    box = [cx - h, cy - h, cx + h, cy + h]
    radius = px(TILE * 0.225)
    img.alpha_composite(soft_shadow(img.size, lambda d: d.rounded_rectangle(box, radius, fill=255), px(4), 0.25, (0, px(2))))
    shade = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(shade)
    for y in range(box[1], box[3] + 1):
        d.line([(box[0], y), (box[2], y)], fill=mix(top, bottom, (y - box[1]) / (box[3] - box[1])) + (255,))
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle(box, radius, fill=255)
    shade.putalpha(mask)
    img.alpha_composite(shade)


def gear(img, centre):
    """System Settings: a grey tile holding a gear - a ring of teeth around a hub."""
    tile(img, centre, (196, 200, 207), (128, 134, 144))
    cx, cy = px(centre[0]), px(centre[1])
    d = ImageDraw.Draw(img)
    outer, inner, hub = px(21), px(15.5), px(6.5)
    teeth = 10
    points = []
    for i in range(teeth * 4):
        a = 2 * math.pi * i / (teeth * 4) - math.pi / 2
        r = outer if i % 4 in (0, 1) else inner
        points.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    d.polygon(points, fill=(246, 247, 249))
    d.ellipse([cx - px(11), cy - px(11), cx + px(11), cy + px(11)], fill=(150, 156, 166))
    d.ellipse([cx - hub, cy - hub, cx + hub, cy + hub], fill=(246, 247, 249))


def shield(img, centre):
    """Privacy & Security: a blue tile holding a white shield with a tick."""
    tile(img, centre, (64, 156, 255), (20, 104, 220))
    cx, cy = px(centre[0]), px(centre[1])
    w, top, mid, bottom = px(15), cy - px(19), cy + px(2), cy + px(21)
    outline = [(cx, top), (cx + w, top + px(6))]
    outline += [bezier((cx + w, top + px(6)), (cx + w, mid + px(8)), (cx, bottom), i / 40) for i in range(41)]
    outline += [bezier((cx, bottom), (cx - w, mid + px(8)), (cx - w, top + px(6)), i / 40) for i in range(41)]
    d = ImageDraw.Draw(img)
    d.polygon(outline, fill=(255, 255, 255))
    d.line([(cx - px(7), cy + px(1)), (cx - px(2), cy + px(6)), (cx + px(8), cy - px(5))], fill=(28, 112, 228), width=px(3.5), joint="curve")


def button(img, centre, regular):
    """The system's "Open Anyway" button, as it stands under Security, with a pointer pressing it. The one
    word in the picture: the notes give the same English name in brackets, whatever their language. Returns
    half the button's width in points, where the arrow before it has to stop."""
    font = ImageFont.truetype(regular, px(13))
    d = ImageDraw.Draw(img)
    text = "Open Anyway"
    w = d.textlength(text, font=font) + px(28)
    h = px(26)
    cx, cy = px(centre[0]), px(centre[1])
    box = [cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2]
    img.alpha_composite(soft_shadow(img.size, lambda m: m.rounded_rectangle(box, px(7), fill=255), px(2), 0.22, (0, px(1))))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle(box, px(7), fill=(255, 255, 255), outline=(196, 204, 216), width=px(1))
    d.text((cx, cy), text, font=font, fill=INK, anchor="mm")

    # The pointer: the classic arrow, black with a white rim, its tip on the button's lower right.
    tip = (box[2] - px(14), box[3] - px(5))
    shape = [(0, 0), (0, 17), (4.2, 13.2), (7, 19.5), (9.6, 18.4), (6.9, 12.3), (12.2, 12.3)]
    pts = [(tip[0] + px(x), tip[1] + px(y)) for x, y in shape]
    d.polygon(pts, fill=(255, 255, 255))
    d.line(pts + [pts[0]], fill=(255, 255, 255), width=px(3), joint="curve")
    inner = [(tip[0] + px(x * 0.8 + 1), tip[1] + px(y * 0.8 + 1.9)) for x, y in shape]
    d.polygon(inner, fill=(20, 20, 24))
    return w / 2 / SCALE


def step_arrow(img, start, end):
    """A short muted arrow between two steps of the first launch."""
    d = ImageDraw.Draw(img)
    a, b = (px(start[0]), px(start[1])), (px(end[0]), px(end[1]))
    d.line([a, (b[0] - px(5), b[1])], fill=MUTED, width=px(2.2))
    head = px(6)
    d.polygon([b, (b[0] - head, b[1] - head * 0.7), (b[0] - head, b[1] + head * 0.7)], fill=MUTED)


def step_number(img, centre, n, bold):
    """A small number in a circle above a step: the order reads without words."""
    d = ImageDraw.Draw(img)
    cx, cy = px(centre[0]), px(centre[1])
    r = px(9)
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=mix(BLUE, MAGENTA, (n - 1) / 2))
    d.text((cx, cy + px(0.5)), str(n), font=ImageFont.truetype(bold, px(11)), fill=(255, 255, 255), anchor="mm")


def first_run(img, s, bold, regular):
    """System Settings -> Privacy & Security -> "Open Anyway", numbered, right of the README folder."""
    settings, security, anyway = s["SETTINGS"], s["SECURITY"], s["OPEN_ANYWAY"]
    half = TILE / 2
    gear(img, settings)
    shield(img, security)
    anyway_half = button(img, anyway, regular)
    gap = 12
    step_arrow(img, (settings[0] + half + gap, settings[1]), (security[0] - half - gap, security[1]))
    step_arrow(img, (security[0] + half + gap, security[1]), (anyway[0] - anyway_half - gap, anyway[1]))
    below = s["STEP_Y"] + half + 22
    for n, (x, _) in enumerate([settings, security, anyway], start=1):
        step_number(img, (x, below), n, bold)


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

    img = ground(width, picture)
    header(img, width, bold, args.regular)
    wells(img, [app, apps])
    arrow(img, (app[0] + half + 16, app[1] - 2), (apps[0] - half - 16, apps[1] - 2), 34)
    card(img, s["CARD"], s["DIVIDER_X"])
    first_run(img, s, bold, args.regular)

    img = img.convert("RGB")
    img.resize((width * 2, picture * 2), Image.LANCZOS).save(os.path.join(HERE, "background@2x.png"), optimize=True)
    img.resize((width, picture), Image.LANCZOS).save(os.path.join(HERE, "background.png"), optimize=True)
    print(f"background.png {width}x{picture}, background@2x.png {width * 2}x{picture * 2}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
