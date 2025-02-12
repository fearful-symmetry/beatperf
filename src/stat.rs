use std::{collections::{HashMap, VecDeque}, fmt::Display, slice};
use serde_json::Number;
use tracing::{debug, error};

use crate::{fields::{GenericMetricFields, KbMetricFields}, Cli};

/// MetricField is a trait that allows handlers to treat metrics differently, in case a 
/// given metric requires unique handling or labeling. Metrics that don't require any special treatment can use `GenericMetricFields`.
pub trait MetricField: std::fmt::Debug + Display {
    /// return a f64 representation of the final item in the metric set
    fn last(&self) -> f64;
    /// return the key of the metric set
    fn name(&self) -> String;
    /// add a new value to the metric set
    fn push(&mut self, value: Number);
    /// returns true if the metric should not be rendered as a chart
    fn hidden(&self) -> bool;
    /// returns an array of the underlying data
    fn generate_data(&self) -> Vec<f64>;
    /// returns the largest member of the array
    fn max(&self) -> f64;
}

/// Represents a runtime, with all the metrics that beatperf cares about
pub struct Multistat{
    fields: Vec<StatsGroup>
}

impl<'a> IntoIterator for &'a Multistat {
    type Item = &'a StatsGroup;
    type IntoIter = slice::Iter<'a, StatsGroup>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter()
    }
}

impl IntoIterator for Multistat {
    type Item = StatsGroup;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl Display for Multistat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for group in &self.fields {
            write!(f, "{}", group)?;
        }
        Ok(())
    }
}

impl Multistat {
    /// dump the last value of every metric to stdout
    pub fn print_last_value(&self) {
        for group in &self.fields {
            group.print_last_value();
        }
    }

    /// update all metrics from a serde_json Map object
    pub fn update(&mut self, root: serde_json::Map<String, serde_json::Value>) {
        for group in &mut self.fields {
            group.update(&root);
        }
    }
}


fn mem_fields() -> StatsGroup {
    let fields: Vec<KbMetricFields> = [
        ("gc_next", false), 
        ("memory_alloc", false), 
        ("memory_sys", false), 
        ("memory_total", true), 
        ("rss", false), 
        ]
    .iter().map(|p| (format!("beat.memstats.{}", p.0), p.1).into()).collect();
    fields.into()
}

fn cpu_fields() -> StatsGroup {
    let fields: Vec<GenericMetricFields> = [
        ("system", false), 
        ("user", false), 
        ("total", false), 
        ]
    .iter().map(|p| (format!("beat.cpu.{}.time.ms", p.0), p.1).into()).collect();
    fields.into()
}

fn processdb_fields() -> StatsGroup {
    let fields: Vec<GenericMetricFields> = [
        ("entry_leader_lookup_fail", true),
        ("entry_leader_relationships_gauge", false), 
        ("entry_leaders_gauge", false), 
        ("exit_events_gauge", false), 
        ("failed_process_lookup_count", true), 
        ("processes_gauge", false), 
        ("procfs_lookup_fail", false), 
        ("reaped_orphan_exits", false), 
        ("reaped_orphan_processes", false),  
        ("reaped_processes", false), 
        ("resolved_orphan_exits", false), 
        ("served_process_count", true), 
        ]
    .iter().map(|p| (format!("processor.add_session_metadata.processdb.{}", p.0), p.1).into()).collect();

    fields.into()
}

fn custom_fields(fields: Vec<String>) -> StatsGroup {
    let mut gen_list: Vec<Box<dyn MetricField>> = Vec::new();

    for field in fields {
        let sg: GenericMetricFields = (field, false).into();
        gen_list.push(Box::new(sg));
    }

    StatsGroup::from(gen_list)
}

impl TryFrom<Cli> for Multistat {
    type Error = anyhow::Error;
    fn try_from(value: Cli) -> Result<Self, Self::Error> {
        let mut metric_types: Vec<StatsGroup> = Vec::new();
        if value.memory {
            let mut memstat = mem_fields();
            memstat.file_tag = "memstat".to_string();
            metric_types.push(memstat);
        }
        if value.cpu {
            let mut cpustat = cpu_fields();
            cpustat.file_tag = "cpu".to_string();
            metric_types.push(cpustat);
        }

        if value.processdb {
            let mut procdbstat = processdb_fields();
            procdbstat.file_tag = "processdb".to_string();
            metric_types.push(procdbstat);
        }

        if let Some(custom) = value.metrics {
            let mut custom_stats = custom_fields(custom);
            custom_stats.file_tag = "metrics".to_string();
            metric_types.push(custom_stats);
        }

        if metric_types.is_empty() {
            Err(anyhow::anyhow!("no metrics configured!"))
        } else {
            Ok(Multistat{fields: metric_types})
        }
        
    }
}


