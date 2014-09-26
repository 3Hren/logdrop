#![experimental]

use std::fmt;
use std::fmt::{Show, Formatter};

pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl ToPrimitive for Level {
    fn to_i64(&self) -> Option<i64> {
        match *self {
            Debug => Some(0),
            Info  => Some(1),
            Warn  => Some(2),
            Error => Some(3)
        }
    }

    fn to_u64(&self) -> Option<u64> {
        return Some(self.to_i64().unwrap() as u64);
    }
}

impl Show for Level {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let reason = match *self {
            Debug => "D",
            Info  => "I",
            Warn  => "W",
            Error => "E"
        };
        reason.fmt(f)
    }
}
