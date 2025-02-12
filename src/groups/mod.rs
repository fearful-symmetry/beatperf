/*! 
 * beatperf treats metrics as groups, with each grouping of metrics corresponding to a single chart or graph.
 * Groups in general will attempt to strike a balance between being generic, and having some knowlegde of the fields or metrics involved, 
 * so they can generate more helpful and specific charts.
 * 
 */

use plotters::{chart::ChartBuilder, coord::Shift, prelude::*};

pub mod processdb;
pub mod memory;
mod generic;
 

pub trait Watcher {
    fn update(&mut self, new: &serde_json::Map<String, serde_json::Value>);
    fn plot(&self) -> anyhow::Result<()>;
    fn new() -> Self;
}

const LABEL_SIZE_LEFT: i32 = 12;
const LABEL_SIZE_BOTTOM: i32 = 12;
const SVG_SIZE: (u32, u32) = (1024, 768);
const CHART_NAME_FONT_PCT_SIZE: i32 = 5;
const HEADROOM_CHART_MAX: f64 = 0.10;

/// Helper for the plotter that formats the y-axis value for kilobytes
fn kbyte_formatter(raw: f64) -> String {
    if raw >= 100_000.0 {
        format!("{} MB", raw /1000.0)
    } else {
        format!("{} KB", raw)
    }
}

/// Helper to set up the base graph object
fn setup_graph<'e, DB: DrawingBackend>(name: String, root: &DrawingArea<DB, Shift> ) ->  ChartBuilder<'_, 'e, DB> {
    let mut chart_new = ChartBuilder::on(root);
    chart_new.caption(name, ("sans-serif", (CHART_NAME_FONT_PCT_SIZE).percent_height()))
    .set_label_area_size(LabelAreaPosition::Left, (LABEL_SIZE_LEFT).percent())
    .set_label_area_size(LabelAreaPosition::Bottom, (LABEL_SIZE_BOTTOM).percent())
    .margin((1).percent());

    chart_new
}