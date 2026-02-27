use quasar_core::prelude::*;

#[event(discriminator = [1])]
pub struct Bad {
    pub x: String,
}

fn main() {}
