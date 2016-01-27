use super::super::Record;
use super::Output;

pub struct Null;

impl Output for Null {
    fn feed(&mut self, _: &Record) {}
}
