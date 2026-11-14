"""A figure caption: a source note under two panels."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(11)
epochs = np.arange(1, 61)
train = 2.1 * np.exp(-epochs / 14) + 0.25 + rng.normal(0, 0.015, epochs.size)
valid = 2.2 * np.exp(-epochs / 16) + 0.38 + rng.normal(0, 0.03, epochs.size)
accuracy = 0.93 - 0.55 * np.exp(-epochs / 12) + rng.normal(0, 0.008, epochs.size)

fig = ub.Figure(11, 4.6, cols=2)
fig.title("Training run")
(
    fig.add_axes(col=0)
    .line(epochs, train, label="Train")
    .line(epochs, valid, label="Validation")
    .title("Loss")
    .xlabel("Epoch")
    .ylabel("Cross-entropy")
)
(
    fig.add_axes(col=1)
    .line(epochs, accuracy, label="Validation")
    .title("Accuracy")
    .xlabel("Epoch")
    .ylabel("Share correct")
    .ytick_format(ub.TickFormat.percent(0))
)
fig.caption(
    "Source: run 42, batch size 128, learning rate 3e-4.\nValidation split: 10% of the images."
)
fig.save_png(Path(__file__).with_suffix(".png"))
