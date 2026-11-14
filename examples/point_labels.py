"""Point labels, error bars in x and y, and a line through the points."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(8)

models = ["Baseline", "Wide", "Deep", "Distilled", "Ensemble"]
latency = np.array([12.0, 21.5, 30.2, 8.4, 44.0])
accuracy = np.array([71.2, 74.8, 77.1, 70.3, 79.6])

folds = ["Fold 1", "Fold 2", "Fold 3", "Fold 4", "Mean"]
nan = float("nan")
score = [66.8, 68.1, 67.2, 66.1, 67.1]
low = [nan, nan, nan, nan, 66.2]
high = [nan, nan, nan, nan, 68.0]

step = np.arange(1, 9)
loss = 2.0 * np.exp(-step / 3.5) + 0.3
spread = 0.05 + 0.25 * np.exp(-step / 4) + rng.uniform(0, 0.02, step.size)

fig = ub.Figure(14, 4.6, cols=3)
(
    fig.add_axes(col=0)
    .scatter(latency, accuracy, annotate=True, format=ub.TickFormat.fixed(1), place=ub.Place.ABOVE)
    .title("Accuracy by model")
    .xlabel("Latency (ms)")
    .ylabel("Accuracy (%)")
    .ylim(66, 84)
)
(
    fig.add_axes(col=1, x=ub.Scale.category())
    .scatter(folds, score, y_error=(low, high), annotate=True, place=ub.Place.RIGHT)
    .title("Cross-validation")
    .ylabel("Accuracy (%)")
)
(
    fig.add_axes(col=2)
    .scatter(
        step,
        loss,
        x_error=(step - 0.3, step + 0.3),
        y_error=(loss - spread, loss + spread),
        connect=True,
    )
    .title("Loss with uncertainty")
    .xlabel("Step")
    .ylabel("Loss")
)
fig.save_png(Path(__file__).with_suffix(".png"))
