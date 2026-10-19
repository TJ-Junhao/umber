"""Monthly rainfall and temperature with independent y scales on one plot."""

from pathlib import Path

import umber as ub

months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
rainfall = [52, 48, 55, 61, 72, 84, 91, 88, 70, 64, 58, 54]
temperature = [4, 5, 9, 13, 18, 22, 25, 24, 20, 14, 9, 5]

fig = ub.Figure(8, 5)
left = fig.add_axes(x=ub.Scale.category())
left.bar(months, rainfall, label="Rainfall").ylabel("Rainfall (mm)").title("A year of weather")
right = left.twin_y()
right.line(months, temperature, label="Temperature").ylabel("Temperature (°C)")
fig.save_png(Path(__file__).with_suffix(".png"))
