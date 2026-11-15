//! Category names of several lines: the tick labels of a category axis that `"\n"` breaks into
//! lines, their placement, the room the axis reserves for them and the spacing of automatic
//! labels.

use super::*;
use crate::layout::{LINE_PITCH, TextBlock};
use crate::resolve::AxisTick;
use crate::scene::Item;
use crate::{AxisData, AxisValue, BarOptions, ColorSpec, ScaleKind, Weight};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

/// A 6 by 4 in figure of bars over `names`, vertical on a category x axis, or horizontal on a
/// category y axis when `rows`. The value axis has fixed ticks, so its labels take the same
/// room whatever the size of the plot.
fn bars(names: &[String], rows: bool) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let (x, y) = if rows {
        (ScaleKind::Linear, ScaleKind::Category)
    } else {
        (ScaleKind::Category, ScaleKind::Linear)
    };
    let mut axes = f.add_axes(x, y).unwrap();
    axes.bar(
        names.to_vec(),
        &vec![1.0; names.len()],
        BarOptions::default(),
    )
    .unwrap();
    if rows {
        axes.xticks(&[0.0, 1.0]).unwrap();
    } else {
        axes.yticks(&[0.0, 1.0]).unwrap();
    }
    f
}

fn plan(f: &Figure) -> (Resolved, Layout) {
    plan_figure(f, (0, 0), FontLibrary::global())
}

/// The texts of the lines of `label`.
fn lines(label: &TextBlock) -> Vec<&str> {
    label.lines().map(|line| line.text.as_str()).collect()
}

#[test]
fn an_x_category_label_stacks_its_lines_each_centred_under_the_tick() {
    let f = bars(&owned(&["2025", "2026\n(to Oct)"]), false);
    let (resolved, layout) = plan(&f);
    let [one, two] = layout.x_tick_labels.as_slice() else {
        panic!("{:?}", layout.x_tick_labels)
    };
    assert_eq!(lines(one), ["2025"]);
    assert_eq!(lines(two), ["2026", "(to Oct)"]);
    let centre = layout.plot.x_at(resolved.x.ticks[1].pos);
    let pitch = LINE_PITCH * f64::from(two.first().style.size);
    for (i, line) in two.lines().enumerate() {
        assert!(close(line.x + line.width() / 2.0, centre), "line {i}");
        assert!(close(line.y - one.first().y, i as f64 * pitch), "line {i}");
    }
    assert!(close(
        two.width(),
        two.lines().map(|l| l.width()).fold(0.0, f64::max)
    ));
    // The scene draws each line as its own text.
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let mut texts = Vec::new();
    collect_texts(&scene.items, &mut texts);
    assert!(texts.contains(&"2026".to_owned()) && texts.contains(&"(to Oct)".to_owned()));
    assert!(!texts.iter().any(|text| text.contains('\n')));
}

fn collect_texts(items: &[Item], out: &mut Vec<String>) {
    for item in items {
        match item {
            Item::Group { items, .. } => collect_texts(items, out),
            Item::Text(text) | Item::RotatedText { text, .. } => out.push(text.text.clone()),
            _ => {}
        }
    }
}

#[test]
fn the_x_tick_row_is_as_tall_as_the_label_of_the_most_lines() {
    let (_, flat) = plan(&bars(&owned(&["a", "b", "c"]), false));
    let (_, tall) = plan(&bars(&owned(&["a", "b\nb", "c\nc\nc"]), false));
    let size = f64::from(tall.x_tick_labels[0].first().style.size);
    // Two further lines take two pitches more from the plot, and nothing else moves.
    assert!(close(
        flat.plot.height - tall.plot.height,
        2.0 * LINE_PITCH * size
    ));
    assert!(close(flat.plot.y, tall.plot.y));
    assert!(close(flat.plot.x, tall.plot.x));
}

