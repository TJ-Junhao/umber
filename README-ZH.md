# umber

umber 是一个 Python 绘图库，渲染核心用 Rust 编写。它可以绘制折线图、散点图、面积图、柱状图和直方图，排版考究，内置三套主题，输出 PNG 或 SVG。

[English](README.md)

![Graphite 主题下两个站点的每日气温折线图](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/lines-graphite.png)

- **大数据量也快。** 所有逐点计算都在 Rust 中完成：100 万个点的折线渲染成 PNG 约 55 ms，10 万个点的散点图约 50 ms（见[性能](#性能)）。
- **显式。** 每个选项都是有类型的值（`Color`、`Size`、`Dash`、`TickFormat` 等）。没有格式字符串，没有位置式的样式参数，也没有隐式转换。
- **尽早报错，并指出问题所在。** 非法输入在引发它的那次调用处就报错，附带说明、提示，并在 traceback 中高亮出错的参数。
- **静态类型。** 类别轴和数值轴是不同的类型：在数值轴上调用 `bar`，pyright 和 mypy 在运行前就会报错。
- **自包含。** 字体随包内置，同一张图在任何机器上看起来都一样。

## 安装

```sh
pip install umber
```

需要 Python 3.12 或更新版本，以及 NumPy 2。我们为 Linux（x86_64 和 aarch64）、macOS（Apple silicon 和 Intel）和 Windows（x64）发布预编译 wheel。其他平台上 `pip` 会从源码构建，需要 Rust 1.88 或更新版本。

## 快速开始

```python
import numpy as np
import umber as ub

t = np.linspace(0, 10, 1_000)
ub.line(t, np.sin(t), label="sin").title("A first chart").figure.save_png("chart.png")
```

快捷函数 `ub.line`、`ub.scatter`、`ub.bar` 和 `ub.hist` 会创建一张只含一个 axes 的图并返回这个 axes，其余部分可以链式调用继续添加。

## 构建一张图

```python
import numpy as np
import umber as ub

rng = np.random.default_rng(7)
days = np.arange(60)
north = 20 + np.cumsum(rng.normal(0, 1, 60))
south = 18 + np.cumsum(rng.normal(0, 1, 60))

fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)  # 宽和高，单位为英寸
ax = fig.add_axes()
ax.title("Daily temperature").subtitle("Two stations, last 60 days")
ax.xlabel("Day").ylabel("°C")
ax.line(days, north, label="North")
ax.line(days, south, label="South", dash=ub.Dash.DASHED)

fig.save_png("temperature.png", dpi=200)
fig.save_svg("temperature.svg")
```

这段代码画出的就是本页顶部那张图。数据可以是 list、tuple、NumPy 数组，或 pandas、polars 的 Series。缺失值（`NaN`，或 pandas、polars 可空列中的 null）会在图中留出空隙。

| 图表 | 方法 |
| --- | --- |
| 折线图 | `ax.line(x, y)` |
| 散点图 | `ax.scatter(x, y, marker=ub.Marker.TRIANGLE, size=ub.Size.times(1.5))` |
| 面积图 | `ax.area(x, y)` |
| 柱状图、分组柱状图、堆叠柱状图 | `ax.bar(categories, heights)`、`ax.grouped_bar(categories, series)`、`ax.stacked_bar(categories, series)` |
| 直方图 | `ax.hist(values, bins=30)` |

柱状图需要类别 x 轴：`fig.add_axes(x=ub.Scale.category())`。对数轴用 `ub.Scale.log()`。坐标范围、刻度和刻度标签格式分别用 `xlim`、`ylim`、`xticks`、`xticks_labeled`、`ytick_format(ub.TickFormat.percent())` 及另一轴上的对应方法设置。

<p>
  <img alt="Pigment 主题下的分组柱状图" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/bars-pigment.png" width="49%">
  <img alt="Pigment 主题下带渐变填充的直方图" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/histogram-pigment.png" width="49%">
</p>

## 主题

内置三套主题：`Theme.PIGMENT`（暖色纸张底，衬线标题）、`Theme.GRAPHITE`（中性色，无衬线）和 `Theme.MIDNIGHT`（深色底，等宽数字）。主题是不可变的值，用它的 `with_*` 方法派生变体：

```python
theme = ub.Theme.MIDNIGHT.with_fill_style(ub.FillStyle.SOLID).with_legend_style(ub.LegendStyle.HEADER)
fig = ub.Figure(theme=theme)
```

![Midnight 主题下的两条正弦曲线](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/lines-midnight.png)

## 错误

错误由引发它的那次调用抛出：数据非法时是 `umber.DataError`，选项值非法时是 `umber.StyleError`，当前状态下不允许的调用是 `umber.UsageError`，类型不对的值是 `TypeError`。三种 umber 错误都继承自 `umber.UmberError`。traceback 会指向出错的参数：

```text
Traceback (most recent call last):
  File "example.py", line 7, in <module>
    ax.line(t, y, label="signal")
    ~~~~~~~^^^^^^^^^^^^^^^^^^^^^^
umber.DataError: Axes.line(): y[2] is inf; values must be finite.
  ax.line(t, y, label="signal")
             ^
hint: use NaN to leave a gap in the line.
```

```text
TypeError: Axes.line(): color is str; color must be of type Color.
  ax.line([0, 1, 2], [1, 3, 2], color="red")
                                ^^^^^^^^^^^
hint: use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.
```

在支持颜色的终端里，提示和高亮会以彩色显示；设置 `NO_COLOR` 可关闭颜色，设置 `FORCE_COLOR` 可强制开启。在 Jupyter 中同样会显示这些说明。

## 静态类型

包内附带类型信息（`py.typed`），并在严格模式下通过 pyright 和 mypy 检查。`Figure().add_axes()` 的类型是 `Axes[float, float]`；`add_axes(x=Scale.category())` 是 `Axes[str, float]`。只适用于某一类轴的方法也有相应的类型约束：

```python
ax = ub.Figure().add_axes()
ax.bar(["a", "b"], [1, 2])  # pyright 和 mypy 报错：bar() 需要 Axes[str, float]
```

## 性能

以下是 `crates/umber-core/benches` 中 [criterion](https://github.com/criterion-rs/criterion.rs) 测试套件的中位数，图幅 6 × 4 英寸、200 dpi，运行于 Linux 下的 Intel Core Ultra 9 290HX：

| 场景 | 耗时 |
| --- | ---: |
| 100 万点折线 → PNG | 53 ms |
| 100 万点折线 → SVG | 21 ms |
| 10 万点散点图 → PNG | 47 ms |
| 100 万个值的直方图 → PNG | 20 ms |

当折线的点数多于绘图区的像素列数时，会按列降采样，保留每列的首点、末点、最小值和最大值，视觉上无损。大型散点图最多使用 8 个线程合成；无论用几个线程，输出都完全相同。

## Rust

渲染核心作为 [`umber-core`](https://crates.io/crates/umber-core) crate 单独发布，可以直接在 Rust 中使用：

```rust
use umber_core::{AxisData, Figure, LineOptions, ScaleKind, Theme};

let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
figure
    .add_axes(ScaleKind::Linear, ScaleKind::Linear)?
    .title("Signal")
    .line(AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]), &[1.0, 3.0, 2.0, 4.0], LineOptions::default())?;
std::fs::write("signal.png", figure.to_png(144)?)?;
```

## 状态

umber 目前处于 0.x 版本。次版本之间 API 可能变化；同一次版本内的补丁版本保持兼容。更多示例见 [`examples/`](examples)。

## 许可证

以 MIT 或 Apache-2.0 双许可（`MIT OR Apache-2.0`），任选其一。内置字体采用 SIL Open Font License 1.1，参见 `crates/umber-core/fonts/LICENSES` 与 `crates/umber-core/fonts/MANIFEST.md`。发布的 wheel 包含静态链接的 Rust 依赖，其许可证列于 `THIRD-PARTY-LICENSES.txt`。
