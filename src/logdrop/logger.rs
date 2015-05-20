#![experimental]

use std::fmt;
use std::fmt::{Debug, Formatter};

pub enum Level {
    Debug,
    Info,
    Warn,
}

impl ToPrimitive for Level {
    fn to_i64(&self) -> Option<i64> {
        match *self {
            Debug => Some(0),
            Info  => Some(1),
            Warn  => Some(2),
        }
    }

    fn to_u64(&self) -> Option<u64> {
        return Some(self.to_i64().unwrap() as u64);
    }
}

impl Debug for Level {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let reason = match *self {
            Debug => "D",
            Info  => "I",
            Warn  => "W",
        };
        reason.fmt(f)
    }
}
