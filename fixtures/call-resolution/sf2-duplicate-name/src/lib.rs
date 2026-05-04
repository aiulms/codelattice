// static-analysis-only: not compilable in Rust (process in mod a/b not visible at crate level)
// Tests same-file heuristic detecting ambiguous duplicate-name Functions
mod a {
    pub fn process() -> i32 { 1 }
}
mod b {
    pub fn process() -> i32 { 2 }
}
fn main_fn() -> i32 {
    process()
}
