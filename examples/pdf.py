"""Save a vector PDF with embedded fonts and a PNG of the same figure."""

from pathlib import Path

import umber as ub

fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)
(
    fig.add_axes()
    .line([0, 1, 2, 3, 4], [1.0, 2.4, 1.8, 3.6, 4.2], label="Signal")
    .title("A vector document")
    .xlabel("Time (s)")
    .ylabel("Amplitude")
)
fig.save_pdf(Path(__file__).with_suffix(".pdf"))
fig.save_png(Path(__file__).with_suffix(".png"))
