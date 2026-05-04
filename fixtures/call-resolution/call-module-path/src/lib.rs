pub mod math {
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }
}

pub mod text {
    pub fn greet(name: &str) -> String {
        format!("Hello, {}", name)
    }
}

pub fn compute() -> i32 {
    math::add(1, 2)
}

pub fn hello() -> String {
    text::greet("world")
}
