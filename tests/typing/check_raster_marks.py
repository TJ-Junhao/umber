"""Static typing expectations for marks drawn as one image and the raster resolution."""

from pathlib import Path
from typing import assert_type

import umber as ub

ax = ub.Figure().add_axes()
x, y = [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]
assert_type(ax.scatter(x, y, render=ub.Render.RASTER), ub.Axes[float, float])
assert_type(ax.line(x, y, render=ub.Render.VECTOR), ub.Axes[float, float])
assert_type(ax.tripcolor(x, y, [1.0, 2.0, 3.0], render=ub.Render.RASTER), ub.Axes[float, float])
assert_type(ub.scatter(x, y, render=ub.Render.RASTER), ub.Axes[float, float])
assert_type(ub.line(x, y, render=ub.Render.RASTER), ub.Axes[float, float])
fig = ax.figure
assert_type(fig.to_svg(raster_dpi=150), str)
assert_type(fig.to_pdf(raster_dpi=600), bytes)
fig.save_svg(Path("chart.svg"), text=ub.SvgText.EDITABLE, raster_dpi=300)
fig.save_pdf("chart.pdf", raster_dpi=300)
assert_type(ub.to_pdf([fig, ax], raster_dpi=200), bytes)
ub.save_pdf("pages.pdf", [fig], raster_dpi=200)
ax.scatter(x, y, render=ub.AUTO)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.line(x, y, render="raster")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.to_pdf(raster_dpi=300.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.to_svg(raster_dpi="300")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.to_pdf(300)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
