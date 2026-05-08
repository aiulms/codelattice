// enum-variant fixture: 测试 enum variant 符号提取和调用解析
// compile-valid
// 覆盖：简单 variant、元组 variant、结构体 variant、
// Enum::Variant(...) 调用解析、enum-variant symbolKind、HAS_PARENT edges

/// 简单枚举（无数据 variant）
pub enum Color {
    Red,
    Green,
    Blue,
}

/// 元组 variant 枚举
pub enum WebEvent {
    PageLoad,
    KeyPress(char),
    Click { x: i64, y: i64 },
}

/// 调用简单 variant
pub fn make_red() -> Color {
    Color::Red
}

/// 调用元组 variant
pub fn make_keypress(c: char) -> WebEvent {
    WebEvent::KeyPress(c)
}

/// 调用结构体 variant
pub fn make_click(x: i64, y: i64) -> WebEvent {
    WebEvent::Click { x, y }
}

/// 方法中调用 variant
pub struct EventLogger;

impl EventLogger {
    pub fn log_click(&self, x: i64, y: i64) -> WebEvent {
        WebEvent::Click { x, y }
    }
}
