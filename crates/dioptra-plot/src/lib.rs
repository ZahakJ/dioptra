//! Dioptra's plotting engine (DESIGN §6, §7).
//!
//! A [`spec::GraphSpec`] plus extracted [`data::GraphData`] becomes a device-independent
//! [`scene::Scene`] in points ([`scene()`]); the scene renders to SVG with our own writer
//! ([`svg::to_svg`]), to PNG through resvg ([`output::to_png`]) and to PDF with embedded
//! subset fonts through svg2pdf ([`output::to_pdf`]). Text is set in the bundled Inter
//! fonts and measured with their own metrics, so the same spec gives the same bytes on
//! every platform and in the browser build.
//!
//! Every scene element that stands for a spec field carries an `id` and a `data-target`
//! JSON pointer; [`scene::Scene::targets`] lists them with bounding boxes so the graph
//! editor can map clicks to inspector fields.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod compose;
pub mod data;
pub mod detmath;
pub mod families;
pub mod output;
pub mod plot;
pub mod samples;
pub mod scene;
pub mod spec;
pub mod stats;
pub mod svg;
pub mod text;
pub mod theme;
pub mod ticks;

pub use compose::compose_layout;
pub use families::FAMILIES;
pub use output::{to_pdf, to_png, OutputError};
pub use scene::{Scene, Target};
pub use svg::to_svg;

/// Errors from building a graph.
#[derive(Debug, thiserror::Error)]
pub enum PlotError {
    /// The family name is not one the engine draws.
    #[error("there is no graph family called \"{0}\"; choose one of: {list}", list = FAMILIES.join(", "))]
    UnknownFamily(String),
    /// The size is out of range.
    #[error("the graph size must be between 10 and 1000 mm on each side (got {0} × {1} mm)")]
    Size(f64, f64),
    /// PNG or PDF output failed.
    #[error(transparent)]
    Output(#[from] OutputError),
}

/// Build the scene for a graph.
pub fn scene(spec: &spec::GraphSpec, data: &data::GraphData) -> Result<Scene, PlotError> {
    let (w, h) = (spec.style.size_mm.width, spec.style.size_mm.height);
    if !((10.0..=1000.0).contains(&w) && (10.0..=1000.0).contains(&h)) {
        return Err(PlotError::Size(w, h));
    }
    let ctx = plot::Ctx::new(spec, data);
    families::build(&ctx).ok_or_else(|| PlotError::UnknownFamily(spec.family.clone()))
}

/// A rendered graph: SVG text, its editor targets and its size.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// SVG document.
    pub svg: String,
    /// Clickable elements.
    pub targets: Vec<Target>,
    /// Width in millimetres.
    pub width_mm: f64,
    /// Height in millimetres.
    pub height_mm: f64,
}

/// Render a scene to SVG with its targets.
pub fn rendered(scene: &Scene) -> Rendered {
    Rendered {
        svg: to_svg(scene),
        targets: scene.targets(),
        width_mm: scene.width_mm(),
        height_mm: scene.height_mm(),
    }
}

/// Build and render a graph to SVG.
pub fn render_svg(spec: &spec::GraphSpec, data: &data::GraphData) -> Result<Rendered, PlotError> {
    Ok(rendered(&scene(spec, data)?))
}
