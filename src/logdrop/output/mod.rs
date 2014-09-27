use super::Payload;

pub use self::files::FileOutput;
pub use self::elasticsearch::ElasticsearchOutput;

pub trait Output {
    fn name(&self) -> &'static str;
    fn feed(&mut self, payload: &Payload);
}

mod files;
mod elasticsearch;

