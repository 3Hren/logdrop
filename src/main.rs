#![feature(core, convert, io, path_ext, test)]

#[macro_use]
extern crate log;
extern crate libc;
extern crate chrono;
extern crate rmp as msgpack;

use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::thread;

use log::LogLevel;

use logdrop::codec;
use logdrop::codec::Codec;
use logdrop::input::{Input, TcpInput};
use logdrop::logging;
use logdrop::output::{Output, Null};
use logdrop::Record;

mod logdrop;

fn run(inputs: Vec<(Box<Input>, Box<Codec>)>, outputs: Vec<Box<Output>>) {
    let (tx, rx) = channel();

    for (input, codec) in inputs.into_iter() {
        trace!(target: "Main", "starting '{}' input", input.typename());

        let tx = tx.clone();
        thread::spawn(move || {
            input.run(tx, codec)
        });
    }

    let channels: Vec<Sender<Record>> = outputs.into_iter().map(|mut output| {
        let(tx, rx) = channel();
        thread::spawn(move || {
            trace!(target: "Main", "starting '{}' output", output.typename());

            loop {
                output.feed(&rx.recv().unwrap());
            }
        });

        tx
    }).collect();

    loop {
        debug!(target: "Main", "waiting for new data ...");

        let mut value = rx.recv().unwrap();
        trace!(target: "Main", "processing {:?}", value);

        if value.find("message").is_none() {
            warn!(target: "Main", "dropping '{:?}': message field required", value);
            continue;
        }

//        match value {
//            Value::Object(ref mut object) => {
//                let now = chrono::Local::now();
//                object.insert("timestamp".to_string(), Value::String(format!("{}", now)));
//            }
//            _ => { unimplemented!() }
//        }

        for tx in channels.iter() {
            tx.send(value.clone()).unwrap();
        }
    }
}

fn main() {
    use logdrop::codec::Codec;

    logging::init(LogLevel::Info).ok().expect("unable to initialize logging system");

    let inputs: Vec<(Box<Input>, Box<Codec>)> = vec![
        (Box::new(TcpInput::new("::".to_string(), 10053)), Box::new(codec::MessagePack)),
    ];

    let outputs: Vec<Box<Output>> = vec![
        Box::new(Null)
//        Box::new(FileOutput::new("/tmp/{parent/child}-{source}-logdrop.log", "[{timestamp}]: {message}")) as Box<Output + Sync +Send>,
//        box ElasticsearchOutput::new("localhost", 9200) as Box<Output + Send>,
    ];
    run(inputs, outputs);
}
