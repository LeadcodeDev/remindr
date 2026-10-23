#![expect(unused_variables, dead_code)]

pub mod application;
pub mod domain;
pub mod ports;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
