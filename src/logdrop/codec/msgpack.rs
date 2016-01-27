use std::convert::From;
use std::collections::HashMap;
use std::io::Read;

use msgpack::decode::value::{Integer, Value};
use msgpack::decode::value::read_value;

use super::Codec;
use super::super::{Record, RecordItem};

#[derive(Clone)]
pub struct MessagePack;

pub struct Iter {
    rd: Box<Read>,
}

impl Iter {
    pub fn new(rd: Box<Read>) -> Iter {
        Iter {
            rd: rd,
        }
    }
}

impl From<Value> for Record {
    fn from(v: Value) -> Record {
        match v {
            Value::Map(map) => {
                let mut res = HashMap::new();
                for (key, val) in map {
                    let key = match key {
                        Value::String(v) => v,
                        _ => unimplemented!(),
                    };

                    let val = From::from(val);

                    res.insert(key, val);
                }

                Record(res)
            }
            _ => unimplemented!(),
        }
    }
}

impl From<Value> for RecordItem {
    fn from(v: Value) -> RecordItem {
        match v {
            Value::Integer(Integer::I64(v)) => RecordItem::F64(v as f64),
            Value::Integer(Integer::U64(v)) => RecordItem::F64(v as f64),
            Value::String(v) => RecordItem::String(v),
            Value::Map(v) => {
                let mut res = HashMap::new();
                for (k, v) in v {
                    let k = match k {
                        Value::String(v) => v,
                        _ => unimplemented!(),
                    };

                    let v = From::from(v);

                    res.insert(k, v);
                }
                RecordItem::Object(res)
            }
            _ => unimplemented!(),
        }
    }
}

impl Iterator for Iter {
    type Item = Record;

    fn next(&mut self) -> Option<Record> {
        let val = read_value(&mut self.rd).unwrap();

        Some(From::from(val))
    }
}

impl Codec for MessagePack {
    fn new(&self) -> Box<Codec> {
        Box::new(self.clone())
    }

    fn decode(&self, rd: Box<Read>) -> Box<Iterator<Item=Record>> {
        Box::new(Iter::new(rd))
    }
}
