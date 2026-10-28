"""fetches the interface's icons into src/icons, only the ones listed below

Blender 2.79 icons come from Blender's source (release/datafiles/blender_icons32), one raw .dat file per icon: a 24 byte
header (width, height, then four ints) and bottom-up RGBA rows. they're written out as 32px PNGs, shown at 16px
Silk icons are the SVG redraws of famfamfam Silk (github.com/Simandara/famfamfam-silk-svg). a few are composed here from a
Silk icon and one of Silk's badges (its decorators), the icon cut away around the badge like Silk's own badged icons

run from anywhere: python3 scripts/fetch_icons.py
"""

import re
import struct
import urllib.parse
import urllib.request
import zlib
from pathlib import Path

ICONS = Path(__file__).resolve().parent.parent / "src" / "icons"

BLENDER_URL = "https://raw.githubusercontent.com/blender/blender/v2.79b/release/datafiles/blender_icons32/icon32_{}.dat"
SILK_URL = "https://raw.githubusercontent.com/Simandara/famfamfam-silk-svg/main/icons/{}.svg"
DECORATOR_URL = "https://raw.githubusercontent.com/Simandara/famfamfam-silk-svg/main/decorators/{}.svg"

# Blender icon names (UI_icons.h, lowercase)
BLENDER = [
    "scene_data",
    "group",
    "object_data",
    "manipul",
    "man_trans",
    "man_rot",
    "man_scale",
    "shapekey_data",
    "key_hlt",
    "action",
    "space2",
]

# Silk icon names as the repo spells them, saved lowercase with underscores
SILK = [
    "Chart curve",
    "Layout sidebar",
    "Disk",
    "Folder page",
    "Add",
    "Arrow Redo",
    "Shape group",
    "Shape ungroup",
]

# Silk icons mirrored left to right: the file and the Silk icon. undo is redo's mirror, Silk's own Arrow Undo has its head at
# the other end so the two didn't look like a pair
MIRRORED = [
    ("arrow_redo_mirrored", "Arrow Redo"),
]

# Silk icons recoloured: the file, the Silk icon, and its colours swapped for others. Control play turns green, in Add's
# greens with a white triangle. its shadow's plus-darker blend is webkit's own, chromium (windows' webview) only knows multiply
RECOLOURED = [
    ("control_play_green", "Control play", {"#DEDEDE": "#65AB55", "#F0F0F0": "#9DE591", "#9F9F9F": "#277224", "#DADADA": "#5CC33D", "#808080": "white", "plus-darker": "multiply"}),
]

# composed icons: the file, the Silk icon and its badge (a decorator, by the repo's name), the badge's top left and scale in
# the icon's 16px. an icon with corner handles loses its bottom right one, the badge sits there
COMPOSED = [
    ("shape_group_go", "Shape group", "arrow go", 6, 7, 1),
    ("shape_group_come", "Shape group", "arrow come", 6, 7, 1),
    ("shape_group_house", "Shape group", "house", 5, 7, 1),
    ("shape_group_magnifier", "Shape group", "magnifier", 7, 7, 1),
    ("shape_handles_magnifier", "Shape handles", "magnifier", 7, 7, 1),
]

# where the corner handles path starts, and where its bottom right handle does
CORNER_HANDLES = {
    "Shape group": ("M2.5 1.5", "M15.5 14.5"),
    "Shape handles": ("M3.5 2.5", "M14.5 13.5"),
}


def fetch(url):
    with urllib.request.urlopen(urllib.parse.quote(url, safe=":/")) as response:
        return response.read()


def png(width, height, rgba):
    # each row starts with filter type 0 (none)
    rows = b"".join(b"\x00" + rgba[y * width * 4 : (y + 1) * width * 4] for y in range(height))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b"")


