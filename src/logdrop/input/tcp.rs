use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use super::Input;
use super::super::Record;
use super::super::codec::Codec;
use super::super::json::Builder;

pub struct TcpInput {
    host: String,
    port: u16,
}

impl TcpInput {
    pub fn new(host: String, port: u16) -> TcpInput {
        TcpInput {
            host: host,
            port: port
        }
    }

    fn serve(stream: TcpStream, tx: Sender<Record>, codec: Box<Codec>) {
        debug!(target: "Input::TCP", "connection accepted from {}", stream.peer_addr().unwrap());

        let rd = BufReader::new(stream);
        let mut codec = codec.decode(Box::new(rd));
//        let mut codec = Builder::new(rd.chars().map(|x| x.unwrap()));


        for record in codec {
            tx.send(record).unwrap();
        }

        debug!(target: "Input::TCP", "stopped serving TCP connection");
    }
}

impl Input for TcpInput {
    fn run(&self, tx: Sender<Record>, codec: Box<Codec>) {
        info!(target: "Input::TCP", "running TCP listener at [{}]:{}", self.host, self.port);

        let host: &str = &self.host;

        match TcpListener::bind((host, self.port)) {
            Ok(listener) => {
                for stream in listener.incoming() {
                    match stream {
                        Ok(stream) => {
                            let tx = tx.clone();
                            let codec = codec.new();
                            thread::spawn(move || TcpInput::serve(stream, tx, codec));
                        },
                        Err(err) => {
                            warn!(target: "Input::TCP", "error occured while accepting connection: {}", err);
                        }
                    }
                }
            },
            Err(err) => {
                error!(target: "Input::TCP", "unable to bind: {}", err);
            }
        }

        info!(target: "Input::TCP", "TCP listener has been stopped");
    }
}
