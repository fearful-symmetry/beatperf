/*!
 * Generic handles a group of metrics, which must all be of a single numerical type.
 * This type is usually wrapped by other groups, and not used directly.
 * 
 */

use std::{collections::{HashMap, VecDeque}, marker::PhantomData};

use serde::de::DeserializeOwned;
use serde_json::Number;
use tracing::{debug, error};

/// A processor provides a way for a user of the Generic type to "preprocess"
/// metrics before they are ingested, for example, converting bytes to kb.
/// `NoOpProcess` is provided for users who do not require processing
pub trait Processor {
    /// The expected input type, usually f64 or u64
    type InValue;
    /// The type after `process()`. Must match the numerical type of the `Generic` instance
    type OutValue;
    fn new() -> Self;
    /// Process the metric
    fn process(&self, raw: Self::InValue) -> Self::OutValue;
}

/// Do not process the metric before its ingested
pub struct NoOpProcess<T>{
    data_type: PhantomData<T>
}

impl<T> Processor for NoOpProcess<T>{
    type InValue = T;
    type OutValue = Self::InValue;
    fn new() -> Self {
        Self{data_type: PhantomData}
    }
    fn process(&self, raw: Self::InValue) -> Self::OutValue {
        raw
    }
}

/// An individual metric field. We use this as we don't actually need a hashmap.
struct MetricField<T: Clone > {
    key: String,
    values: Vec<T>
}

/// A grouping of metrics of a single type.
 pub struct Generic<T: Clone + DeserializeOwned, Proc: Processor> {
    user_key: Vec<String>,
    // data is lazily instantiated, as we can't verify the type until we get a json event
    data: Vec<MetricField<T>>,
    datapoints: usize, 
    processor: Proc
}

impl<F, T, P, I> From<Vec<F>> for Generic<T, P>
where 
    F: ToString,
    T: Clone +  DeserializeOwned,
    I:  Clone +DeserializeOwned,
    P: Processor<InValue = I, OutValue = T>
{
    fn from(value: Vec<F>) -> Self {
        Generic::new(value.iter().map(|v|v.to_string()).collect(), P::new() )
    }
}


