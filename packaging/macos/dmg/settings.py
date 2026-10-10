# The window of the QymCAD disk image, read by `dmgbuild` (run from packaging/macos/bundle.sh) and by
# draw_background.py, which paints the picture behind the icons. ONE SOURCE FOR BOTH: the arrow has to
# start at the program and end at Applications, and the panel has to sit under the notes - so the places
# are written here once, and the picture is drawn from them rather than from a second copy of the numbers.
#
# `dmgbuild` executes this file with `defines` set from its `-D` arguments:
#   stage - the folder holding QymCAD.app and the two notes, made by bundle.sh;
#   art   - this directory, where the background pictures lie.
#
# Coordinates are Finder points, measured from the top left of the window's content; an icon location is
# the centre of the icon. The background is exactly the content size, at 1x and 2x.
import os.path

stage = defines.get("stage", "")  # noqa: F821 - given by dmgbuild
art = defines.get("art", "")  # noqa: F821

# The content area. `window_rect` is the whole window, title bar included, so the window is made that much
# taller than the picture: Finder reported the opened window as 640 x 508 for a 640 x 480 picture.
WIDTH, HEIGHT = 640, 480
TITLE_BAR = 28

APP = (160, 190)
APPLICATIONS = (480, 190)
NOTE_EN = (250, 396)
NOTE_RU = (390, 396)

# LZMA-compressed, read-only: 27.3 MB for the bundle that is 40.5 MB as zlib (UDZO), 36.6 MB as LZFSE (ULFO)
# and 36.5 MB as the zip, measured on one build. It mounts on macOS 10.15 and newer; this build needs 12.
format = "ULMO"
filesystem = "HFS+"

files = [
    os.path.join(stage, "QymCAD.app"),
    os.path.join(stage, "README.txt"),
    os.path.join(stage, "ПРОЧТИ.txt"),
]
symlinks = {"Applications": "/Applications"}

# The volume icon: the mounted image shows the program's own icon on the desktop and in the sidebar, not
# a generic drive.
icon = os.path.join(stage, "QymCAD.app", "Contents", "Resources", "qymcad.icns")

# background.png beside background@2x.png: dmgbuild joins the pair into one HiDPI TIFF itself.
background = os.path.join(art, "background.png")

window_rect = ((200, 140), (WIDTH, HEIGHT + TITLE_BAR))
default_view = "icon-view"
show_toolbar = False
show_sidebar = False
show_pathbar = False
show_status_bar = False
show_tab_view = False
show_icon_preview = False
arrange_by = None
icon_size = 96
text_size = 13
label_pos = "bottom"

icon_locations = {
    "QymCAD.app": APP,
    "Applications": APPLICATIONS,
    "README.txt": NOTE_EN,
    "ПРОЧТИ.txt": NOTE_RU,
}
