#![no_std]
#[macro_use]
pub mod macros;
pub mod traits;
pub mod checks;
pub mod accounts;
pub mod programs;
pub mod context;
pub mod prelude;

// Example program ID for the example module
prelude::declare_id!("22222222222222222222222222222222222222222222");
pub mod example;