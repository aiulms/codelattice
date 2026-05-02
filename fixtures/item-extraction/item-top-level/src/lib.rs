pub fn hello() -> &'static str { "hello" }

pub struct User {
    name: String,
}

pub enum Color {
    Red,
    Green,
    Blue,
}

pub trait Drawable {
    fn draw(&self);
}

pub type Point = (f64, f64);

pub const MAX_SIZE: usize = 100;

pub static VERSION: &str = "1.0";

macro_rules! say_hello {
    () => { println!("hello") };
}

fn private_helper() {}

struct InternalStruct {}
