// compile-valid: true
// 嵌套模块层次结构 contract fixture
// 验证 crate:: 路径、super:: 路径、import-resolved 跨文件调用

pub mod utils;

pub fn top_level() -> i32 {
    42
}

/// crate:: 限定路径调用深层模块函数
pub fn call_via_crate_path() -> i32 {
    crate::utils::calculations::multiply(3, 7)
}
