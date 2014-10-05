use super::Payload;

pub use self::tcp::TCPInput;

pub trait Input {
    fn run(&self, tx: Sender<Payload>);
}

mod tcp;

mod files {

// Accept [Path].
// Okay with *.
// MVP: 1 file
// V2. 1 dir with changes
// V3. multiple dirs.
struct FilesInput {

}

}