#[test]
fn a_y_category_label_is_centred_on_its_tick_with_each_line_right_aligned() {
    let f = bars(&owned(&["North", "South\nand East", "West"]), true);
    let (resolved, layout) = plan(&f);
    let label = &layout.y_tick_labels[1];
    assert_eq!(lines(label), ["South", "and East"]);
    for line in label.lines() {
        assert!(close(line.x + line.width(), layout.plot.x - 6.0));
    }
    // The block is centred where a label of one line sits.
    let size = f64::from(label.first().style.size);
    let one_line = layout.plot.y_at(resolved.y.ticks[1].pos) + 0.35 * size;
    let last = label.lines().last().unwrap();
    assert!(close((label.first().y + last.y) / 2.0, one_line));
    assert!(close(last.y - label.first().y, LINE_PITCH * size));
    // The column is as wide as the widest line.
    let (_, flat) = plan(&bars(&owned(&["North", "and East", "West"]), true));
    assert!(close(layout.plot.x, flat.plot.x));
}

#[test]
fn a_y_label_of_several_lines_takes_room_above_and_below_the_plot() {
    let (_, flat) = plan(&bars(&owned(&["a", "b"]), true));
    let (_, tall) = plan(&bars(&owned(&["a", "b\nb\nb"]), true));
    // The label of a tick at the plot's top or bottom would reach a pitch further each way.
    let size = f64::from(tall.y_tick_labels[0].first().style.size);
    let pitch = LINE_PITCH * size;
    assert!(close(tall.plot.y - flat.plot.y, pitch));
    assert!(tall.plot.bottom() <= flat.plot.bottom());
    let last = tall.y_tick_labels[1].lines().last().unwrap();
    assert!(last.y <= tall.frame.bottom());
}

/// The ticks of `side` of a figure of bars over `names`.
fn ticks(names: &[String], rows: bool) -> Vec<AxisTick> {
    let (resolved, _) = plan(&bars(names, rows));
    if rows {
        resolved.y.ticks
    } else {
        resolved.x.ticks
    }
}

#[test]
fn automatic_labels_keep_apart_by_their_whole_blocks() {
    // On x a label takes the width of its widest line: the same labels are kept as for names
    // of that line alone.
    let wide: Vec<String> = (0..40).map(|i| format!("Category {i:02}")).collect();
    let stacked: Vec<String> = wide.iter().map(|name| format!("{name}\nx")).collect();
    let kept = |ticks: Vec<AxisTick>| ticks.into_iter().map(|t| t.pos).collect::<Vec<_>>();
    assert_eq!(kept(ticks(&stacked, false)), kept(ticks(&wide, false)));
    assert!(ticks(&wide, false).len() < 40);
    // On y a label takes the height of all its lines, so taller labels are thinned more.
    let short: Vec<String> = (0..24).map(|i| format!("c{i}")).collect();
    let tall: Vec<String> = short.iter().map(|name| format!("{name}\n-\n-")).collect();
    let (few, many) = (ticks(&tall, true).len(), ticks(&short, true).len());
    assert!(few < many, "{few} {many}");
}

#[test]
fn a_highlighted_category_of_several_lines_sets_every_line_semibold() {
    let mut f = bars(&owned(&["2025", "2026\n(to Oct)"]), false);
    f.single_axes_mut()
        .xtick_highlight(owned(&["2026\n(to Oct)"]), ColorSpec::AUTO)
        .unwrap();
    let (_, layout) = plan(&f);
    let label = &layout.x_tick_labels[1];
    assert!(
        label
            .lines()
            .all(|line| line.style.weight == Weight::SemiBold)
    );
    assert_eq!(
        layout.x_tick_labels[0].first().style.weight,
        Weight::Regular
    );
    assert_eq!(layout.x_tick_colors[1], f.theme().emphasis_color());
}

#[test]
fn a_category_name_is_the_same_string_in_every_mark_and_reference() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let name = "2026\n(to Oct)";
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .bar(owned(&["2025", name]), &[1.0, 2.0], BarOptions::default())
        .unwrap()
        .line(
            AxisData::Categories(owned(&[name])),
            &[1.5],
            crate::LineOptions::default(),
        )
        .unwrap()
        .vline(
            AxisValue::Category(name.into()),
            crate::RuleOptions::default(),
        )
        .unwrap()
        .vline_between(
            crate::Between::new("2025", name).unwrap(),
            crate::RuleOptions::default(),
        )
        .unwrap();
    let (resolved, _) = plan(&f);
    let crate::resolve::AxisScale::Category(scale) = &resolved.x.scale else {
        panic!("a category axis")
    };
    assert_eq!(scale.categories(), owned(&["2025", name]));
    assert!(f.to_scene(FontLibrary::global()).is_ok());
}
