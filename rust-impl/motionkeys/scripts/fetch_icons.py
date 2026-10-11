"""fetches the interface's icons into src/icons, only the ones listed below

Blender 2.79 icons come from Blender's source (release/datafiles/blender_icons32), one raw .dat file per icon: a 24 byte
header (width, height, then four ints) and bottom-up RGBA rows. they're written out as 32px PNGs, shown at 16px
Silk icons are the SVG redraws of famfamfam Silk (github.com/Simandara/famfamfam-silk-svg)

run from anywhere: python3 scripts/fetch_icons.py
"""

import struct
import urllib.parse
import urllib.request
import zlib
from pathlib import Path

ICONS = Path(__file__).resolve().parent.parent / "src" / "icons"

BLENDER_URL = "https://raw.githubusercontent.com/blender/blender/v2.79b/release/datafiles/blender_icons32/icon32_{}.dat"
SILK_URL = "https://raw.githubusercontent.com/Simandara/famfamfam-silk-svg/main/icons/{}.svg"

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
]


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


if __name__ == "__main__":
    main()
