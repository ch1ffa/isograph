use std::io;

use common::owned::{evaluate_input, Input};
use pico::database::Database;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

mod calc;
mod common;

fn main() {
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();

    let mut db = Database::new();
    let key = "expr1";

    let mut input = Input {
        value: "2 + 2 * 2".to_string(),
    };
    input.set(&mut db, key);

    let mut result = evaluate_input(&mut db, key);
    info!("result: {result}");
    debug!("{db:#?}");

    input = Input {
        value: "3 * 2".to_string(),
    };
    input.set(&mut db, key);

    result = evaluate_input(&mut db, key);
    info!("result: {result}");
    debug!("{db:#?}");
}
