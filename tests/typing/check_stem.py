"""Static typing expectations for ``Axes.stem``; both checkers reject the marked calls."""

import umber as ub


def generic_stem[X, Y](axes: ub.Axes[X, Y], x: ub.Values[X], y: ub.Values[float]) -> ub.Axes[X, Y]:
    return axes.stem(x, y)


numeric = ub.Figure().add_axes()
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [0.0, 1.0]
names = ["a", "b"]
numeric.stem(t, t)
numeric.stem(t, t, label="Response", color=ub.Color.BLUE, marker=ub.Marker.DIAMOND)
numeric.stem(t, t, size=ub.Size.times(2)).stem(t, t, size=ub.Size.pt(3))
chained: ub.Axes[float, float] = numeric.stem(t, t).line(t, t)
chained_cats: ub.Axes[str, float] = cats.stem(names, t).scatter(names, t)

numeric.stem(names, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.stem(t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.stem(t, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.stem(t, t, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.stem(t, t, marker="circle")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.stem(t, t, size=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.stem(t, t, "Response")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
