# Updates

**Help -> Check for updates**.

The program asks the site whether a version newer than yours has come out, and says so in the status line
at the bottom of the window. When there is something to update to, a bright line appears there with the
new version number — press it and a window opens with the details.

## What happens when you press it

A window opens: your version, the available version with its date, and what changed in it. What
stands at the bottom depends on how this copy was installed.

**A copy that can replace itself** — a single AppImage file, or an unpacked archive outside the
system folders — downloads the new version in the background as soon as it learns about it. The
status line then says the update is arriving, and when it waits, that a restart applies it:

> The application was updated in the background. Restart it to apply the changes.
> [Restart now]

Pressing **Restart now** unpacks what is left (an unpacked archive on Windows arrives as an archive
of its own folder) and starts the new version. Nothing is asked on the way: the check already
happened at startup, in the background.

**Any other copy only offers one button — Open the release page.** It opens a browser on the page
holding the files for every system and the full list of changes. You fetch the file with the
browser and install it the same way you did the first time.

## If you installed through a package manager

Update the same way you installed:

| installed from | update with |
|---|---|
| AUR (`qymcad-bin`) | your AUR helper |
| winget | `winget upgrade` |
| Flatpak | the software centre, or `flatpak update` |

There is no need to download the file from the release page in these cases: it will not replace the
package but sit beside it, and you end up with two programs instead of one.

**Inside Flatpak this is absent altogether** — no menu item, no line in the settings. The store updates the
package and tells you about new versions itself.

## How often to ask

**Settings -> General -> Check for updates**: at every start, once a day (the default), once a week, once
a month, or never. The menu item works whichever is chosen, including "never".

## What goes over the network

One request to `https://cad.qymis.tech/latest.json`, carrying three things about your build:

* the version number,
* the system and its word size (`linux-x86_64`, for instance),
* how the program was installed (`appimage`, `msi`, `system` and so on).

All of that is written on the download page and singles nobody out: the request carries no name of yours,
no name of your machine and no number issued to you. Documents, models and file paths go nowhere, ever.

When the copy can replace itself, a second request follows: the release file for your platform, from
the release host. It carries nothing beyond the three things above.

To switch it off, choose "never" in the same setting. There are then no requests at all.

## If it does not work

* **"Could not reach the site"** — there is no network, or the site is not answering. This does not mean
  "no updates": the answer is unknown. Try again later.
* **"No updates found"** in the window — yours is the newest version. An automatic check that finds
  nothing says nothing: the status line stays empty.
* **"This version has been declared unfit. Please update."** — the release you have has a known fault; update
  from the release page.
* **There is no menu item** — you are either inside Flatpak or running a build you compiled yourself. A
  hand-built copy has nothing to compare against: its version number is the same between releases.
