"""Pie panels are distinct from Cartesian axes."""

import umber as ub

pie: ub.PieAxes = ub.Figure().add_pie_axes()
pie.pie(["A", "B"], [1, 2], hole=0.5, annotate=True, size=ub.Size.pt(10))
pie.title("Parts").legend(ub.LegendStyle.CHIPS)
quick: ub.PieAxes = ub.pie(["A"], [1])
figure: ub.Figure = quick.figure
pie.line([0, 1], [1, 2])  # type: ignore[attr-defined]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
pie.pie([1], [1])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
pie.pie(["A"], [1], hole=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
