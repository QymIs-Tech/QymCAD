# The window of the QymCAD disk image, read by `dmgbuild` (run from packaging/macos/bundle.sh) and by
# draw_background.py, which paints the picture behind the icons. ONE SOURCE FOR BOTH: the arrow has to
# start at the program and end at Applications, and the panel has to sit under the notes - so the places
# are written here once, and the picture is drawn from them rather than from a second copy of the numbers.
#
# `dmgbuild` executes this file with `defines` set from its `-D` arguments:
#   stage - the folder holding QymCAD.app and the four notes, made by bundle.sh;
#   art   - this directory, where the background pictures lie.
#
# Coordinates are Finder points, measured from the top left of the window's content; an icon location is
# the centre of the icon. The background is the content plus the status bar's strip, at 1x and 2x.
import os.path
import unicodedata

stage = defines.get("stage", "")  # noqa: F821 - given by dmgbuild
art = defines.get("art", "")  # noqa: F821

# THE WINDOW IS THE DESIGN PLUS TWO STRIPS FINDER DRAWS OVER IT. `window_rect` is the whole window: the
# title bar comes off its top, 32 points on macOS 26, and the status bar off its bottom, 28 points - both
# measured on a screenshot of the opened window. The status bar is Finder's own setting, shared by every
# window, and the image cannot turn it off: the first layout lost the notes' names under it. So the design
# is HEIGHT tall, the picture runs on under the status bar's strip with plain ground, and the window is
# made tall enough for both - with the bar hidden the strip simply shows as a margin.
WIDTH, HEIGHT = 640, 480
TITLE_BAR = 32
STATUS_BAR = 28

APP = (160, 176)
APPLICATIONS = (480, 176)

# One note per language of the program, in a row on the card; each name says its language.
NOTE_Y = 370
NOTES = {
    "README.txt": (110, NOTE_Y),
    "ПРОЧТИ.txt": (250, NOTE_Y),
    "ПРОЧИТАЙ.txt": (390, NOTE_Y),
    "ОҚЫҢЫЗ.txt": (530, NOTE_Y),
}
# What draw_background.py writes under each note: the language's own name for itself.
LANGUAGES = {"README.txt": "English", "ПРОЧТИ.txt": "Русский", "ПРОЧИТАЙ.txt": "Українська", "ОҚЫҢЫЗ.txt": "Қазақша"}

# LZMA-compressed, read-only: 27.3 MB for the bundle that is 40.5 MB as zlib (UDZO), 36.6 MB as LZFSE (ULFO)
# and 36.5 MB as the zip, measured on one build. It mounts on macOS 10.15 and newer; this build needs 12.
format = "ULMO"
filesystem = "HFS+"

files = [os.path.join(stage, "QymCAD.app")] + [os.path.join(stage, name) for name in NOTES]
symlinks = {"Applications": "/Applications"}

# The volume icon: the mounted image shows the program's own icon on the desktop and in the sidebar, not
# a generic drive.
icon = os.path.join(stage, "QymCAD.app", "Contents", "Resources", "qymcad.icns")

# background.png beside background@2x.png: dmgbuild joins the pair into one HiDPI TIFF itself.
background = os.path.join(art, "background.png")

window_rect = ((200, 120), (WIDTH, TITLE_BAR + HEIGHT + STATUS_BAR))
default_view = "icon-view"
show_toolbar = False
show_sidebar = False
show_pathbar = False
show_status_bar = False
show_tab_view = False
show_icon_preview = False
arrange_by = None
icon_size = 88
text_size = 13
label_pos = "bottom"

# KEYED BY THE DECOMPOSED NAME. HFS+ stores a file name decomposed (NFD): "Й" becomes "И" plus a combining
# breve. Finder matches a place to a file by that stored name, so a place keyed by the composed name is
# never found - ПРОЧИТАЙ.txt was put wherever Finder liked, while ПРОЧТИ.txt and ОҚЫҢЫЗ.txt, which hold no
# letter that decomposes, stood in their places. Seen on the screenshot of the opened window.
icon_locations = {
    "QymCAD.app": APP,
    "Applications": APPLICATIONS,
    **{unicodedata.normalize("NFD", name): place for name, place in NOTES.items()},
}
