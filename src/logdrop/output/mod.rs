use std;

use super::Record;

pub trait Output : Sync + Send {
    fn feed(&mut self, payload: &Record);

    fn typename(&self) -> &'static str {
        unsafe { std::intrinsics::type_name::<Self>() }
    }
}

mod null;
//mod files;

//pub use self::files::FileOutput;
pub use self::null::Null;
