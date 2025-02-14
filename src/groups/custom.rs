use anyhow::Context;
use plotters::prelude::*;
use tracing::debug;

use crate::groups::*;
use super::{generic::{Generic, NoOpProcess}, Watcher};


pub struct CustomMetrics {
    group: Generic<f64, NoOpProcess<f64>>,
    fname: String,
}


impl Watcher for CustomMetrics {
    fn new(fields: Option<Vec<String>>) -> Self {

        let group = if let Some(mf) = fields {
            Generic::from(mf)
        } else {
            Generic::from(vec![".beat.runtime.goroutines"])
        };
        
        CustomMetrics { fname: "custom".to_string(), group }
    }

    fn update(&mut self, new: &serde_json::Map<String, serde_json::Value>) {
        self.group.update(new);
    }

    fn plot(&self) -> anyhow::Result<()> {
        let map_data = self.group.plot();

        let name = format!("./{}_plot.svg", &self.fname);
        debug!("writing {}...", name);
    
        let (min, max) = get_min_max_float(&map_data)?;

        let root = SVGBackend::new(&name, SVG_SIZE).into_drawing_area();
        root.fill(&WHITE)?;
    
        let mut chart = setup_graph(self.fname.clone(), &root, DEFAULT_GRAPH_MARGIN, LABEL_SIZE_LEFT);
        let mut chart_con = chart.build_cartesian_2d(0usize..self.group.datapoints(), min..max)?;
    
        chart_con.configure_mesh().x_desc("Datapoints").y_desc("Values").draw()?;
    
        for (idx, (name, group)) in map_data.iter().enumerate() {
            let color = Palette99::pick(idx).mix(0.9);
            chart_con.draw_series(LineSeries::new(group.iter().enumerate().map(|(p_idx, d)| (p_idx, *d)), color.stroke_width(2)))?
            .label(name)
            .legend(move |(x, y)| Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled()));
        }
    
        chart_con.configure_series_labels().border_style(BLACK).background_style(WHITE.mix(0.8)).position(SeriesLabelPosition::UpperLeft).draw()?;
    
        root.present().context("could not write file")?;
        
        Ok(())
    }
}