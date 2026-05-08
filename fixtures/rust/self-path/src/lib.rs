// self-path fixture: 测试 self:: 路径解析
// compile-valid
// 覆盖：self::free_function（crate 级）、HAS_PARENT edges、模块结构、DESIGNATION

pub fn top_level_fn() -> i32 {
    42
}

pub struct Calculator {
    base: i32,
}

impl Calculator {
    pub fn new(base: i32) -> Self {
        Calculator { base }
    }

    pub fn add(&self, x: i32) -> i32 {
        self.base + x
    }
}

/// 直接调用（无 self:: 前缀）— 基准对照
pub fn direct_caller() -> i32 {
    top_level_fn()
}

/// self:: 路径调用 free function（crate 级，modulePath 正确）
pub fn self_caller() -> i32 {
    self::top_level_fn()
}

/// self:: 路径调用 associated function（self::Type::method 模式）
pub fn self_associated_caller() -> Calculator {
    self::Calculator::new(10)
}

pub mod inner {
    pub fn inner_fn() -> i32 {
        100
    }
}

pub mod deeper {
    pub mod nested {
        pub fn deep_fn() -> i32 {
            300
        }
    }
}
