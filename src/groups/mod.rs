/*! 
 * beatperf treats metrics as groups, with each grouping of metrics corresponding to a single chart or graph.
 * Groups in general will attempt to strike a balance between being generic, and having some knowlegde of the fields or metrics involved, 
 * so they can generate more helpful and specific charts.
 * 
 */

use std::collections::HashMap;
use anyhow::anyhow;

use plotters::{chart::ChartBuilder, coord::Shift, prelude::*};

pub mod processdb;
pub mod memory;
pub mod pipeline;
pub mod output;
pub mod custom;

mod generic;
 
/// A trait for groups of metrics that allows a group to have their own opinions about how a set of metrics should be graphed and ordered
pub trait Watcher {
    /// Update the metrics based on a map we get from beats
    fn update(&mut self, new: &serde_json::Map<String, serde_json::Value>);
    /// Generate an SVG plot
    fn plot(&self) -> anyhow::Result<()>;
    /// Create a new instance with optional metrics. 
    fn new(additional_fields: Option<Vec<String>>) -> Self;
}

/// The default margin percentage for a graph
const DEFAULT_GRAPH_MARGIN: i32 = 1;
/// The default left label size
const LABEL_SIZE_LEFT: i32 = 9;
/// The default bottom label size
const LABEL_SIZE_BOTTOM: i32 = 12;
/// The graph dimensions
const SVG_SIZE: (u32, u32) = (1024, 768);
/// The default font size for labels
const CHART_NAME_FONT_PCT_SIZE: i32 = 5;
/// The defauld additional y axis to add, to make way for the graph legend
const HEADROOM_CHART_MAX: f64 = 0.10;

/// Helper for the plotter that formats the y-axis value for kilobytes
fn kbyte_formatter(raw: f64) -> String {
    if raw >= 100_000.0 {
        format!("{} MB", raw /1000.0)
    } else {
        format!("{} KB", raw)
    }
}

fn pct_formatter(raw: f64) -> String {
    format!("{:.2}%", raw)
}

/// Helper to set up the base graph object
fn setup_graph<'e, DB: DrawingBackend>(name: String, root: &DrawingArea<DB, Shift>, margin: i32, label_left_size: i32 ) ->  ChartBuilder<'_, 'e, DB> {
    let mut chart_new = ChartBuilder::on(root);
    chart_new.caption(name, ("sans-serif", (CHART_NAME_FONT_PCT_SIZE).percent_height()))
    .set_label_area_size(LabelAreaPosition::Left, (label_left_size).percent())
    .set_label_area_size(LabelAreaPosition::Bottom, (LABEL_SIZE_BOTTOM).percent())
    .margin((margin).percent());

    chart_new
}


fn get_min_max_float(map: &HashMap<String, Vec<f64>>) -> anyhow::Result<(f64, f64)> {
    let max = map.iter().filter_map(| (_key, value) | value.iter().copied().reduce(f64::max))
    .reduce(f64::max).ok_or_else(||anyhow!("data does not have any values"))?;

    let mut min = map.iter().filter_map(| (_key, value) | value.iter().copied().reduce(f64::min))
    .reduce(f64::min).ok_or_else(||anyhow!("data does not have any values"))?;

    if min == max {
        min = 0.0
    }

    Ok((min, max))
}

fn get_min_max_uint(map: &HashMap<String, Vec<u64>>) -> anyhow::Result<(u64, u64)> {
    let max = map.iter().filter_map(| (_key, value) | value.iter().max())
    .max().copied().ok_or_else(||anyhow!("data does not have any values"))?;

    let mut min = map.iter().filter_map(| (_key, value) | value.iter().min())
    .min().copied().ok_or_else(||anyhow!("data does not have any values"))?;

    if min == max {
        min = 0
    }

    Ok((min, max))
}

/// Genterate the basic setup for the graph
fn gen_events_graph<DB: DrawingBackend<ErrorType: 'static>>
(name: String, map: HashMap<String, Vec<u64>>, datapoints: usize, area: &DrawingArea<DB, Shift>, margin: i32, label_left_size: i32, name_prefix: &str) -> anyhow::Result<()> {
    let (min, max) = get_min_max_uint(&map)?;

    let mut chart_events = setup_graph(name, area, margin, label_left_size);
    let mut chart_context_events = chart_events.build_cartesian_2d(0usize..datapoints,(min..max).log_scale())?;
    chart_context_events.configure_mesh().y_desc("events").draw()?;


    for (idx, (name, group)) in map.iter().enumerate() {
        let color = Palette99::pick(idx).mix(0.9);
        chart_context_events.draw_series(LineSeries::new(group.iter().enumerate().map(|(p_idx, d)| (p_idx, *d)), color.stroke_width(2)))?
        .label(name.trim_start_matches(name_prefix))
        .legend(move |(x, y)| Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled()));

    }

    chart_context_events.configure_series_labels().border_style(BLACK).background_style(WHITE.mix(0.8)).position(SeriesLabelPosition::UpperLeft).draw()?;

    Ok(())
}