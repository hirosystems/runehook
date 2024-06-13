#[macro_use]
extern crate hiro_system_kit;

#[macro_use]
extern crate serde_derive;

extern crate serde;

pub mod cli;
pub mod config;
pub mod db;
pub mod scan;
pub mod service;

// #[tokio::main]
fn main() {
    cli::main();
}
