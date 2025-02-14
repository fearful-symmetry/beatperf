
use std::collections::HashMap;

use crate::groups::*;
use super::{generic::{Generic, NoOpProcess, Processor}, Watcher};
use anyhow::Context;
use tracing::debug;

const EVENTS_KEY: &str = "libbeat.pipeline.events";
const QUEUE_KEY: &str = "libbeat.pipeline.queue";
const FILLED_PCT_KEY: &str = "libbeat.pipeline.queue.filled.pct";
pub struct Pipeline {
    group_events: Generic<u64, NoOpProcess<u64>>,
    group_queue: Generic<u64, NoOpProcess<u64>>,
    filled_pct: Generic<f64, PctProcessor>,
    fname: String
}

pub struct PctProcessor {}

impl Processor for PctProcessor {
    type InValue = f64;
    type OutValue = f64;
    fn new() -> Self {
        Self {  }
    }
    fn process(&self, raw: Self::InValue) -> Self::OutValue {
        raw  * 100.0
    }
}


impl Watcher for Pipeline {
    fn new(_ : Option<Vec<String>>) -> Self {
        let group_events = Generic::from(vec![EVENTS_KEY]);
        let group_queue = Generic::from(vec![QUEUE_KEY]);
        let filled_pct = Generic::from(vec![FILLED_PCT_KEY]);
        Pipeline { group_events, group_queue, filled_pct, fname: "pipeline".to_string() }
    }

    fn update(&mut self, new: &serde_json::Map<String, serde_json::Value>) {
        self.group_events.update(new);
        self.group_queue.update(new);
        self.filled_pct.update(new);
    }

    fn plot(&self) -> anyhow::Result<()> {  
        let name = format!("./{}_plot.svg", &self.fname);
        debug!("writing {}...", name);

    
        let root = SVGBackend::new(&name, SVG_SIZE).into_drawing_area();
        root.fill(&WHITE)?;

        let (upper_q, lower_3q) = root.split_vertically(SVG_SIZE.1/4);

        let (upper_bottom, lower_bottom) = lower_3q.split_vertically(((SVG_SIZE.1/4)*3)/2);

        // set up events subgraph
        let map_data_events = self.group_events.plot();
        gen_events_graph("Events".to_string(), map_data_events, self.group_events.datapoints(), &lower_bottom, 5, 18, EVENTS_KEY)?;

        // set up queue subgraph
        let map_data_queue = self.group_queue.plot();
        // skip any values ending in `pct` or `bytes`
        let filtered_map: HashMap<String, Vec<u64>> = map_data_queue.into_iter().filter(|(k, _)| !k.contains("bytes") && !k.contains("pct")).collect();
        gen_events_graph("Queue".to_string(), filtered_map, self.group_events.datapoints(), &upper_bottom, 5, 18, QUEUE_KEY)?;

        // set up percent full
        let map_data_full = self.filled_pct.plot();
        gen_pct_graph("Queue % Full".to_string(), map_data_full, self.filled_pct.datapoints(), upper_q)?;
    
        root.present().context("could not write file")?;

        Ok(())
    }
}

fn gen_pct_graph<DB: DrawingBackend<ErrorType: 'static>>(name: String, map: HashMap<String, Vec<f64>>, datapoints: usize, area : DrawingArea<DB, Shift>) -> anyhow::Result<()> {
    let (min, max) = get_min_max_float(&map)?;

    let headroom = (max - min) * HEADROOM_CHART_MAX;

    let mut chart_events = setup_graph(name, &area, 5, 18);
    let mut chart_context_events = chart_events.build_cartesian_2d(0usize..datapoints,min..max+headroom)?;
    chart_context_events.configure_mesh().y_label_formatter(&|i| pct_formatter(*i)).draw()?;

    for (idx, (name, group)) in map.iter().enumerate() {
        let color = Palette99::pick(idx).mix(0.9);
        chart_context_events.draw_series(LineSeries::new(group.iter().enumerate().map(|(p_idx, d)| (p_idx, *d)), color.stroke_width(2)))?
        .label(name.clone());
    }

    Ok(())
}