def blender_icon(name):
    data = fetch(BLENDER_URL.format(name))
    width, height = struct.unpack("<II", data[:8])
    pixels = data[24 : 24 + width * height * 4]
    # rows are stored bottom up
    rows = [pixels[y * width * 4 : (y + 1) * width * 4] for y in range(height)]
    return png(width, height, b"".join(reversed(rows)))


# an svg's size and what's inside it, its ids prefixed so two can share a file
def svg_parts(svg, prefix):
    width, height = map(float, re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', svg).groups())
    inner = re.search(r"<svg[^>]*>(.*)</svg>", svg, re.S).group(1)
    for id in re.findall(r'id="([^"]+)"', inner):
        inner = inner.replace(f'id="{id}"', f'id="{prefix}{id}"').replace(f"url(#{id})", f"url(#{prefix}{id})")
    return width, height, inner


# the icon cut away around the badge by a mask of the badge's shapes, stroked a pixel wider, then the badge on top
def compose(icon, badge, x, y, scale):
    _, _, icon_inner = svg_parts(icon, "i_")
    _, _, badge_inner = svg_parts(badge, "b_")
    outline = re.sub(r"<defs>.*?</defs>", "", badge_inner, flags=re.S)
    outline = re.sub(r'\s(fill|stroke|filter|clip-path|stroke-linejoin|stroke-width|opacity|fill-rule|clip-rule)="[^"]*"', "", outline)
    place = f"translate({x} {y}) scale({scale})"
    mask = f'<mask id="cut" maskUnits="userSpaceOnUse" x="0" y="0" width="16" height="16"><rect width="16" height="16" fill="white"/><g transform="{place}" fill="black" stroke="black" stroke-width="{2 / scale}" stroke-linejoin="round">{outline}</g></mask>'
    return f'<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><defs>{mask}</defs><g mask="url(#cut)">{icon_inner}</g><g transform="{place}">{badge_inner}</g></svg>\n'


# everything in the 16px svg flipped left to right
def mirror(svg):
    return re.sub(r"(<svg[^>]*>)(.*)(</svg>)", r'\1<g transform="translate(16 0) scale(-1 1)">\2</g>\3', svg, flags=re.S)


# the icon without its bottom right handle, the handles are one path of four circles
def without_corner_handle(svg, start, corner):
    handles = re.search(rf'<path d="({re.escape(start)}[^"]+)"', svg).group(1)
    circles = re.findall(r"M[^M]+", handles)
    return svg.replace(handles, "".join(c for c in circles if not c.startswith(corner)))


def main():
    (ICONS / "blender").mkdir(parents=True, exist_ok=True)
    (ICONS / "silk").mkdir(parents=True, exist_ok=True)

    for name in BLENDER:
        (ICONS / "blender" / f"{name}.png").write_bytes(blender_icon(name))
        print(f"blender/{name}.png")

    for name in SILK:
        file = name.lower().replace(" ", "_") + ".svg"
        (ICONS / "silk" / file).write_bytes(fetch(SILK_URL.format(name)))
        print(f"silk/{file}")

    for file, name in MIRRORED:
        (ICONS / "silk" / f"{file}.svg").write_text(mirror(fetch(SILK_URL.format(name)).decode()))
        print(f"silk/{file}.svg")

    for file, name, colours in RECOLOURED:
        svg = fetch(SILK_URL.format(name)).decode()
        for old, new in colours.items():
            svg = svg.replace(f'"{old}"', f'"{new}"')
        (ICONS / "silk" / f"{file}.svg").write_text(svg)
        print(f"silk/{file}.svg")

    for file, name, badge, x, y, scale in COMPOSED:
        icon = fetch(SILK_URL.format(name)).decode()
        if name in CORNER_HANDLES:
            icon = without_corner_handle(icon, *CORNER_HANDLES[name])
        (ICONS / "silk" / f"{file}.svg").write_text(compose(icon, fetch(DECORATOR_URL.format(badge)).decode(), x, y, scale))
        print(f"silk/{file}.svg")


if __name__ == "__main__":
    main()
