// lib-a: 被 workspace 其他成员依赖的库
// compile-valid

/// 公开函数（避免 macro 以免产生 unsupported-macro-expansion diagnostic）
pub fn greet(name: &str) -> String {
    let mut s = String::from("Hello, ");
    s.push_str(name);
    s.push('!');
    s
}

/// 公开结构体
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    pub fn distance(&self) -> f64 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }
}

/// 公开枚举
pub enum Status {
    Active,
    Inactive,
    Pending,
}
