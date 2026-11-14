"""A dot plot with its values written in a column beside the plot."""

from pathlib import Path

import numpy as np

import umber as ub

models = ["Baseline", "Augmented", "Pretrained", "Fine-tuned", "Distilled", "Ensemble"]
mean = np.array([66.5, 69.2, 71.6, 74.3, 70.8, 76.9])
half = np.array([0.8, 0.9, 0.6, 0.7, 0.9, 0.5])

fig = ub.Figure(8, 4.6)
(
    fig.add_axes(y=ub.Scale.category())
    .scatter(
        mean,
        models,
        x_error=(mean - half, mean + half),
        annotate=True,
        format=ub.TickFormat.fixed(1),
        place=ub.Place.COLUMN,
    )
    .value_column("Mean (%)")
    .title("Test accuracy by model")
    .xlabel("Accuracy (%)")
)
fig.save_png(Path(__file__).with_suffix(".png"))
