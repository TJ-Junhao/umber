"""Static typing expectations for ``Size``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import umber as ub

ax = ub.Figure().add_axes()
t = [0.0, 1.0]

# Positive: sizes in every form, on the methods and the quick functions.
ax.line(t, t, width=ub.Size.AUTO)
ax.line(t, t, width=ub.Size.times(1.5))
ax.line(t, t, width=ub.Size.pt(2.0))
ax.scatter(t, t, size=ub.Size.times(2))
ax.scatter(t, t, size=ub.Size.pt(6))
ub.line(t, t, width=ub.Size.times(2))
ub.scatter(t, t, size=ub.Size.pt(6))
size: ub.Size = ub.Size.AUTO

# Negative: a plain number or string is not a size.
ax.line(t, t, width=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.line(t, t, width=2)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.scatter(t, t, size=6.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.scatter(t, t, size="large")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.line(t, t, width=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.scatter(t, t, size=6.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: the constructors take numbers, and the class has no public constructor.
ub.Size.pt("2")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Size.times(None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Size(2.0)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]

# Negative: size parameters are keyword-only.
ax.line(t, t, "label", ub.Color.AUTO, ub.Dash.SOLID, ub.Size.AUTO)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
