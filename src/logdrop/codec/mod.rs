use std::io::Read;

use super::Record;

pub trait Codec: Sync + Send {
    fn new(&self) -> Box<Codec>;
    fn decode(&self, rd: Box<Read>) -> Box<Iterator<Item=Record>>;
}

mod msgpack;

pub use self::msgpack::MessagePack;

