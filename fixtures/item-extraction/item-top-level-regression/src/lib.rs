pub fn hello() {}
pub struct User {}
pub enum Color { Red, Green, Blue }
pub trait Drawable { fn draw(&self); }
pub type Point = (f64, f64);
pub const MAX: usize = 100;
pub static VERSION: &str = "1.0";
macro_rules! create_fn { ($name:ident) => { fn $name() {} }; }
fn private_helper() {}