/// Represents a group of metrics that map to a single chart.
pub struct StatsGroup{
    fields: Vec<Box<dyn MetricField>>,
    updates: usize,
    /// When we write out this to charts_rs, we want to separate out different metrics to different files
    pub file_tag: String
}


impl From<Vec<Box<dyn MetricField>>> for StatsGroup {
    fn from(value: Vec<Box<dyn MetricField>>) -> Self {
        StatsGroup{fields: value, updates: 0, file_tag: String::new()}
    }
}

impl Display for StatsGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.file_tag)?;
        for field in &self.fields {
            writeln!(f, "\t{}", field.name())?;
        };
        Ok(())
    }
}

impl StatsGroup {
    pub fn print_last_value(&self) {
        for val in &self.fields {
            println!("\t{}: {}", val.name(), val.last());
        }
    }

    pub fn update(&mut self, root: &serde_json::Map<String, serde_json::Value>)  {
        self.updates+=1;

        for paths in &mut self.fields {
            let new_data = get_root_elem(root, &paths.name());
            match new_data {
                Some(serde_json::Value::Number(val)) => {
                    paths.push(val.clone());
                }, 
                None => {
                    debug!("key {} does not exist", paths.name());
                },
                _ => {
                    error!("key {} is not a number!", paths.name());
                }
            };
        };
    }

    pub fn len(&self) -> usize {
        self.updates
    }

    pub fn range_max(&self) -> f64 {
        self.fields.iter().map(|g| g.max()).reduce(f64::max).unwrap_or_default()
    }

    pub fn plot(&self) -> HashMap<String, Vec<f64>> {
        let mut map: HashMap<String, Vec<f64>> = HashMap::new();
        for dat in &self.fields  {
            if !dat.hidden() {
                map.insert(dat.name(), dat.generate_data());
            }
        };
        map
    }
}

/// simple recursive algo to fetch the the value from a hashmap when our key.is.formatted.like.this
fn get_root_elem<'a>(data: &'a serde_json::Map<String, serde_json::Value>, nested_key: &str) -> Option<&'a serde_json::Value> {
    let mut key_list: VecDeque<String> = nested_key.split(".").map(|e| e.to_string()).collect();

    if key_list.len() == 1 {
        data.get(&key_list[0])
    } else {
        let child_key = key_list.pop_front().unwrap();
        let child = data.get(&child_key)?;
        match child {
            serde_json::Value::Object(val) => {
                let merged = key_list.into_iter().reduce(|acc, e| format!("{}.{}", acc, e))?;
                get_root_elem(val, &merged)
            }
            _ => {
                None
            }
        }
    }
    
}


#[cfg(test)]
mod tests {
    use crate::fields::GenericMetricFields;

    use super::StatsGroup;

    fn create_nested_json(val_l3: u64, val_l2: u64) -> String {
        let json = format!(r#"{{
        "root": {{
                "l1" : {{
                    "l2": {{
                        "l3" : {{
                            "metric": {}
                        }},
                        "metric": {}
                    }}
                }}
            }}
        }}"#, val_l3, val_l2);
        //println!("{}", json);
        json

    }

    #[test]
    fn test_nested_key() -> anyhow::Result<()> {
        let result1: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&create_nested_json(42, 0))?;
        let result2: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&create_nested_json(63, 0))?;

        let field: Vec<GenericMetricFields> = vec![("root.l1.l2.l3.metric", false).into()];
        let mut stats = StatsGroup::from(field);
        stats.update(&result1);
        stats.update(&result2);

        println!("raw: {}", stats);
        let test_ser = stats.plot();

        assert_eq!(test_ser["root.l1.l2.l3.metric"], vec![42.0, 63.0]);

        Ok(())
    }

    #[test]
    fn test_hidden_metrics() -> anyhow::Result<()>{
        let result1: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&create_nested_json(42, 21))?;
        let field: Vec<GenericMetricFields> = vec![("root.l1.l2.l3.metric", true).into(), ("root.l1.l2.metric", false).into()];
        let mut stats = StatsGroup::from(field);
        stats.update(&result1);

        println!("raw: {}", stats);
        let test_ser = stats.plot();

        assert_eq!(test_ser["root.l1.l2.metric"], vec![21.0]);

        Ok(())
    }

}
