"""Make the Windows icons from the badge logo.

    python scripts/make_icons.py

Writes assets/icons/opencalc.ico (the application) and assets/icons/ocx.ico (a
worksheet file), each with the usual sizes from 16 to 256 pixels. Needs Pillow. The
results are committed, so this only has to be run when the logo changes.
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
BADGE = ROOT / "assets" / "logos" / "badge_icon_dark_256.png"
OUT = ROOT / "assets" / "icons"
SIZES = [16, 24, 32, 48, 64, 128, 256]
SCALE = 4  # draw large, then shrink, for smooth edges
BIG = 256 * SCALE


def badge(size, grey=None):
    """The badge at `size` pixels; light-grey parts recoloured to `grey` if given."""
    im = Image.open(BADGE).convert("RGBA").resize((size, size), Image.LANCZOS)
    if grey:
        px = im.load()
        for y in range(size):
            for x in range(size):
                r, g, b, a = px[x, y]
                if a and max(r, g, b) - min(r, g, b) < 24:  # grey, not red
                    px[x, y] = (*grey, a)
    return im


def application():
    """The badge on a dark rounded square, readable on any taskbar."""
    im = Image.new("RGBA", (BIG, BIG), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([8 * SCALE, 8 * SCALE, BIG - 8 * SCALE, BIG - 8 * SCALE],
                        radius=52 * SCALE, fill=(30, 32, 36, 255))
    mark = badge(int(BIG * 0.66))
    offset = (BIG - mark.width) // 2
    im.alpha_composite(mark, (offset, offset))
    return im


def worksheet():
    """A sheet of paper with a folded corner, the badge on it."""
    im = Image.new("RGBA", (BIG, BIG), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    left, top, right, bottom = 36 * SCALE, 12 * SCALE, 220 * SCALE, 244 * SCALE
    fold = 56 * SCALE
    outline = (150, 154, 160, 255)
    page = [(left, top), (right - fold, top), (right, top + fold), (right, bottom), (left, bottom)]
    d.polygon(page, fill=(255, 255, 255, 255), outline=outline, width=4 * SCALE)
    corner = [(right - fold, top), (right - fold, top + fold), (right, top + fold)]
    d.polygon(corner, fill=(226, 228, 232, 255), outline=outline, width=4 * SCALE)
    # A red band along the foot, so a stack of worksheets reads as OpenCalc's.
    d.rectangle([left + 2 * SCALE, bottom - 30 * SCALE, right - 2 * SCALE, bottom - 2 * SCALE],
                fill=(198, 40, 40, 255))
    mark = badge(int(BIG * 0.48), grey=(150, 154, 160))
    im.alpha_composite(mark, ((left + right - mark.width) // 2, top + fold + 8 * SCALE))
    return im


def save(name, image):
    OUT.mkdir(parents=True, exist_ok=True)
    frames = [image.resize((s, s), Image.LANCZOS) for s in SIZES]
    frames[-1].save(OUT / f"{name}.ico", sizes=[(s, s) for s in SIZES], append_images=frames[:-1])
    frames[-1].save(OUT / f"{name}.png")


if __name__ == "__main__":
    save("opencalc", application())
    save("ocx", worksheet())
    print("wrote", OUT)
