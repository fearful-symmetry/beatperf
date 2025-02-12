use anyhow::{anyhow, Context};
use plotters::prelude::*;
use tracing::debug;

use crate::groups::*;

use super::{generic::{Generic, Processor}, Watcher};


pub struct MemoryProcessor {}

impl Processor for MemoryProcessor {
    type InValue = u64;
    type OutValue = f64;
    fn new() -> Self {
        Self {  }
    }
    fn process(&self, raw: Self::InValue) -> Self::OutValue {
        raw as f64 / 1000.0
    }
}

pub struct MemoryMetrics {
    group: Generic<f64, MemoryProcessor>,
    fname: String
}

impl Watcher for MemoryMetrics {

    fn new() -> Self {
        let group = Generic::from(vec!["beat.memstats"]);
        MemoryMetrics { group, fname: "memstat".to_string() }
    }

    fn update(&mut self, new: &serde_json::Map<String, serde_json::Value>) {
        self.group.update(new);
    }

    fn plot(&self) -> anyhow::Result<()> {
        let mut map_data = self.group.plot();
        // filter out the memory_total metric, which is a massive counter that sums all memory bytes
        map_data.remove("beat.memstats.memory_total");

        let max = map_data.iter().filter_map(| (_key, value) | value.iter().copied().reduce(f64::max))
            .reduce(f64::max).ok_or_else(||anyhow!("data does not have any values"))?;
        let min = map_data.iter().filter_map(| (_key, value) | value.iter().copied().reduce(f64::min))
            .reduce(f64::min).ok_or_else(||anyhow!("data does not have any values"))?;

        // give the top of the chart some headroom, this way the legend won't collide with the graphs.
        let headroom = (max - min) * HEADROOM_CHART_MAX;

        let name = format!("./{}_plot.svg", self.fname);
        debug!("writing {}...", name);

        let root = SVGBackend::new(&name, SVG_SIZE).into_drawing_area();
        root.fill(&WHITE)?;
    
        let mut chart = setup_graph(self.fname.clone(), &root);
        let mut chart_con = chart.build_cartesian_2d(0usize..self.group.datapoints(), min..(max + headroom))?;
    
        chart_con.configure_mesh().x_desc("Datapoints").y_desc("Memory Usage").y_label_formatter(&|i| kbyte_formatter(*i)).draw()?;
    
        for (idx, (name, group)) in map_data.iter().enumerate() {
            let color = Palette99::pick(idx).mix(0.9);
            chart_con.draw_series(LineSeries::new(group.iter().enumerate().map(|(p_idx, d)| (p_idx, *d)), color.stroke_width(2)))?
            .label(name)
            .legend(move |(x, y)| Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled()));
    
        }
    
        chart_con.configure_series_labels().border_style(BLACK).position(SeriesLabelPosition::UpperLeft).draw()?;
    
        root.present().context("could not write file")?;

        Ok(())
    }
}