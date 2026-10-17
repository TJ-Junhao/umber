# umber

umber 是一个 Python 绘图库，渲染核心用 Rust 编写。它可以绘制折线图、阶梯图、散点图、面积图、堆叠面积图、柱状图、直方图、箱线图、小提琴图、气泡图、误差棒、区间带、杆状图、参考线、热力图、网格色块图（mesh）、二维直方图和六边形分箱图（hexbin），支持数值轴、对数轴、类别轴和时间轴，可以单独成图，也可以排成共享坐标轴的多面板网格。它提供 colormap 和 colorbar，排版考究，内置三套主题，输出 PNG 或 SVG。

[English](https://github.com/TJ-Junhao/umber/blob/main/README.md)

![Graphite 主题下两个站点的每日气温折线图](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/lines-graphite.png)

- **大数据量也快。** 渲染在 Rust 中完成：100 万个点的折线渲染成 PNG 约 58 ms，10 万个点的散点图约 50 ms（见[性能](#性能)）。
- **显式。** 每个选项都是有类型的值（`Color`、`Size`、`Dash`、`TickFormat` 等）。没有格式字符串，没有位置式的样式参数，也没有隐式转换。
- **尽早报错，并指出问题所在。** 非法输入在引发它的那次调用处就报错，附带说明、提示，并在 traceback 中高亮出错的参数。
- **静态类型。** 类别轴和数值轴是不同的类型：在数值轴上调用 `bar`，pyright 和 mypy 在运行前就会报错。
- **内置字体。** 内置字体覆盖的字符在任何机器上都显示一致；未覆盖的字符会回退到系统字体。

## 安装

```sh
pip install umber
```

需要 Python 3.12 或更新版本，以及 NumPy 2。我们为 Linux（x86_64 和 aarch64）、macOS（Apple silicon 和 Intel）和 Windows（x64）发布预编译 wheel；free-threaded 的 Python 3.14 以及 3.15 及更新版本另有专用 wheel。其他平台上 `pip` 会从源码构建，需要 Rust 1.88 或更新版本。

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
| 阶梯图 | `ax.step(x, y, where=ub.Step.MID)` |
| 堆叠面积图 | `ax.stacked_area(x, [ub.Series("A", a), ub.Series("B", b)])` |
| 箱线图、分组箱线图 | `ax.box(categories, groups)`、`ax.grouped_box(categories, [ub.Distribution("A", groups)])` |
| 小提琴图、分组小提琴图 | `ax.violin(categories, groups)`、`ax.grouped_violin(categories, distributions)` |
| 气泡图 | `ax.bubbles(x, y, sizes, label="Cities")`（附带尺寸图例） |
| 误差棒 | `ax.y_error_bars(x, low, high)`、`ax.x_error_bars(y, low, high)` |
| 区间带 | `ax.band(x, low, high)` |
| 杆状图 | `ax.stem(x, y)` |
| 参考线和参考区间 | `ax.hline(y)`、`ax.vline(x)`、`ax.hspan(low, high)`、`ax.vspan(low, high)` |
| 热力图 | `ax.heatmap(x_categories, y_categories, values, annotate=True)` |
| 网格色块图 | `ax.mesh(x_edges, y_edges, values)` |
| 二维直方图、六边形分箱图 | `ax.hist2d(x, y, bins=40)`、`ax.hexbin(x, y, gridsize=30)` |
| 按值着色的散点 | `ax.scatter(x, y, color=ub.ColorBy(values))` |

柱状图、箱线图和小提琴图需要一条类别轴。在类别 x 轴（`fig.add_axes(x=ub.Scale.category())`）上它们是竖直的；在类别 y 轴（`fig.add_axes(y=ub.Scale.category())`）上它们是水平的，以数值对类别画散点即为点图（dot plot）。热力图要求两条轴都是类别轴。对数轴用 `ub.Scale.log()`，时间轴用 `ub.Scale.time()`。坐标范围、刻度和刻度标签格式分别用 `xlim`、`ylim`、`xticks`、`xticks_labeled`、`ytick_format(ub.TickFormat.percent())` 及另一轴上的对应方法设置。

自动 limits 包含完整的 bubble marker 及其 outline；显式 limits 会裁切它们。Legend swatch 显示 mark 实际绘制的 fill，即与 plot background 合成后的颜色。

<p>
  <img alt="Pigment 主题下的分组柱状图" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/bars-pigment.png" width="49%">
  <img alt="Pigment 主题下带渐变填充的直方图" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/histogram-pigment.png" width="49%">
</p>

## Grid 与共享轴

`Figure(rows=2, cols=2)` 声明一个 grid。默认的 `width=ub.AUTO` 和 `height=ub.AUTO` 为每列分配 6 英寸、每行分配 4 英寸；显式尺寸则是整张 figure 的尺寸。添加 panel 时指定左上角 cell 的位置，索引从零开始：

```python
import copy
import umber as ub

fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.COL, legend=ub.Legend.MERGED)
fig.title("Station signals").subtitle("Shared time scales")
fig.add_axes(row=0, col=0).line([0, 1, 2], [1, 3, 2], label="North")
fig.add_axes(row=1, col=0).line([0, 1, 2], [2, 1, 3], label="South")
fig.add_axes(row=0, col=1, row_span=2).line([0, 1, 2], [1, 3, 2], label="North")
fig.save_png("subplots.png")

variant = copy.copy(fig)
variant.axes_at(row=0, col=1).title("Comparison")  # 原图不变
```

`Share.NONE`、`Share.ROW`、`Share.COL` 和 `Share.ALL` 指定哪些 panel 共享 scale、limits、ticks 和 tick format。默认使用合并的 legend；`Legend.PER_PANEL` 则为每个 panel 保留独立 legend。空 cell 保持空白，不允许 panel 相互重叠。

用 `fig.axes_at(row=..., col=..., x=..., y=...)` 取回已有 axes，也可用于副本。scale kind 和 log base 必须与创建时一致；类别轴需传入 `x=ub.Scale.category()`，返回类型为 `Axes[str, float]`。跨多个 cell 的 panel 只能通过左上角位置取回。仅当某个维度大小为 1 时，才能省略该维度的 row 或 col。旧的 `Figure.axes` 已移除，由这个有精确类型的方法替代。

`fig.cells()` 按 row-major 顺序返回不可变的 `Cell` 位置，包含已占用位置；`fig.cells(n)` 返回前 `n` 个。例如，`for cell, values in zip(fig.cells(), datasets): cell.add_axes().line(x, values)` 为每份数据添加一个 panel。`AUTO` 是有类型的 sentinel，不是 `None`，仅接受它的参数才能使用。

## 颜色与 colorbar

热力图、网格色块图、二维直方图和六边形分箱图按数值为每个格子着色；`color=ub.ColorBy(values)` 对散点图或气泡图的点做同样的事。同一个 axes 上所有按值着色的 mark 共用一个 color scale，plot 旁的 colorbar 说明这一映射。

```python
import numpy as np
import umber as ub

rng = np.random.default_rng(41)
x = rng.normal(size=20_000)
y = 0.6 * x + rng.normal(scale=0.7, size=x.size)

ax = ub.Figure(theme=ub.Theme.MIDNIGHT).add_axes()
ax.hexbin(x, y, gridsize=30, scale=ub.Scale.log()).colorbar("Count")
ax.figure.save_png("density.png")
```

`cmap=` 选择 colormap：每套主题各有自己的顺序（sequential）和发散（diverging）colormap，另内置 `Cmap.VIRIDIS`、`Cmap.MAGMA`、`Cmap.CIVIDIS` 和 `Cmap.BLUE_RED`。`Cmap.from_colors([...])` 用你给的颜色构建 colormap，颜色之间在感知均匀的 Oklab 空间中混合；`.reversed()` 将其反转。`scale=ub.Scale.log()` 按对数映射数值，`center=0` 使映射变为以该值为中心对称的发散映射。颜色范围从最小到最大的有限值；`ax.clim(low, high)` 可指定范围，范围外的值取两端颜色。

`ax.colorbar(label, position=..., format=...)` 设置 colorbar 的标签，用 `ColorbarPosition.BOTTOM` 移到 plot 下方，或用 `ColorbarPosition.NONE` 隐藏。在 grid 上，`Figure(share_color=ub.Share.ROW)` 让每行的 panel 共用一个颜色范围，并在该行右侧画一个 colorbar；`Share.COL` 在每列下方各画一个，`Share.ALL` 则为整个 grid 画一个。`fig.colorbar(...)` 设置这些共享的 colorbar。

<p>
  <img alt="Graphite 主题下带数值标注、使用发散 colormap 的相关系数热力图" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/heatmap-graphite.png" width="49%">
  <img alt="Midnight 主题下按对数 color scale 着色的六边形点计数图" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/hexbin-midnight.png" width="49%">
</p>

## 时间轴

`ub.Scale.time()` 创建时间轴。它接受 `datetime`、`date` 和 `numpy.datetime64` 值，任意单位的 NumPy `datetime64` 数组，以及不带时区的 pandas 和 polars datetime 列：

```python
import numpy as np
import umber as ub

hours = np.datetime64("2026-03-02T00", "h") + np.arange(7 * 24)
demand = 60 + 20 * np.sin((np.arange(7 * 24) - 6) / 24 * 2 * np.pi)

ax = ub.Figure(8, 4).add_axes(x=ub.Scale.time())
ax.band(hours, demand - 6, demand + 6, label="Range").line(hours, demand, label="Demand")
ax.vspan(np.datetime64("2026-03-05T01"), np.datetime64("2026-03-05T05"), label="Maintenance")
ax.figure.save_png("demand.png")
```

刻度落在月、日、小时等日历步长上，标签简洁；`xtick_format(ub.TickFormat.iso())` 改用 ISO 8601 格式。时间轴上的 limits、ticks、参考线和参考区间都接受时间值。时间精确到微秒，`NaT` 会留出空隙；带时区的时间会被拒绝，请先转换为不带时区的时间，例如在 pandas 中使用 `.tz_convert(None)`。区间带和堆叠面积图只在 x 轴上接受时间。

![Pigment 主题下一周的逐小时负荷，含波动范围和维护时段](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/time-pigment.png)

## 标题与 legend

`fig.title(text)` 和 `fig.subtitle(text)` 设置整个 grid 上方的文字；`ax.title(text)` 和 `ax.subtitle(text)` 只属于一个 panel。每个方法都返回原对象，支持链式调用；空字符串可移除文字。figure title 的字号是 panel title 的 1.5 倍。

`Theme.with_title_align(ub.Align.LEFT)` 设置标题对齐方式，也可使用 `CENTER` 和 `RIGHT`。subtitle 跟随对应 title 对齐。多 panel figure 的 merged legend 固定放在 figure title 和 subtitle 下方，跟随它们对齐，没有独立的位置设置。若要按 panel 放置 legend，请使用 `Legend.PER_PANEL`。

```python
import umber as ub

theme = ub.Theme.PIGMENT.with_title_align(ub.Align.CENTER).with_legend_position(
    ub.LegendPosition.BOTTOM
)
fig = ub.Figure(theme=theme).title("Station signals")
ax = fig.add_axes().line([0, 1, 2], [1, 3, 2], label="North")
ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
ax.legend(ub.LegendStyle.HEADER, position=ub.AUTO)  # 恢复 theme 的 BOTTOM
```

对单 panel figure 和 `Legend.PER_PANEL`，`LegendPosition.TOP_LEFT`（默认）和 `TOP_RIGHT` 将 legend 横排在 panel title 下方；`RIGHT` 放在 plot 右侧纵排；`BOTTOM` 横排在 x axis title 下方；`BEST` 在 plot 内选择遮挡数据最少的位置。panel legend 的位置不跟随标题对齐。省略 `position` 或传入 `ub.AUTO` 会清除之前的覆盖值，使用 theme 的位置，并不等于 `BEST`。`LegendStyle.DIRECT` 只接受默认的 `position=ub.AUTO`，因为标签固定在曲线末端。`LegendStyle.NONE` 隐藏 legend。

## 主题

内置三套主题：`Theme.PIGMENT`（暖色纸张底，衬线标题）、`Theme.GRAPHITE`（中性色，无衬线）和 `Theme.MIDNIGHT`（深色底，等宽数字）。主题是不可变的值，用它的 `with_*` 方法派生变体：

```python
import umber as ub

theme = ub.Theme.MIDNIGHT.with_fill_style(ub.FillStyle.SOLID).with_legend_style(
    ub.LegendStyle.HEADER
)
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

包内附带类型信息（`py.typed`），并在严格模式下通过 pyright 和 mypy 检查。`Figure().add_axes()` 的类型是 `Axes[float, float]`；`add_axes(x=Scale.category())` 是 `Axes[str, float]`，`add_axes(y=Scale.category())` 是 `Axes[float, str]`，两条类别轴是 `Axes[str, str]`，`add_axes(x=Scale.time())` 是 `Axes[Time, float]`。只适用于某一类轴的方法也有相应的类型约束：

```python
import umber as ub

ax = ub.Figure().add_axes()
ax.bar(["a", "b"], [1, 2])  # pyright 和 mypy 报错：bar() 需要 Axes[str, float]
```

## 性能

以下是 `crates/umber-core/benches` 中 [criterion](https://github.com/criterion-rs/criterion.rs) 测试套件的中位数，图幅 6 × 4 英寸、200 dpi，运行于 Linux 下的 Intel Core Ultra 9 290HX：

| 场景 | 耗时 |
| --- | ---: |
| 100 万点折线 → PNG | 58 ms |
| 时间轴上的 100 万点折线 → PNG | 54 ms |
| 100 万点折线 → SVG | 19 ms |
| 10 万点散点图 → PNG | 50 ms |
| 100 万个值的直方图 → PNG | 21 ms |
| 100 万个点的二维直方图，100 × 100 个 bin → PNG | 12 ms |
| 100 万个点的六边形分箱图，grid size 50 → PNG | 69 ms |
| 1,000 × 1,000 个格子的热力图 → PNG | 0.96 s |
| 4 × 5 网格，每格一条 1 万个点的折线 → PNG | 103 ms |

对于 x 值有序的数值或时间折线，按 300 dpi 的标称分辨率计算绘图区的像素列数，点数超过列数的四倍时按列降采样，保留每列的首点、末点、最小值和最大值，视觉上无损。大型散点图最多使用 8 个线程合成；无论用几个线程，输出都完全相同。

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

umber 目前处于 0.x 版本。API 尚未稳定，可能变更或移除，且不保证提供弃用期。更多示例见 [`examples/`](https://github.com/TJ-Junhao/umber/tree/main/examples)。

## 许可证

以 MIT 或 Apache-2.0 双许可（`MIT OR Apache-2.0`），任选其一。内置字体采用 SIL Open Font License 1.1，参见 `crates/umber-core/fonts/LICENSES` 与 `crates/umber-core/fonts/MANIFEST.md`。发布的 wheel 包含静态链接的 Rust 依赖，其许可证列于 `THIRD-PARTY-LICENSES.txt`。
