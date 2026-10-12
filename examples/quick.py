"""The quick functions: one call makes a figure with one mark."""

from pathlib import Path

import numpy as np

import umber as ub

out = Path(__file__).parent
rng = np.random.default_rng(5)
x = np.linspace(0, 6, 50)

ub.line(x, np.sin(x), label="sin(x)").title("Quick line").figure.save_png(out / "quick.png")
ub.hist(rng.normal(size=1000), bins=20).title("Quick histogram").figure.save_png(
    out / "quick_hist.png"
)
