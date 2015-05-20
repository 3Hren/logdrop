use super::Payload;

use std;

pub use self::files::FileOutput;
pub use self::elasticsearch::ElasticsearchOutput;

pub trait Output {
    fn feed(&mut self, payload: &Payload);

    fn typename(&self) -> &'static str {
        unsafe { (*std::intrinsics::get_tydesc::<Self>()).name }
    }
}

mod files;
//mod elasticsearch;

