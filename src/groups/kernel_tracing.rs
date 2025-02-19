use anyhow::Context;
use plotters::prelude::*;
use tracing::debug;

use crate::groups::*;
use super::{generic::{Generic, NoOpProcess}, Watcher};

const PROCDB_KEY: &str = "processor.add_session_metadata.kernel_tracing";

pub struct KernelTracing {
    group: Generic<u64, NoOpProcess<u64>>,
    fname: String
}


impl Watcher for KernelTracing {
    fn new(_ : Option<Vec<String>>) -> Self {
        let group = Generic::from(vec![PROCDB_KEY]);
        KernelTracing { group, fname: "kernel_tracing".to_string() }
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