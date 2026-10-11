"""Static typing expectations for style arguments and file paths.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

from pathlib import Path

import umber as ub

ax = ub.Figure().add_axes()
t = [0.0, 1.0]

# Positive: enum and colour arguments.
ax.line(t, t, label="a", color=ub.Color.RED, dash=ub.Dash.DASHED)
ax.line(t, t, color=ub.Color.hex("#3366B8"))
ax.line(t, t, color=ub.Color.rgb(10, 20, 30).alpha(0.5))
ax.scatter(t, t, marker=ub.Marker.SQUARE)
ax.legend(ub.LegendStyle.DIRECT)
fig = ub.Figure(6, 4, theme=ub.Theme.MIDNIGHT)
fig.save_png("out.png")
fig.save_png(Path("out.png"), dpi=300)
fig.save_svg("out.svg", text=ub.SvgText.EDITABLE)
fig.save_svg(Path("out.svg"))

# Negative: strings in place of enums, colours and themes.
ax.line(t, t, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.line(t, t, dash="--")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.scatter(t, t, marker="o")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.legend(None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Figure(theme="midnight")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Figure(theme=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: ``save_png`` takes ``str`` or ``os.PathLike[str]``, not ``bytes``.
fig.save_png(b"x.png")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.save_svg(b"x.svg")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: style parameters are keyword-only.
ax.line(t, t, "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ub.Figure(6, 4, ub.Theme.PIGMENT)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
