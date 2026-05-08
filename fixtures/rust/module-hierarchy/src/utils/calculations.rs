use super::double;

/// 通过 use super::double 导入后调用（import-resolved）
pub fn multiply(a: i32, b: i32) -> i32 {
    double(a) * b
}

/// 直接 super:: 路径调用（super-path-resolved）
pub fn call_super_direct() -> i32 {
    super::double(5)
}
