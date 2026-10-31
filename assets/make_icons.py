#!/usr/bin/env python3
"""Writes the desktop app icon (assets/icon.svg) and the installer artwork (apps/desktop/src-tauri/installer/).

    python3 assets/make_icons.py
    pnpm dlx @resvg/resvg-js-cli --fit-width 1024 assets/icon.svg /tmp/icon-1024.png
    cd apps/desktop && pnpm tauri icon /tmp/icon-1024.png && rm -rf src-tauri/icons/android src-tauri/icons/ios

The script itself renders the installer images with resvg (through `pnpm dlx`, so it needs pnpm and network the
first time). There is no PIL: the PNGs resvg writes are decoded here and saved as the 24-bit BMPs NSIS and WiX want.

The mark is the in-app logo (`.dot` in apps/desktop/src/styles.css): a coral circle with a smaller amber circle
behind it, up and to the right. In units of that 13px dot: coral r 6.5 at (0, 0), amber r 4.5 at (5, -5); their
bounding box is 16 units square, centred at (1.5, -1.5).
"""

import math
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
INSTALLER = HERE.parent / "apps" / "desktop" / "src-tauri" / "installer"

# Not WhatsApp's colours: a warm coral and amber on ink or paper.
INK, PAPER, PAPER2, SOFT = "#23244F", "#FFF7EE", "#FFE9D6", "#6B6C8F"
CORAL, AMBER = "#F2604F", "#F7A928"

SANS = "Segoe UI, Inter, DejaVu Sans, sans-serif"


