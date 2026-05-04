// compile-valid: enum variant constructors are valid Rust
fn helper() -> i32 {
    1
}

fn test_fn() -> Result<i32, &'static str> {
    let x = Some(42);
    let y = Ok(helper());
    let z = Err("error");
    helper();
    Ok(x.unwrap())
}
