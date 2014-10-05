#![feature(phase)]

extern crate libc;
extern crate sync;

#[phase(plugin, link)] extern crate log;

pub mod fsevent;
