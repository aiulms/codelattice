// compile-valid: inline module 内 same-module 调用
// 因 modulePath flat 限制，call site modulePath = "crate"（非 "crate::inner"）
// same-module lookup 失败，same-file heuristic 成功解析
mod inner {
    fn helper() -> i32 { 42 }
    pub fn call_it() -> i32 { helper() }
}
