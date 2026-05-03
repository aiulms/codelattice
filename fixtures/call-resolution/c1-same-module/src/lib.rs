pub fn helper() -> i32 {
    42
}

pub fn main_fn() {
    let x = helper();
    let y = helper();
    println!("{}", x + y);
}
