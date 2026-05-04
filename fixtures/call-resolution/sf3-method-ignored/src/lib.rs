struct Config { value: i32 }
impl Config {
    fn get_value(&self) -> i32 { self.value }
}
fn main_fn() -> i32 {
    let c = Config { value: 42 };
    c.get_value()
}
