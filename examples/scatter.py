"""Two point clouds with different markers and sizes."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(7)
cloud_a = rng.normal(loc=(0.0, 0.0), scale=0.8, size=(80, 2))
cloud_b = rng.normal(loc=(2.5, 1.5), scale=0.6, size=(60, 2))

fig = ub.Figure()
(
    fig.add_axes()
    .scatter(cloud_a[:, 0], cloud_a[:, 1], label="group A", marker=ub.Marker.CIRCLE)
    .scatter(
        cloud_b[:, 0],
        cloud_b[:, 1],
        label="group B",
        marker=ub.Marker.TRIANGLE,
        size=ub.Size.times(1.5),
    )
    .title("Two clouds")
    .xlabel("first feature")
    .ylabel("second feature")
)
fig.save_png(Path(__file__).with_suffix(".png"))
