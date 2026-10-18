//! Category points alone and above distribution summaries, in both orientations.

use umber_core::{
    BoxOptions, FontLibrary, ScaleKind, SizeSpec, StripOptions, SwarmOptions, ViolinOptions,
};

use super::super::{Output, axes, emit_scene, figure, linear, log, themes};

/// Emits strip and swarm figures for every theme and orientation.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for horizontal in [false, true] {
            for swarm in [false, true] {
                for under in ["alone", "box", "violin"] {
                    let (x, y) = if horizontal {
                        (log(10.0), ScaleKind::Category)
                    } else {
                        (ScaleKind::Category, linear())
                    };
                    let mut f = figure(6.0, 4.0, theme.clone(), x, y);
                    let categories = ["North", "South"].map(String::from).to_vec();
                    let groups = vec![
                        vec![1.0, 2.0, 2.0, 3.0, 5.0, 6.0],
                        vec![2.0, 3.0, 4.0, 4.0, 6.0, 8.0],
                    ];
                    let mut ax = axes(&mut f);
                    ax.title("Individual samples");
                    match under {
                        "box" => {
                            ax.box_plot(categories.clone(), groups.clone(), BoxOptions::default())
                                .unwrap();
                        }
                        "violin" => {
                            ax.violin(categories.clone(), groups.clone(), ViolinOptions::default())
                                .unwrap();
                        }
                        _ => {}
                    }
                    let size = SizeSpec::pt(5.0).unwrap();
                    if swarm {
                        ax.swarm(
                            categories,
                            groups,
                            SwarmOptions {
                                size,
                                label: "Samples".into(),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    } else {
                        ax.strip(
                            categories,
                            groups,
                            StripOptions {
                                size,
                                label: "Samples".into(),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    }
                    let kind = if swarm { "swarm" } else { "strip" };
                    let direction = if horizontal {
                        "horizontal-log"
                    } else {
                        "vertical"
                    };
                    emit_scene(
                        &format!("{kind}-{under}-{direction}-{name}"),
                        &f.to_scene(lib).unwrap(),
                        sink,
                    );
                }
            }
        }
    }
}
