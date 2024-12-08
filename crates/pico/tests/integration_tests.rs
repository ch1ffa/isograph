use common::owned::{evaluate_input, Input};
use pico::database::Database;

mod calc;
mod common;

#[test]
fn returns_result() {
    let mut db = Database::new();
    let key = "test_expr";

    Input::set(&mut db, key, "2 + 2 * 2".to_string());
    let mut result = evaluate_input(&mut db, key);
    assert_eq!(result, Ok(6));

    Input::set(&mut db, key, "(2 + 2) * 2".to_string());
    result = evaluate_input(&mut db, key);
    assert_eq!(result, Ok(8));
}
