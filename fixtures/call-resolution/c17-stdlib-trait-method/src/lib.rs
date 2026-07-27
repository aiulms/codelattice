// 验证 C1 扩展：唯一定义的 stdlib trait method 解析
// 这些 method-call 在 crate 内无同名 method，receiver 类型不可静态推断，
// 但 method 本身在 std 中唯一定义在某个 trait/type 上，可安全映射。
// confidence 0.55（与现有 to_string/clone/collect 一致：stdlib trait method，receiver 未验证）

pub fn iterator_unique_methods() -> usize {
    let v = vec![1, 2, 3, 4, 5];
    // count() 唯一定义在 Iterator trait
    let n = v.iter().count();
    // any() 唯一定义在 Iterator trait
    let _has = v.iter().any(|x| *x > 3);
    // find() 唯一定义在 Iterator trait
    let _first = v.iter().find(|x| **x == 3);
    // cloned() 唯一定义在 Iterator trait
    let _cloned: Vec<i32> = v.iter().cloned().collect();
    n
}

pub fn option_unique_methods(o: Option<i32>) -> bool {
    // is_some() 唯一定义在 Option（Result 上是 is_ok，无歧义）
    o.is_some()
}

pub fn result_unique_methods(r: Result<i32, String>) -> bool {
    // is_ok() 唯一定义在 Result（Option 上是 is_some，无歧义）
    r.is_ok()
}
