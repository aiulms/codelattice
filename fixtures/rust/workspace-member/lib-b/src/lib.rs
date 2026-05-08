// lib-b: 依赖 workspace 成员 lib-a
// compile-valid

use lib_a::{greet, Point, Status};

/// 调用 workspace 成员 lib-a 的函数
pub fn welcome() -> String {
    greet("World")
}

/// 使用 workspace 成员的 struct
pub fn make_point() -> Point {
    Point::new(3.0, 4.0)
}

/// 使用 workspace 成员的 enum variant
pub fn active_status() -> Status {
    Status::Active
}

/// 跨 crate 级联调用
pub fn welcome_length() -> usize {
    welcome().len()
}
