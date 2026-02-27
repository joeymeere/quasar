use quasar_core::prelude::*;

#[event(discriminator = [1])]
pub struct Bad {
    pub x: Vec<u8>,
}

fn main() {}
