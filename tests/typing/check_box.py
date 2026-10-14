"""Box plots require category x, numeric y, nested groups and Distribution values."""

import umber as ub

cats = ub.Figure().add_axes(x=ub.Scale.category())
numeric = ub.Figure().add_axes()
names = ["A", "B"]
groups = [[1.0, 2.0], [3.0]]
d = ub.Distribution("Samples", groups, color=ub.Color.BLUE)
chained: ub.Axes[str, float] = cats.box(names, groups).grouped_box(names, [d])
cats.box(names, ((), (1.0,)), label="Samples", color=ub.Color.RED)
cats.grouped_box(names, (d, d))

numeric.box(names, groups)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
numeric.grouped_box(names, [d])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
cats.box([1.0], groups)  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
cats.box(names, [["bad"]])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
cats.box(names, groups, "Samples")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
cats.box(names, groups, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.grouped_box(names, [ub.Series("A", [1.0])])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
ub.Distribution(1, groups)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Distribution("A", [["bad"]])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
ub.Distribution("A", groups, ub.Color.RED)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]


def wrong_y(ax: ub.Axes[str, str]) -> None:
    ax.box(names, groups)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
    ax.grouped_box(names, [d])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
