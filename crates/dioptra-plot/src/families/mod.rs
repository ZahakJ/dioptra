//! Graph families. Each module turns the spec and data into a [`crate::plot::Plot`]
//! (or, for pie and donut, a scene directly).

pub mod before_after;
pub mod bland_altman;
pub mod boxplot;
pub mod column;
pub mod common;
pub mod density;
pub mod estimation;
pub mod forest;
pub mod heatmap;
pub mod histogram;
pub mod kaplan_meier;
pub mod pie;
pub mod qq;
pub mod raincloud;
pub mod residuals;
pub mod roc;
pub mod scatter;
pub mod strip;
pub mod superplot;
pub mod violin;
pub mod volcano;

use crate::plot::{annotation_layers, render, Ctx, Plot};
use crate::scene::Scene;

/// Every family name, in menu order.
pub const FAMILIES: [&str; 25] = [
    "scatter",
    "line",
    "column",
    "box",
    "violin",
    "strip",
    "raincloud",
    "before_after",
    "estimation",
    "superplot",
    "histogram",
    "density",
    "heatmap",
    "kaplan_meier",
    "roc",
    "bland_altman",
    "volcano",
    "forest",
    "correlation_matrix",
    "pie",
    "donut",
    "qq",
    "residuals",
    // Aliases accepted for familiarity.
    "xy",
    "bar",
];

/// Build the scene for a family; `None` for an unknown family name.
pub fn build(ctx: &Ctx<'_>) -> Option<Scene> {
    let plot: Plot = match ctx.spec.family.as_str() {
        "scatter" | "xy" => scatter::plot(ctx, false),
        "line" => scatter::plot(ctx, true),
        "column" | "bar" => column::plot(ctx),
        "box" => boxplot::plot(ctx),
        "violin" => violin::plot(ctx),
        "strip" => strip::plot(ctx),
        "raincloud" => raincloud::plot(ctx),
        "before_after" => before_after::plot(ctx),
        "estimation" => estimation::plot(ctx),
        "superplot" => superplot::plot(ctx),
        "histogram" => histogram::plot(ctx),
        "density" => density::plot(ctx),
        "heatmap" => heatmap::plot(ctx),
        "correlation_matrix" => heatmap::correlation(ctx),
        "kaplan_meier" => kaplan_meier::plot(ctx),
        "roc" => roc::plot(ctx),
        "bland_altman" => bland_altman::plot(ctx),
        "volcano" => volcano::plot(ctx),
        "forest" => forest::plot(ctx),
        "qq" => qq::plot(ctx),
        "residuals" => residuals::plot(ctx),
        "pie" => return Some(pie::scene(ctx, false)),
        "donut" => return Some(pie::scene(ctx, true)),
        _ => return None,
    };
    let mut plot = plot;
    plot.layers.extend(annotation_layers(ctx));
    Some(render(ctx, plot))
}
