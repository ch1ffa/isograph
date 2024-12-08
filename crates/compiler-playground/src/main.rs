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

    Input::set(&mut db, key, "2 + 2 * 2".to_string());

    if let Ok(result) = evaluate_input(&mut db, key) {
        info!("result: {result}");
    }
    debug!("{db:#?}");

    Input::set(&mut db, key, "3 * 2".to_string());

    if let Ok(result) = evaluate_input(&mut db, key) {
        info!("result: {result}");
    }
    debug!("{db:#?}");
}
