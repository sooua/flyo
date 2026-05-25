"""Render the Flyo "stack" logo at multiple sizes and bundle into flyo.ico.

The ICO format is a container of bitmaps at different resolutions; Windows
picks the closest match for the display context (taskbar, Explorer detail
view, Start menu, etc.). We embed 7 sizes from 16px (favicon territory) to
256px (Vista+ Jumbo icons).

The artwork mirrors the SVG in `web/src/logo.tsx`: three stacked rounded
rectangles in a single warm-near-black colour with opacity stops 0.35 / 0.6 /
1.0. Pillow draws each rect on a per-size RGBA canvas, then `save(..., ICO,
sizes=...)` writes the multi-resolution container.

Run from the workspace root, or anywhere — paths are anchored to this file.
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

# Coordinates from the 24x24 viewBox source SVG (logo.tsx).
# Each layer: (x, y, width, height, corner_radius, opacity)
LAYERS = (
    (4, 3, 16, 4, 1.5, 0.35),   # back  (most translucent)
    (2, 8, 20, 5, 2.0, 0.6),    # middle
    (0, 14, 24, 9, 2.5, 1.0),   # front (fully opaque, biggest)
)
COLOR = (36, 36, 36)  # warm near-black (#242424)
SIZES = (16, 20, 24, 32, 48, 64, 128, 256)
OUT = Path(__file__).resolve().parent.parent / "assets" / "flyo.ico"


def render_one(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    scale = size / 24

    for x, y, w, h, r, opacity in LAYERS:
        draw.rounded_rectangle(
            xy=(x * scale, y * scale, (x + w) * scale, (y + h) * scale),
            radius=r * scale,
            fill=(*COLOR, round(255 * opacity)),
        )
    return img


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)

    images = [render_one(s) for s in SIZES]
    # The first arg's `sizes` tuple tells PIL to keep all those resolutions.
    # `append_images` provides the high-quality pre-rendered versions so PIL
    # doesn't downsample from the largest (which loses our crisp curves).
    images[-1].save(
        OUT,
        format="ICO",
        sizes=[(s, s) for s in SIZES],
        append_images=images[:-1],
    )
    size_kb = OUT.stat().st_size / 1024
    print(f"wrote {OUT}  ({size_kb:.1f} KB, {len(SIZES)} sizes)")


if __name__ == "__main__":
    main()
