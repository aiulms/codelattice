fn inner_helper() -> i32 {
    5
}

pub fn call_self() {
    let v = self::inner_helper();
    let _ = v;
}
