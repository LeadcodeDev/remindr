#![expect(unused_variables, dead_code)]

pub mod application;
pub mod domain;
pub mod ports;
#[cfg(feature = "test-support")]
pub mod testing;
