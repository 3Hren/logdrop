#![feature(macro_rules)]
#![feature(phase)]

extern crate collections;
extern crate serialize;
extern crate time;
extern crate url;
extern crate http;
extern crate bigbrother;

use serialize::json::{String, Object};

use logdrop::Payload;
use logdrop::logger::{Debug, Info, Warn};
use logdrop::input::{Input, TCPInput};
use logdrop::output::{Output, FileOutput, ElasticsearchOutput};

mod logdrop;

// Input - event driven entity, that reads something as bytes.
//  - socket
//  - file
//  - stdin
//  - ...

// Decoder - converter from bytes to json.
//  - json:  byte -> json
//  - plain: byte -> textline -> json
//  - ...

// Pipeline - thing, that consumes json and process with its fields.
//  - add timestamp, if it isn't present
//  - check for message field, fail if not (why?)
//  - ...

// Output - thing, that consumes json with fields and writes it into some sink.
// Guaranteed, that event contains some required fields, like message or timestamp.
//  - file
//  - elasticsearch
//  - ...

#[deriving(Show)]
pub enum ProcessorError {
    NotFound,
}

trait Processor {
    fn contains(&self, key: &str) -> bool;
//    fn timestamp(&mut self);
}

impl Processor for Payload {
    fn contains(&self, key: &str) -> bool {
        let key = String::from_str(key);
        match self.find(&key) {
            Some(_) => true,
            None => false
        }
    }
}

fn run(inputs: Vec<Box<Input + Send>>, outputs: Vec<Box<Output + Send>>) {
    let (tx, rx) = channel();

    for input in inputs.into_iter() {
        log!(Info, "Main" -> "starting '{}' input", input.typename());
        let tx = tx.clone();
        spawn(proc() {
            input.run(tx)
        });
    }

    let channels: Vec<Sender<Payload>> = outputs.into_iter().map(|output| {
        let(tx, rx) = channel();
        spawn(proc() {
            log!(Info, "Main" -> "starting '{}' output", output.typename());
            let mut output = output;
            loop {
                output.feed(&rx.recv());
            }
        });
        tx
    }).collect();

    loop {
        log!(Debug, "Main" -> "waiting for new data ...");
        let mut payload = rx.recv();
        if !payload.contains("message") {
            log!(Warn, "Main" -> "dropping '{}': message field required", payload);
            continue;
        }

        match payload {
            Object(ref mut object) => {
                let now = time::now();
                let timestamp = time::strftime("%Y-%m-%d %H:%M:%S", &now);
                object.insert("timestamp".to_string(), String(timestamp));
            }
            _ => { unreachable!(); }
        }

        for tx in channels.iter() {
            tx.send(payload.clone());
        }
    }
}

fn main() {
    let es = box ElasticsearchOutput::new("localhost", 9200) as Box<Output + Send>;
    let inputs = vec![
        box TCPInput::new("::", 10053) as Box<Input + Send>,
    ];

    let outputs = vec![
        box FileOutput::new("/tmp/{parent/child}-{source}-logdrop.log", "[{timestamp}]: {message}") as Box<Output + Send>,
//        box ElasticsearchOutput::new("localhost", 9200) as Box<Output + Send>,
    ];
    run(inputs, outputs);
}
