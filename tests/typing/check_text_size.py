"""Text sizes accept Size values and preserve the axes' coordinate types."""

from typing import assert_type

import umber as ub

fig = ub.Figure()
ax = fig.add_axes()
categories = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
assert_type(fig.title("Title", size=ub.Size.pt(18)), ub.Figure)
assert_type(fig.subtitle("Subtitle", size=ub.Size.times(1.2)), ub.Figure)
assert_type(fig.colorbar("Intensity", size=ub.Size.pt(12)), ub.Figure)
assert_type(ax.title("Title", size=ub.Size.pt(18)), ub.Axes[float, float])
assert_type(ax.subtitle("Subtitle", size=ub.Size.AUTO), ub.Axes[float, float])
assert_type(ax.xlabel("Distance", size=ub.Size.times(1.2)), ub.Axes[float, float])
assert_type(ax.ylabel("Height", size=ub.Size.pt(10)), ub.Axes[float, float])
assert_type(ax.xtick_labels(size=ub.Size.pt(9)), ub.Axes[float, float])
assert_type(ax.ytick_labels(), ub.Axes[float, float])
assert_type(categories.xtick_labels(size=ub.Size.pt(9)), ub.Axes[str, str])
assert_type(categories.ytick_labels(size=ub.Size.pt(9)), ub.Axes[str, str])
assert_type(ax.legend(ub.LegendStyle.HEADER, size=ub.Size.pt(9)), ub.Axes[float, float])
assert_type(ax.colorbar("Intensity", size=ub.Size.pt(9)), ub.Axes[float, float])
ax.title("Title", size=12)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.xlabel("Distance", size=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.ylabel("Height", size=ub.AUTO)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.subtitle("Subtitle", size="large")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.xtick_labels(size=12)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.ytick_labels(size=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.legend(ub.LegendStyle.HEADER, size=12)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.colorbar(size=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.title("Title", size=12)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.subtitle("Subtitle", size=ub.AUTO)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.colorbar(size=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.title("Title", ub.Size.pt(9))  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.xtick_labels(ub.Size.pt(9))  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
