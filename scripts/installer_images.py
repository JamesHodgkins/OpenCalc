#!/usr/bin/env python3
"""Make the Windows installer's two pictures.

The installer (installer/windows/main.wxs) wants 24-bit BMP files, which are what is
committed: installer/windows/banner.bmp (493 x 58, the top of every page after the
first) and installer/windows/dialog.bmp (493 x 312, the Welcome and Finish pages).
This draws them from the badge logo and the bundled Inter fonts, and keeps the PNG
versions beside them in installer/windows/source/ to look at. Run it when the logo
changes and commit all four. Needs Pillow (`pip install pillow`).

Where the installer draws its own text matters: on the banner keep x 0-418 clear for
the title and description (logo in the last ~72 px); on the dialog picture the title
is at about x 180-490, y 22-50 and the body at x 180-478, y 108-138, so that part is
left plain. Check the result by building the installer and opening it (`msiexec /i`
with no `/qn`), then cancel.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "installer" / "windows"
BADGE = ROOT / "assets" / "logos" / "badge_icon_dark_256.png"
FONTS = ROOT / "crates" / "opencalc-app" / "assets" / "fonts"
RED = (198, 40, 40)
GREY = (150, 154, 160)


def badge(size, grey=None):
    im = Image.open(BADGE).convert("RGBA").resize((size, size), Image.LANCZOS)
    if grey:
        px = im.load()
        for y in range(size):
            for x in range(size):
                r, g, b, a = px[x, y]
                if a and max(r, g, b) - min(r, g, b) < 24:
                    px[x, y] = (*grey, a)
    return im


def banner():
    im = Image.new("RGB", (493, 58), (255, 255, 255))
    # A soft wash of grey on the left that fades to white, as behind the title.
    wash = Image.new("RGB", (493, 58))
    px = wash.load()
    for x in range(493):
        level = 255 - int(14 * max(0.0, 1 - x / 260))
        for y in range(58):
            px[x, y] = (level, level, level)
    im.paste(wash)
    mark = badge(44, grey=GREY)
    # Alpha-composite the badge on a dark tile at the right, as the app icon is.
    tile = Image.new("RGBA", (50, 50), (0, 0, 0, 0))
    ImageDraw.Draw(tile).rounded_rectangle([0, 0, 49, 49], radius=11, fill=(30, 32, 36, 255))
    tile.alpha_composite(badge(34), (8, 8))
    im.paste(tile, (493 - 50 - 12, 4), tile)
    del mark
    return im


def dialog():
    im = Image.new("RGB", (493, 312), (244, 245, 246))
    draw = ImageDraw.Draw(im)
    # A large, faint badge behind the white panel the installer writes on.
    ghost = badge(300).convert("RGBA")
    flat = Image.new("RGBA", ghost.size, (232, 233, 235, 255))
    flat.putalpha(ghost.getchannel("A"))
    ghost = flat
    faint = Image.new("RGBA", ghost.size, (0, 0, 0, 0))
    faint.alpha_composite(ghost)
    im.paste(faint, (230, 6), faint)
    # The panel under the installer's text, as in the sister installer.
    panel = Image.new("RGBA", (335, 293), (255, 255, 255, 214))
    im.paste(panel, (148, 10), panel)
    # The wordmark at the bottom left.
    tile = Image.new("RGBA", (40, 40), (0, 0, 0, 0))
    ImageDraw.Draw(tile).rounded_rectangle([0, 0, 39, 39], radius=9, fill=(30, 32, 36, 255))
    tile.alpha_composite(badge(28), (6, 6))
    im.paste(tile, (10, 252), tile)
    regular = ImageFont.truetype(str(FONTS / "Inter-Regular.ttf"), 20)
    bold = ImageFont.truetype(str(FONTS / "Inter-Bold.ttf"), 20)
    draw.text((56, 259), "Open", font=regular, fill=(60, 63, 68))
    width = draw.textlength("Open", font=regular)
    draw.text((56 + width, 259), "Calc", font=bold, fill=RED)
    return im


if __name__ == "__main__":
    (OUT / "source").mkdir(parents=True, exist_ok=True)
    for name, image in {"banner": banner(), "dialog": dialog()}.items():
        image.save(OUT / "source" / f"{name}.png")
        image.save(OUT / f"{name}.bmp", "BMP")
        print(OUT / f"{name}.bmp")