impl<T, Proc, I> Generic<T, Proc>
where
    T: Clone +DeserializeOwned,
    I: Clone +DeserializeOwned,
    Proc: Processor<InValue = I, OutValue = T>
{
    /// Create a new generic from a given group of metrics in dot notation and a processor.
    /// The elements of a group can either point to a list of individual metrics, or a map that `Generic`
    /// can reduce down to a list. 
    /// ```
    /// // A single event, which will be of type u64
    /// let new: Generic<u64, NoOpProcess<u64>> = Generic::from(vec![".beat.runtime.goroutines"]);
    /// 
    /// // A group of metrics, which will all be of type u64
    /// let new: Generic<u64, NoOpProcess<u64>> = Generic::from(vec![".beat.runtime"]);
    /// ```
    /// 
    /// All the metrics must be of type `T`, while `I` is the type as seen in the raw json event.
    /// The internal list of metrics is lazily instantiated, and all the internal types and fields will not be resolved until the first `update()`.
    pub fn new(group: Vec<String>, processor: Proc) -> Generic<T, Proc> {
        Generic { user_key: group, data: Vec::new(), datapoints: 0 , processor}
    }

    /// Update the metrics
    pub fn update(&mut self, root: &serde_json::Map<String, serde_json::Value>)  {
        // lazily initialize the vectors
        if self.data.is_empty() {
            self.init_metrics(root);
        }

        for metric in &mut self.data {
            let new_data = get_root_elem(root, &metric.key);
            match new_data {
                Some(val) => {
                    let raw: I = match serde_json::from_value(val.clone()){
                        Ok(v) => v,
                        Err(e) => {
                            error!("could not report {}, got unexpected type: {}", metric.key, e);
                            continue;
                        } 
                    };
                    metric.values.push(self.processor.process(raw));
                },
                None => {
                    debug!("key {} does not exist", metric.key);
                }
            }
        }
        self.datapoints+=1;

    }

    /// Turn our metrics into a hashmap
    pub fn plot(&self) -> HashMap<String, Vec<T>> {
        let mut acc: HashMap<String, Vec<T>> = HashMap::new();
        for points in &self.data{
            acc.insert(points.key.to_string(), points.values.clone());
        }
        acc
    }

    /// The total number of datapoints
    pub fn datapoints(&self) -> usize {
        self.datapoints
    }

    /// This is a little cursed, but it exists to deal with all the cases we can run into when we try to turn a bunch of 
    /// metrics in.dot.form into a 2D vector of values
    fn init_metrics(&mut self, root: &serde_json::Map<String, serde_json::Value>) {
        for metric_field in &self.user_key {
            let new_data = get_root_elem(root, metric_field);

            let mut raw_fields: Vec<(String, Number)> = Vec::new();

            match new_data {
                // user has given us a value that maps to a single number value
                Some(serde_json::Value::Number(val)) => {
                    raw_fields.push((metric_field.to_string(), val.clone()));
                }
                // user has given us a value that maps to a map with multiple values, recusively find all of them.
                Some(serde_json::Value::Object(inner)) => {
                    // now we have a giant map we need to flatten
                    let flat_values = flatten_map(inner);
                    for (inner_key, inner_val) in flat_values {
                        let root_key = format!("{}.{}", metric_field, inner_key);
                        raw_fields.push((root_key, inner_val));
                    }
                },
                _ => {
                    error!("key {} is not a number!", metric_field);
                }
            }

            // we now have an array of every key that comes from the user-supplied string. 
            // validate each against our generic type
            for (field_key, field_val) in raw_fields {
                    let raw: I = match serde_json::from_value(serde_json::Value::Number(field_val)){
                    Ok(v) => {
                        debug!("got value for key {}", field_key);
                        v
                    },
                    Err(e) => {
                        error!("could not add metric {} to monitor, got unexpected type: {}", metric_field, e);
                        continue;
                    } 
                };
                self.data.push(MetricField { key: field_key, values: vec![self.processor.process(raw)] });
            }
            
        }

    }

}

/// Flatten a map into a vector of dot-notated keys
fn flatten_map(data: &serde_json::Map<String, serde_json::Value>) -> Vec<(String, Number)> {
    let mut acc: Vec<(String, Number)> = Vec::new();

    for (key, val) in data {

        match val { 
            serde_json::Value::Number(found_num) => {
                acc.push((key.to_string(), found_num.clone()));
            },
            serde_json::Value::Object(nested) => {
                let inner = flatten_map(nested);
                acc.extend(inner.into_iter().map(|(k,v)| (format!("{}.{}", key, k), v)));
            },
            _ => {
                debug!("skipping {}", key);
            }
        }
    }

    acc
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
mod test {
    use std::collections::HashMap;

    use serde_json::Number;
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::EnvFilter;

    use crate::groups::generic::{Generic, NoOpProcess};

    use super::flatten_map;

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
    fn test_flatten() -> anyhow::Result<()> {
        let data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&create_nested_json(42, 45))?;

        let res = flatten_map(&data);
        assert_eq!(res, vec![("root.l1.l2.l3.metric".to_string(), Number::from(42)), ("root.l1.l2.metric".to_string(), Number::from(45))]);

        Ok(())
    }

    #[test]
    fn test_submap_generic() -> anyhow::Result<()> {    
        tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().with_default_directive(LevelFilter::DEBUG.into()).from_env_lossy()) 
        .init();
    
        let result1: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&create_nested_json(42, 5))?;
        let result2: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&create_nested_json(63, 8))?;

        
        let mut stats: Generic<u64, NoOpProcess<_>> = Generic::from(vec!["root.l1.l2"]);
        stats.update(&result1);
        stats.update(&result1);
        stats.update(&result2);

        let golden = HashMap::from([("root.l1.l2.metric".to_string(), vec![5u64, 5, 8]), ("root.l1.l2.l3.metric".to_string(), vec![42, 42, 63])]);
        assert_eq!(golden, stats.plot());
        



        Ok(())
    }
}