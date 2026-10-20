# Design notes

umber is a plotting library for scientific computing and for figures that go into papers. This page explains the choices behind its API and its behaviour, what each choice costs, and how the library is built.

[简体中文](https://github.com/TJ-Junhao/umber/blob/main/DESIGN-ZH.md)

## Principles

**Explicit over implicit, simple over complex.** Every option is a typed value: a `Color`, a `Size`, a `Dash`, an enum member. There are no format strings, no positional style arguments and no silent conversions. New parameters are keyword-only and have defaults, so a call reads the same in a year as it does today.

**Fail fast, and say how to fix it.** An invalid call raises where it is made, with a message, a hint and the offending argument highlighted in the traceback. When something does not fit (a title, an axis label, a legend, a pie label) umber raises instead of clipping it, and a size it names in the error is a size that works. A figure that silently loses a label is worse than an error.

**One shape per call, no hidden state.** `add_axes()` always returns one axes, whatever the grid. There is no current figure, no `plt`, no global configuration. A figure is a value: `copy.copy` and `copy.deepcopy` both give an independent copy.

**Types are the first line of defence.** Category, numeric and time axes are different types, so `bar` on a numeric axes is reported by pyright and mypy before the code runs. Python's type hints are not enforced at run time, but a large share of mistakes surfaces in the editor, and the same check gives a fast, precise signal to coding agents that write plotting code. This is why umber requires Python 3.12: it relies on the type-parameter syntax of PEP 695 and PEP 696.

**Small surfaces.** A method does one thing and keeps its parameter list short. Names stay short where that costs no clarity (`ColorBy`, `Cmap`, `row` and `col`), but a type is never traded for brevity: text sizes are `Size.pt(14)` or `Size.times(1.2)`, not a bare number whose unit you have to remember.

**Low learning cost, room to customise.** The defaults should produce a figure you can publish, and the next step should be one typed option away. Options exist for cases that are not common but are real, for example sharing a colour scale by row or by column as well as across the whole grid.

**Made for papers.** Output is PNG, SVG and PDF. Most figures in papers are embedded as PDF, so PDF output is vector, embeds the fonts it uses as subsets and keeps text selectable. Colours are computed exactly, in the perceptual Oklab space.

## Decisions and what they cost

| Decision | Why | Cost |
| --- | --- | --- |
| Options are enums and typed values, never strings | Editors complete them, type checkers check them, and the docs of each value are one hover away | More imports and longer calls than `"r--"` |
| `add_axes()` returns one axes; the grid is declared on `Figure(rows=, cols=)` | The return type never depends on the grid's shape | No one-call `subplots()` that unpacks a grid |
| Axis scales are fixed when the axes is created | The axes' type follows its scales, and a scale cannot change under existing marks | Changing a scale means creating new axes |
| Data frames are passed as columns, with `by=` for groups; column names are never read | One way to pass data, the same for pandas, polars, NumPy and lists | `ax.line(df["day"], df["temp"], by=df["station"])` is longer than passing names |
| A line never fills; areas are `ax.area()` | A fill is usually not wanted, and a separate method keeps both parameter lists short | Two methods where some libraries have one |
| Text that does not fit raises `StyleError` | Lost text is lost information | Long titles on small figures need a larger figure or a smaller font |
| A font without a bold face raises when bold text is drawn, unless created with `bold_fallback=True` | The same rule as everywhere else: no silent substitution | One more argument for fonts that lack bold |
| Diverging colormaps keep a midpoint close to the background | Values near the centre recede, which is what a centred heatmap is for | Points near the centre would vanish, so colour-mapped points get an outline that strengthens as their colour approaches the background |
| No API stability promise before 1.0.0 | Early decisions can still be corrected instead of being carried forever | Upgrading within 0.x may require changes; release notes list them |

## What umber does not do

- **Interactive or animated output, GUI windows and backends other than PNG, SVG and PDF.** They add a lot of complexity for features few plots use. `fig.show()` opens a PNG in the system viewer, or displays it in a notebook.
- **3-D charts,** for now. The goal for the near term is to cover the 2-D charts of matplotlib.
- **A stable API before 1.0.0.**

## How umber is built

umber is written by AI coding agents, and this section says how.

- **Code.** Every line of code in this repository, in the Rust core, the Python bindings and the tests, and the documentation, was written by AI models: Claude Opus 5.5, Claude Sonnet 5.5 and Claude Haiku 5.5 from Anthropic, GPT-6 Astra and GPT-6.1 Sol from OpenAI, and Gemini 4 Argon from Google. GLM 5.3 from Zhipu AI was also used, for security audits and checks of changes. The author has not written any of the code by hand.
- **Design.** The author decides what umber is and how it behaves: its scope, the rules it follows, the shape of its API and every default that affects how a figure looks. Visual defaults are chosen by comparing rendered figures; some, such as the outline rule for colour-mapped points, were proposed by the author after the options the models offered were rejected.
- **Checks.** Code written by one vendor's model is checked by a model from another vendor. Every change runs the full test suite on Linux, macOS and Windows, static type checks in strict mode and an output gate that renders about 4,500 reference outputs (scenes, PNG, SVG and PDF) and compares them byte for byte; a change to any of them has to be declared as intended. The author watches CI and test coverage, checks each feature by hand once it is done and accepts each release.
- **Limits.** No person has read every line. The checks above are what umber relies on, and they will not catch everything. If you find a bug, please open an issue.

As of version 0.4.0 the repository has about 195,000 lines of Rust and Python, tests and comments included, written in 171 pull requests.

## Contributing

umber is maintained by one person in their spare time. There is no fixed schedule for releases or fixes, and support is best effort. Pull requests are not accepted for now; issues and bug reports are welcome. If umber stops being maintained, the repository will say so. The licence (MIT or Apache-2.0) allows forks, and the tests, the reference outputs and these design notes are public, so a fork can carry on from where it stopped.
