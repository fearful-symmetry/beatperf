use anyhow::{anyhow, Context};
use plotters::prelude::*;
use tracing::debug;

use crate::groups::*;

use super::{generic::{Generic, NoOpProcess}, Watcher};


pub struct ProcessDB {
    group: Generic<u64, NoOpProcess<u64>>,
    fname: String
}


impl Watcher for ProcessDB {
    fn new() -> Self {
        let group = Generic::from(vec!["processor.add_session_metadata.processdb"]);
        ProcessDB { group, fname: "processdb".to_string() }
    }

    fn update(&mut self, new: &serde_json::Map<String, serde_json::Value>) {
        self.group.update(new);
    }

    fn plot(&self) -> anyhow::Result<()> {
        let map_data = self.group.plot();
        let max =  map_data.iter().filter_map(| (_key, value) | value.iter().max())
        .max().copied().ok_or_else(||anyhow!("data does not have any values"))?;

        let name = format!("./{}_plot.svg", &self.fname);
        debug!("writing {}...", name);
    
        // You'd think it would be easy to make this generic and throw it in a function.
        // YOU WOULD BE WRONG
        // the plotter crate does some bonkers stuff with generics, so wrapping this all in function that can take different types of
        // range values is a nightmare
        let root = SVGBackend::new(&name, SVG_SIZE).into_drawing_area();
        root.fill(&WHITE)?;
        let mut chart = setup_graph(self.fname.clone(), &root);
        let mut chart_con = chart.build_cartesian_2d(0usize..self.group.datapoints(),(0..max).log_scale())?;
        chart_con.configure_mesh().x_desc("Datapoints").y_desc("DB Values").draw()?;
    
    
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