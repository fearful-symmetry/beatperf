use anyhow::Context;
use plotters::prelude::*;
use tracing::debug;

use crate::groups::*;
use super::{generic::{Generic, NoOpProcess}, Watcher};

const PROCDB_KEY: &str = "libbeat.output.events";

pub struct Output {
    group: Generic<u64, NoOpProcess<u64>>,
    fname: String
}


impl Watcher for Output {
    fn new(_ : Option<Vec<String>>) -> Self {
        let group = Generic::from(vec![PROCDB_KEY]);
        Output { group, fname: "Output Events".to_string() }
    }

    fn update(&mut self, new: &serde_json::Map<String, serde_json::Value>) {
        self.group.update(new);
    }

    fn plot(&self) -> anyhow::Result<()> {
        let map_data = self.group.plot();

        let name = format!("./{}_plot.svg", &self.fname);
        debug!("writing {}...", name);
    
        let root = SVGBackend::new(&name, SVG_SIZE).into_drawing_area();
        root.fill(&WHITE)?;

        gen_events_graph(self.fname.clone(), map_data, self.group.datapoints(), &root, DEFAULT_GRAPH_MARGIN, LABEL_SIZE_LEFT, PROCDB_KEY)?;
    
        root.present().context("could not write file")?;

        Ok(())
    }
}