use std::io::{Acceptor, Listener, BufferedReader};
use std::io::net::tcp::{TcpListener, TcpAcceptor, TcpStream};

use time;

use logdrop::Payload;
use logdrop::json::Builder;
use logdrop::logger::{Debug, Info, Warn};

use super::Input;

pub struct TCPInput {
    host: String,
    port: u16,
}

impl TCPInput {
    pub fn new(host: &str, port: u16) -> TCPInput {
        TCPInput {
            host: host.to_string(),
            port: port
        }
    }

    fn accept(mut acceptor: TcpAcceptor, tx: Sender<Payload>) {
        for stream in acceptor.incoming() {
            match stream {
                Ok(stream) => {
                    let tx = tx.clone();
                    spawn(proc() TCPInput::serve(stream, tx));
                },
                Err(err) => {
                    log!(Warn, "Input::TCP" -> "error occured while accepting connection: {}", err);
                }
            }
        }
        drop(acceptor);
    }

    fn serve(stream: TcpStream, tx: Sender<Payload>) {
        let mut stream = stream;
        log!(Debug, "Input::TCP" -> "connection accepted from {}", stream.peer_name().unwrap());

        let mut reader = BufferedReader::new(stream);
        let mut builder = Builder::new(reader.chars().map(|x| x.unwrap()));
        loop {
            let payload = match builder.next() {
                Some(v) => v,
                None => break
            };
            tx.send(payload);
        }

        log!(Debug, "Input::TCP" -> "stopped serving tcp input");
    }
}

impl Input for TCPInput {
    fn run(&self, tx: Sender<Payload>) {
        log!(Info, "Input::TCP" -> "starting tcp listener at [{}]:{}", self.host, self.port);
        let listener = TcpListener::bind(self.host.as_slice(), self.port);

        let acceptor = listener.listen().unwrap();
        spawn(proc() TCPInput::accept(acceptor, tx));
    }
}
