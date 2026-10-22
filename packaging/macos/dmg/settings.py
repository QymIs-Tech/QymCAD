# The window of the QymCAD disk image, read by `dmgbuild` (run from packaging/macos/bundle.sh) and by
# draw_background.py, which paints the picture behind the icons. ONE SOURCE FOR BOTH: the arrow has to
# start at the program and end at Applications, and the card has to hold the folder of notes beside the
# picture of the first launch - so the places are written here once, and the picture is drawn from them
# rather than from a second copy of the numbers.
#
# THE WINDOW NAMES NO LANGUAGE. Three icons - the program, Applications, the README folder - and a picture
# without words but the system's own "Open Anyway". The notes, one per language, lie inside the folder, so a
# new language changes neither this file nor the picture.
#
# `dmgbuild` executes this file with `defines` set from its `-D` arguments:
#   stage - the folder holding QymCAD.app and the README folder of notes, made by bundle.sh;
#   art   - this directory, where the background pictures lie.
#
# Coordinates are Finder points, measured from the top left of the window's content; an icon location is
# the centre of the icon. The background is the content plus the status bar's strip, at 1x and 2x.
import os.path

stage = defines.get("stage", "")  # noqa: F821 - given by dmgbuild
art = defines.get("art", "")  # noqa: F821

# THE WINDOW IS THE DESIGN PLUS TWO STRIPS FINDER DRAWS OVER IT. `window_rect` is the whole window: the
# title bar comes off its top, 32 points on macOS 26, and the status bar off its bottom, 28 points - both
# measured on a screenshot of the opened window. The status bar is Finder's own setting, shared by every
# window, and the image cannot turn it off: an earlier layout lost the names of the icons under it. So the design
# is HEIGHT tall, the picture runs on under the status bar's strip with plain ground, and the window is
# made tall enough for both - with the bar hidden the strip simply shows as a margin.
WIDTH, HEIGHT = 640, 480
TITLE_BAR = 32
STATUS_BAR = 28

APP = (160, 176)
APPLICATIONS = (480, 176)

# The card under the two: the README folder on its left, and right of a thin rule the way to the first
# launch, drawn as the system shows it - System Settings -> Privacy & Security -> "Open Anyway".
CARD = (40, 268, WIDTH - 40, HEIGHT - 10)
README = (122, 360)
DIVIDER_X = 214
STEP_Y = 354
SETTINGS = (282, STEP_Y)
SECURITY = (392, STEP_Y)
OPEN_ANYWAY = (516, STEP_Y)

# LZMA-compressed, read-only: 27.3 MB for the bundle that is 40.5 MB as zlib (UDZO), 36.6 MB as LZFSE (ULFO)
# and 36.5 MB as the zip, measured on one build. It mounts on macOS 10.15 and newer; this build needs 12.
format = "ULMO"
filesystem = "HFS+"

files = [os.path.join(stage, "QymCAD.app"), os.path.join(stage, "README")]
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

# The names are ASCII, and that matters: HFS+ stores a name decomposed (NFD), Finder matches a place to that
# stored form, and a place keyed by a composed "Й" was never found - a Ukrainian note stood wherever Finder
# liked, seen on the screenshot of an earlier window.
icon_locations = {
    "QymCAD.app": APP,
    "Applications": APPLICATIONS,
    "README": README,
}
