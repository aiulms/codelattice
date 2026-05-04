// receiver-type-aware method resolution fixture
// 验证：
//   - 显式类型注解的 let 绑定 → scan_variable_type_annotation → lookup_receiver_type_method
//   - confidence 0.65（高于 trait-only 0.55）
//   - 无类型注解的 let 绑定 → 不误解析（保持 unresolved）

// Vec type annotation → push, len
pub fn vec_methods() {
    let v: Vec<i32> = Vec::new();
    v.push(1);
    let _len = v.len();
}

// str type annotation → starts_with, trim_start
pub fn str_methods() {
    let s: &str = "hello";
    s.starts_with("he");
    s.trim_start();
}

// Option type annotation → unwrap_or
pub fn option_methods() {
    let o: Option<i32> = Some(42);
    o.unwrap_or(0);
}

// Result type annotation → is_ok, is_err
pub fn result_methods() {
    let r: Result<i32, &str> = Ok(42);
    r.is_ok();
    r.is_err();
}

// String type annotation → len
pub fn string_methods() {
    let s: String = String::from("hello");
    s.len();
}

// No type annotation — should remain unresolved
pub fn no_type_annotation() {
    let v = vec![1, 2, 3];
    v.push(4); // unresolved: no type annotation on let binding
}

// Function parameter — not supported in Phase 2
pub fn param_method(name: &str) {
    name.len(); // unresolved: name is a parameter, not let binding
}
