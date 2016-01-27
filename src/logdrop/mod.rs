use std::collections::HashMap;

pub mod logging;

pub mod input;
pub mod codec;
pub mod output;

mod json;

#[derive(Debug, Clone)]
pub struct Record(HashMap<String, RecordItem>);

#[derive(Debug, Clone)]
pub enum RecordItem {
    Null,
    Bool(bool),
    F64(f64),
    String(String),
    Array(Vec<RecordItem>),
    Object(HashMap<String, RecordItem>),
}

impl Record {
    pub fn find(&self, name: &str) -> Option<&RecordItem> {
        self.0.get(name)
    }
}
