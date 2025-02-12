// helpers for generating the metrics paths, so we can avoid a bunch of copy-and-pasting

use std::fmt::Display;

use serde_json::Number;

use crate::stat::{MetricField, StatsGroup};



/// Represent a metric that we must convert from bytes to kb
#[derive(Default, Clone, Debug)]
pub struct KbMetricFields {
    pub key: String,
    pub data: Vec<f64>,
    pub hidden: bool
}

impl From<String> for KbMetricFields {
    fn from(value: String) -> Self {
        KbMetricFields{key: value, ..Default::default()}
    }
}

impl Display for KbMetricFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl From<Vec<KbMetricFields>> for StatsGroup {
    fn from(value: Vec<KbMetricFields>) -> Self {
        let mut gen_list: Vec<Box<dyn MetricField>> = Vec::new();

        for field in value {
            gen_list.push(Box::new(field));
        }
    
        StatsGroup::from(gen_list)
    }
}

impl From<(String, bool)> for KbMetricFields {
    fn from(value: (String, bool)) -> Self {
        KbMetricFields { key: value.0, hidden: value.1, ..Default::default() }
    }
}

impl MetricField for KbMetricFields {
    fn last(&self) -> f64 {
        *self.data.last().unwrap_or(&0.0)
    }
    fn name(&self) -> String {
        self.key.clone()
    }
    fn push(&mut self, value: Number) {
        let num =  value.as_f64().unwrap_or_default();
        self.data.push(num);
    }

    fn hidden(&self) -> bool {
        self.hidden
    }

    fn generate_data(&self) -> Vec<f64> {
        self.data.clone()
    }

    fn max(&self) -> f64 {
        self.data.iter().copied().reduce(f64::max).unwrap_or_default()
    }
}

/// Represents a single metric, a generic implementation of MetricField
#[derive(Default, Clone, Debug)]
pub struct GenericMetricFields {
    pub key: String,
    pub data: Vec<serde_json::Number>,
    pub hidden: bool
}

impl <T: AsRef<str>>From<(T, bool)> for GenericMetricFields {
    fn from(value: (T, bool)) -> Self {
        GenericMetricFields { key: value.0.as_ref().to_string(), hidden: value.1, ..Default::default() }
    }
}

impl Display for GenericMetricFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl From<Vec<GenericMetricFields>> for StatsGroup {
    fn from(value: Vec<GenericMetricFields>) -> Self {
        let mut gen_list: Vec<Box<dyn MetricField>> = Vec::new();

        for field in value {
            gen_list.push(Box::new(field));
        }
    
        StatsGroup::from(gen_list)
    }
}

impl MetricField for GenericMetricFields {
    fn last(&self) -> f64 {
        match self.data.last(){
            Some(val) => {
                val.as_f64().unwrap_or_default()
            }, 
            None => {
                0.0
            }
        }
    }

    fn name(&self) -> String {
        self.key.clone()
    }
    fn push(&mut self, value: Number) {
        self.data.push(value);
    }
    fn hidden(&self) -> bool {
        self.hidden
    }

    fn generate_data(&self) -> Vec<f64> {
        let acc: Vec<f64> = self.data.iter().map(|f| (f.as_f64().unwrap_or_default()) / 1000.0).collect();
        acc
    }

    fn max(&self) -> f64 {
        self.data.iter().map(|d| d.as_f64().unwrap_or_default()).reduce(f64::max).unwrap_or_default()
    }

}