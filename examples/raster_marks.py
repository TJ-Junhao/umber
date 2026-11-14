"""Dense marks drawn as one image in vector output, so the PDF stays small.

A million-point scatter and 100 random walks are drawn with ``render=Render.RASTER``. In the PDF
each figure's marks become one image: the 100 lines follow each other in one axes and share it.
"""

from pathlib import Path

import numpy as np

import umber as ub

out = Path(__file__).parent
rng = np.random.default_rng(2026)

# A million points, small and nearly transparent so that the density shows.
n = 1_000_000
x = rng.normal(0, 1, n)
y = 0.6 * x + rng.normal(0, 0.8, n)
fig = ub.Figure(7, 5, theme=ub.Theme.PIGMENT.with_marker_alpha(0.07))
(
    fig.add_axes()
    .scatter(x, y, size=ub.Size.pt(0.6), render=ub.Render.RASTER)
    .title("One million points")
    .xlabel("x")
    .ylabel("y")
)
fig.save_pdf(out / "raster_marks.pdf")
fig.save_png(out / "raster_marks.png")

# 100 random walks of 2,000 steps, which share one image.
steps = np.arange(2_000)
walks = np.cumsum(rng.normal(0, 1, (100, steps.size)), axis=1)
color = ub.Color.BLUE.alpha(0.25)
fig = ub.Figure(7, 5)
ax = fig.add_axes().title("100 random walks").xlabel("Step").ylabel("Position")
for walk in walks:
    ax.line(steps, walk, color=color, width=ub.Size.pt(0.6), render=ub.Render.RASTER)
fig.save_pdf(out / "raster_marks_lines.pdf")
fig.save_png(out / "raster_marks_lines.png")

for name in ("raster_marks.pdf", "raster_marks_lines.pdf"):
    print(f"{name}: {(out / name).stat().st_size / 1024:.0f} KiB")
