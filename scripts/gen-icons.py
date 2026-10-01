#!/usr/bin/env python3
"""Generate the application icon set from a single source drawing.

Keeps the repo free of binary asset churn: edit this file, re-run, and the
PNG sizes Tauri expects are regenerated. Uses only Pillow, no network.
"""

from __future__ import annotations

import pathlib

from PIL import Image, ImageDraw

OUT = pathlib.Path(__file__).resolve().parent.parent / "icons"

# OneNote purple, sampled from Microsoft's product palette.
PURPLE = (108, 62, 190)
PURPLE_DARK = (74, 39, 140)
WHITE = (255, 255, 255)

SIZES = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,
}


def rounded_page(size: int) -> Image.Image:
    """A notebook page with a folded corner and a ruled line motif."""
    ss = 4  # supersample for smooth edges
    canvas = Image.new("RGBA", (size * ss, size * ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(canvas)

    s = size * ss
    m = int(s * 0.12)          # margin
    fold = int(s * 0.22)       # corner fold size
    radius = int(s * 0.08)

    # Body
    d.rounded_rectangle([m, m, s - m, s - m], radius=radius, fill=PURPLE)

    # Folded top-right corner: cut the square out, then add a lighter triangle.
    d.polygon(
        [(s - m - fold, m), (s - m, m), (s - m, m + fold)],
        fill=(0, 0, 0, 0),
    )
    d.polygon(
        [(s - m - fold, m), (s - m, m + fold), (s - m - fold, m + fold)],
        fill=PURPLE_DARK,
    )

    # Spine down the left edge
    d.rounded_rectangle(
        [m, m, m + int(s * 0.055), s - m],
        radius=int(s * 0.02),
        fill=PURPLE_DARK,
    )

    # Ruled lines
    line_x0 = m + int(s * 0.15)
    line_x1 = s - m - int(s * 0.12)
    thickness = max(1, int(s * 0.035))
    for i, frac in enumerate((0.40, 0.52, 0.64, 0.76)):
        y = int(s * frac)
        length = line_x1 - line_x0
        if i == len((0.40, 0.52, 0.64, 0.76)) - 1:
            length = int(length * 0.6)
        d.rounded_rectangle(
            [line_x0, y, line_x0 + length, y + thickness],
            radius=thickness // 2,
            fill=WHITE,
        )

    return canvas.resize((size, size), Image.LANCZOS)


def tray_icon(size: int = 22) -> Image.Image:
    """Monochrome-friendly glyph for the system tray."""
    ss = 4
    canvas = Image.new("RGBA", (size * ss, size * ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(canvas)
    s = size * ss
    stroke = max(2, int(s * 0.09))
    d.rounded_rectangle(
        [stroke, stroke, s - stroke, s - stroke],
        radius=int(s * 0.18),
        outline=WHITE,
        width=stroke,
    )
    inset = stroke + int(s * 0.22)
    for frac in (0.42, 0.58):
        y = int(s * frac)
        d.line(
            [inset, y, s - inset, y],
            fill=WHITE,
            width=stroke,
        )
    return canvas.resize((size, size), Image.LANCZOS)


SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" width="128" height="128">
  <defs>
    <clipPath id="page">
      <path d="M16 8h72l24 24v88a4 4 0 0 1-4 4H16a4 4 0 0 1-4-4V12a4 4 0 0 1 4-4z"/>
    </clipPath>
  </defs>
  <path d="M16 8h72l24 24v88a4 4 0 0 1-4 4H16a4 4 0 0 1-4-4V12a4 4 0 0 1 4-4z"
        fill="#6c3ebe"/>
  <g clip-path="url(#page)">
    <rect x="12" y="8" width="12" height="112" fill="#4a278c"/>
    <path d="M88 8h24v24z" fill="#4a278c"/>
  </g>
  <g fill="#ffffff">
    <rect x="38" y="52" width="62" height="7" rx="3.5"/>
    <rect x="38" y="68" width="62" height="7" rx="3.5"/>
    <rect x="38" y="84" width="62" height="7" rx="3.5"/>
    <rect x="38" y="100" width="38" height="7" rx="3.5"/>
  </g>
</svg>
"""


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, size in SIZES.items():
        path = OUT / name
        rounded_page(size).save(path)
        print(f"wrote {path.relative_to(OUT.parent)} ({size}x{size})")

    tray = OUT / "tray.png"
    tray_icon().save(tray)
    print(f"wrote {tray.relative_to(OUT.parent)} (22x22)")

    svg = OUT / "icon.svg"
    svg.write_text(SVG, encoding="utf-8")
    print(f"wrote {svg.relative_to(OUT.parent)} (scalable)")

    # Multi-resolution .ico for Windows targets of the same source tree.
    rounded_page(256).save(
        OUT / "icon.ico",
        sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("wrote icons/icon.ico")


if __name__ == "__main__":
    main()
