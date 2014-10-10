use super::Payload;

pub use self::tcp::TCPInput;

use std;

pub trait Input {
    fn run(&self, tx: Sender<Payload>);

    fn typename(&self) -> &'static str {
        unsafe { (*std::intrinsics::get_tydesc::<Self>()).name }
    }
}