def mark(cx: float, cy: float, unit: float) -> str:
    """The mark with its bounding box centred on (cx, cy); `unit` is pixels per 1/13 of the coral dot."""
    x, y = cx - 1.5 * unit, cy + 1.5 * unit  # coral centre
    return (f'<circle cx="{x + 5 * unit:.2f}" cy="{y - 5 * unit:.2f}" r="{4.5 * unit:.2f}" fill="{AMBER}"/>'
            f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{6.5 * unit:.2f}" fill="{CORAL}"/>')


def squircle(x0: float, size: float, n: float = 5.0, steps: int = 360) -> str:
    """A superellipse |x|^n + |y|^n = 1 filling the square at (x0, x0), the shape of a macOS icon."""
    h = size / 2
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        px = math.copysign(abs(c) ** (2 / n), c) * h
        py = math.copysign(abs(s) ** (2 / n), s) * h
        pts.append(f"{x0 + h + px:.2f},{x0 + h + py:.2f}")
    return "M" + " L".join(pts) + " Z"


def svg(w: int, h: int, body: str) -> str:
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">\n{body}\n</svg>\n'


def icon() -> str:
    # 80px transparent margin, like other macOS icons; the mark is 16 units * 36 = 576px of the 864px tile.
    return svg(1024, 1024, f'<path d="{squircle(80, 864)}" fill="{INK}"/>\n{mark(512, 512, 36)}')


def wordmark(x: float, y: float, size: float, anchor: str = "middle") -> str:
    return (f'<text x="{x}" y="{y}" font-family="{SANS}" font-size="{size}" font-weight="800" '
            f'letter-spacing="{-0.03 * size:.2f}" text-anchor="{anchor}" fill="{INK}">oopz</text>')


def nsis_sidebar() -> str:
    return svg(164, 314, f"""<rect width="164" height="314" fill="{PAPER}"/>
<circle cx="150" cy="300" r="70" fill="{PAPER2}"/>
{mark(82, 112, 4.5)}
{wordmark(82, 196, 38)}
<rect x="62" y="212" width="40" height="6" rx="3" fill="{CORAL}"/>""")


def nsis_header() -> str:
    return svg(150, 57, f'<rect width="150" height="57" fill="{PAPER}"/>\n{mark(122, 28.5, 2.3)}')


def wix_banner() -> str:
    # Windows writes the dialog title over the left of the banner: keep it plain.
    return svg(493, 58, f'<rect width="493" height="58" fill="{PAPER}"/>\n{mark(458, 29, 2.3)}')


def wix_dialog() -> str:
    # Only the left 164px is ours; the welcome and finish text is drawn over the rest.
    return svg(493, 312, f"""<rect width="493" height="312" fill="{PAPER}"/>
<rect width="164" height="312" fill="{PAPER2}"/>
{mark(82, 110, 4.5)}
{wordmark(82, 194, 38)}
<rect x="62" y="210" width="40" height="6" rx="3" fill="{CORAL}"/>""")


# Finder icon centres the DMG layout is designed for (tauri.conf.json bundle.macOS.dmg).
DMG_APP, DMG_APPS = (180, 170), (480, 170)


def dmg_background() -> str:
    y = DMG_APP[1]
    x0, x1 = DMG_APP[0] + 88, DMG_APPS[0] - 88
    return svg(660, 400, f"""<rect width="660" height="400" fill="{PAPER}"/>
<g fill="none" stroke="{CORAL}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" opacity="0.75">
  <path d="M{x0} {y} H{x1}"/>
  <path d="M{x1 - 16} {y - 16} L{x1} {y} L{x1 - 16} {y + 16}"/>
</g>
<text x="330" y="300" font-family="{SANS}" font-size="15" fill="{SOFT}" text-anchor="middle">Drag <tspan fill="{INK}" font-weight="700">oopz</tspan> to Applications</text>""")


def render(svg_text: str, width: int, out: Path, tmp: Path) -> None:
    src = tmp / (out.stem + ".svg")
    src.write_text(svg_text)
    subprocess.run(["pnpm", "dlx", "@resvg/resvg-js-cli", "--fit-width", str(width), "--log-level", "off", str(src), str(out)],
                   check=True, cwd=tmp, stdout=subprocess.DEVNULL)


def read_png(path: Path) -> tuple[int, int, list[bytes]]:
    """Decodes an 8-bit RGBA, non-interlaced PNG (what resvg writes) into rows of RGBA bytes."""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    pos, idat, w = 8, b"", 0
    while pos < len(data):
        (length,), kind = struct.unpack(">I", data[pos:pos + 4]), data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            w, h, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", chunk)
            assert (depth, colour, interlace) == (8, 6, 0), f"{path}: not 8-bit RGBA non-interlaced"
        elif kind == b"IDAT":
            idat += chunk
        pos += 12 + length
    raw, bpp, stride = zlib.decompress(idat), 4, w * 4
    rows, prev = [], bytearray(stride)
    for r in range(h):
        f, line = raw[r * (stride + 1)], bytearray(raw[r * (stride + 1) + 1:(r + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line))
        prev = line
    return w, h, rows


def write_bmp(path: Path, w: int, h: int, rows: list[bytes], bg: str = PAPER) -> None:
    """A 24-bit uncompressed (BI_RGB) bottom-up BMP, alpha composited onto `bg`."""
    back = [int(bg[i:i + 2], 16) for i in (1, 3, 5)]
    pad = (4 - w * 3 % 4) % 4
    pixels = bytearray()
    for row in reversed(rows):
        for x in range(w):
            r, g, b, a = row[x * 4:x * 4 + 4]
            pixels += bytes(round((v * a + k * (255 - a)) / 255) for v, k in ((b, back[2]), (g, back[1]), (r, back[0])))
        pixels += b"\0" * pad
    header = struct.pack("<2sIHHI", b"BM", 54 + len(pixels), 0, 0, 54)
    info = struct.pack("<IiiHHIIiiII", 40, w, h, 1, 24, 0, len(pixels), 2835, 2835, 0, 0)
    path.write_bytes(header + info + pixels)


def main() -> None:
    (HERE / "icon.svg").write_text(icon())
    INSTALLER.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        for name, draw, width in [("nsis-header", nsis_header, 150), ("nsis-sidebar", nsis_sidebar, 164),
                                  ("wix-banner", wix_banner, 493), ("wix-dialog", wix_dialog, 493)]:
            png = tmp / f"{name}.png"
            render(draw(), width, png, tmp)
            write_bmp(INSTALLER / f"{name}.bmp", *read_png(png))
        render(dmg_background(), 660, INSTALLER / "dmg-background.png", tmp)


if __name__ == "__main__":
    main()
