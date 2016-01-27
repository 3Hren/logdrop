use std;
use std::sync::mpsc::Sender;

use super::codec::Codec;
use super::Record;

pub trait Input : Sync + Send {
    fn run(&self, tx: Sender<Record>, codec: Box<Codec>);

    fn typename(&self) -> &'static str {
        unsafe { std::intrinsics::type_name::<Self>() }
    }
}

mod tcp;

pub use self::tcp::TcpInput;
