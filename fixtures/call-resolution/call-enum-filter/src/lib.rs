// compile-valid: enum variant constructors are valid Rust
fn helper() -> i32 {
    1
}

fn test_fn() -> Result<i32, &'static str> {
    let x = Some(42);
    let y: Result<i32, &str> = Ok(helper());
    let z: Result<i32, &str> = Err("error");
    helper();
    Ok(x.unwrap())
}
