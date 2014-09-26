use super::Payload;

pub use self::tcp::TCPInput;

pub trait Input {
    fn run(&self, tx: Sender<Payload>);
}

mod tcp;
