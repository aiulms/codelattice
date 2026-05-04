mod a {
    pub fn process() -> i32 { 1 }
}
mod b {
    pub fn process() -> i32 { 2 }
}
fn main_fn() -> i32 {
    process()
}
