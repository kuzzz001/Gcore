//! A set of drivers that should be work in both OS and user space.

#![no_std]
#![allow(unused_variables, dead_code, invalid_value)]

extern crate alloc;
#[cfg(feature = "log")]
#[macro_use]
extern crate log;

#[macro_use]
mod logging;

pub mod block;
pub mod net;
pub mod provider;
